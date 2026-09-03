use crate::admission::AdmissionControl;
use crate::cgroup::{read_cgroup_stats, try_reset_memory_peak, CgroupStats};
use crate::classify::{classify, RunOutputs};
use crate::error::EngineError;
use crate::workspace::RunWorkspace;
use crate::{image, spec, EngineConfig};

use codexec_exec_contract::{ExecutionEngine, ExecutionOutcome, ExecutionRequest};

use containerd_client::services::v1::container::Runtime;
use containerd_client::services::v1::snapshots::{
    snapshots_client::SnapshotsClient, PrepareSnapshotRequest, RemoveSnapshotRequest,
};
use containerd_client::services::v1::{
    containers_client::ContainersClient, tasks_client::TasksClient, Container, CreateContainerRequest,
    CreateTaskRequest, DeleteContainerRequest, DeleteTaskRequest, KillRequest, StartRequest, WaitRequest,
};
use containerd_client::tonic::transport::Channel;
use containerd_client::tonic::Request;
use containerd_client::with_namespace;

use async_trait::async_trait;
use moka::future::Cache;
use prost_types::Any;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::timeout;

pub struct ContainerdExecutionEngine {
    channel: Channel,
    config: EngineConfig,
    chain_id_cache: Cache<String, String>,
    admission: AdmissionControl,
}

impl ContainerdExecutionEngine {
    pub async fn connect(config: EngineConfig) -> Result<Self, EngineError> {
        let channel = containerd_client::connect(&config.containerd_socket_path)
            .await
            .map_err(|e| EngineError::Containerd(format!("failed to connect to containerd: {e}")))?;
        let admission = AdmissionControl::new(config.total_cpu_cores, config.total_memory_bytes);
        Ok(Self {
            channel,
            chain_id_cache: Cache::builder().max_capacity(256).build(),
            config,
            admission,
        })
    }

    fn cgroups_path_relative(container_id: &str) -> String {
        format!("/codexec/{container_id}")
    }

    fn host_cgroup_dir(&self, container_id: &str) -> PathBuf {
        self.config.cgroup_root.join("codexec").join(container_id)
    }

    async fn resolve_chain_id(&self, image_ref: &str) -> Result<String, EngineError> {
        if let Some(cached) = self.chain_id_cache.get(image_ref).await {
            return Ok(cached);
        }
        let chain_id = image::resolve_chain_id(self.channel.clone(), &self.config.namespace, image_ref).await?;
        self.chain_id_cache.insert(image_ref.to_string(), chain_id.clone()).await;
        Ok(chain_id)
    }

    async fn prepare(&self, req: &ExecutionRequest) -> Result<RunContext, EngineError> {
        let container_id = format!("codexec-{}", req.run_id);
        let workspace = RunWorkspace::create(&self.config.workspace_root, req.run_id, req)?;

        let chain_id = self.resolve_chain_id(&req.image_ref).await?;

        let mut snapshots = SnapshotsClient::new(self.channel.clone());
        let mounts = snapshots
            .prepare(with_namespace!(
                PrepareSnapshotRequest {
                    snapshotter: self.config.snapshotter.clone(),
                    key: container_id.clone(),
                    parent: chain_id,
                    labels: Default::default(),
                },
                &self.config.namespace
            ))
            .await?
            .into_inner()
            .mounts;

        let cgroups_path = Self::cgroups_path_relative(&container_id);
        let oci_spec_json = spec::build_spec(req, &workspace, &cgroups_path)?;
        let spec_any = Any {
            type_url: "types.containerd.io/opencontainers/runtime-spec/1/Spec".into(),
            value: oci_spec_json.into_bytes(),
        };

        let mut containers = ContainersClient::new(self.channel.clone());
        containers
            .create(with_namespace!(
                CreateContainerRequest {
                    container: Some(Container {
                        id: container_id.clone(),
                        image: req.image_ref.clone(),
                        runtime: Some(Runtime { name: "io.containerd.runc.v2".into(), options: None }),
                        spec: Some(spec_any),
                        snapshotter: self.config.snapshotter.clone(),
                        snapshot_key: container_id.clone(),
                        ..Default::default()
                    }),
                },
                &self.config.namespace
            ))
            .await?;

        Ok(RunContext { container_id, workspace, rootfs_mounts: mounts })
    }

    async fn execute_inner(&self, req: &ExecutionRequest, ctx: &RunContext) -> Result<ExecutionOutcome, EngineError> {
        let mut tasks = TasksClient::new(self.channel.clone());
        let rootfs = ctx.rootfs_mounts.clone();

        tasks
            .create(with_namespace!(
                CreateTaskRequest {
                    container_id: ctx.container_id.clone(),
                    rootfs,
                    stdin: String::new(),
                    stdout: "/dev/null".to_string(),
                    stderr: "/dev/null".to_string(),
                    ..Default::default()
                },
                &self.config.namespace
            ))
            .await?;

        tasks
            .start(with_namespace!(
                StartRequest { container_id: ctx.container_id.clone(), ..Default::default() },
                &self.config.namespace
            ))
            .await?;

        let start = Instant::now();
        let outer_budget = Duration::from_millis(
            req.compile_time_limit_ms + req.wall_time_limit_ms + self.config.kill_grace_period_ms,
        );

        let cgroup_dir = self.host_cgroup_dir(&ctx.container_id);
        let wait_req = with_namespace!(
            WaitRequest { container_id: ctx.container_id.clone(), ..Default::default() },
            &self.config.namespace
        );

        let poll = self.poll_and_maybe_kill(ctx, req, &cgroup_dir);

        let mut tasks_wait = TasksClient::new(self.channel.clone());
        let timed_out = tokio::select! {
            res = timeout(outer_budget, tasks_wait.wait(wait_req)) => {
                match res {
                    Ok(Ok(_)) => false,
                    Ok(Err(status)) => return Err(status.into()),
                    Err(_elapsed) => true,
                }
            }
            () = poll => true, // the CPU-time poll decided to kill
        };

        if timed_out {
            let _ = tasks
                .kill(with_namespace!(
                    KillRequest { container_id: ctx.container_id.clone(), exec_id: String::new(), signal: 9, all: true },
                    &self.config.namespace
                ))
                .await;
            let _ = timeout(
                Duration::from_secs(5),
                tasks.wait(with_namespace!(
                    WaitRequest { container_id: ctx.container_id.clone(), ..Default::default() },
                    &self.config.namespace
                )),
            )
            .await;
        }

        let wall_time_ms = start.elapsed().as_millis() as u64;
        let stats = read_cgroup_stats(&cgroup_dir).unwrap_or_default();
        let outputs = read_run_outputs(&ctx.workspace);

        Ok(classify(
            &outputs,
            timed_out,
            &stats,
            stats.cpu_usage_usec / 1000,
            req.memory_limit_kb,
            wall_time_ms,
            req.max_output_bytes as usize,
        ))
    }

    /// Concurrently watches the phase-marker file (to snapshot a CPU-usage
    /// baseline at the compile/run boundary, isolating run-phase CPU time
    /// from combined compile+run cgroup accounting) and polls cpu.stat to
    /// enforce the user-facing `cpu_time_limit_ms` budget — cgroups have
    /// no native "kill after N CPU-seconds" primitive, only a rate
    /// throttle, so this is the only way to enforce it.
    async fn poll_and_maybe_kill(&self, ctx: &RunContext, req: &ExecutionRequest, cgroup_dir: &PathBuf) {
        let mut interval = tokio::time::interval(Duration::from_millis(self.config.cpu_poll_interval_ms));
        let mut baseline: Option<CgroupStats> = None;
        loop {
            interval.tick().await;
            let phase = tokio::fs::read_to_string(ctx.workspace.phase_file()).await.unwrap_or_default();
            let phase = phase.trim();

            if baseline.is_none() && phase == "running" {
                let b = read_cgroup_stats(cgroup_dir).unwrap_or_default();
                try_reset_memory_peak(cgroup_dir);
                baseline = Some(b);
            }

            if phase == "done" || phase == "compile_failed" {
                // wrapper already exited; Tasks.Wait will resolve on its own.
                std::future::pending::<()>().await;
            }

            if let Ok(stats) = read_cgroup_stats(cgroup_dir) {
                let base_usec = baseline.map(|b| b.cpu_usage_usec).unwrap_or(0);
                let run_cpu_ms = stats.cpu_usage_usec.saturating_sub(base_usec) / 1000;
                if phase == "running" && run_cpu_ms >= req.cpu_time_limit_ms {
                    return;
                }
            }
        }
    }

    async fn cleanup(&self, ctx: &RunContext) {
        let mut tasks = TasksClient::new(self.channel.clone());
        let _ = tasks
            .delete(with_namespace!(
                DeleteTaskRequest { container_id: ctx.container_id.clone() },
                &self.config.namespace
            ))
            .await;

        let mut containers = ContainersClient::new(self.channel.clone());
        let _ = containers
            .delete(with_namespace!(DeleteContainerRequest { id: ctx.container_id.clone() }, &self.config.namespace))
            .await;

        let mut snapshots = SnapshotsClient::new(self.channel.clone());
        let _ = snapshots
            .remove(with_namespace!(
                RemoveSnapshotRequest {
                    snapshotter: self.config.snapshotter.clone(),
                    key: ctx.container_id.clone(),
                },
                &self.config.namespace
            ))
            .await;
        // ctx.workspace (RunWorkspace) drops when `ctx` is dropped by the
        // caller -> synchronous recursive rm, runs even on panic unwind.
    }
}

struct RunContext {
    container_id: String,
    workspace: RunWorkspace,
    rootfs_mounts: Vec<containerd_client::types::Mount>,
}

fn read_run_outputs(ws: &RunWorkspace) -> RunOutputs {
    let read = |name: &str| std::fs::read_to_string(ws.out_dir().join(name)).unwrap_or_default();
    let parse_i32 = |s: String| s.trim().parse::<i32>().unwrap_or(-1);

    RunOutputs {
        phase: read("phase").trim().to_string(),
        compile_stderr: read("compile_stderr.txt"),
        compile_exit_code: parse_i32(read("compile_exit_code")),
        run_stdout: read("run_stdout.txt"),
        run_stderr: read("run_stderr.txt"),
        run_exit_code: parse_i32(read("run_exit_code")),
    }
}

#[async_trait]
impl ExecutionEngine for ContainerdExecutionEngine {
    async fn execute(&self, req: ExecutionRequest) -> ExecutionOutcome {
        let _permit = self.admission.acquire(req.cpu_limit_cores, req.memory_limit_kb * 1024).await;

        let ctx = match self.prepare(&req).await {
            Ok(ctx) => ctx,
            Err(e) => {
                return ExecutionOutcome::InternalError { message: e.to_string(), retryable: e.retryable() }
            }
        };

        let result = self.execute_inner(&req, &ctx).await;

        if let Err(e) = timeout(Duration::from_secs(10), self.cleanup(&ctx)).await {
            tracing::warn!(run_id = %req.run_id, error = %e, "containerd cleanup timed out");
        }

        match result {
            Ok(outcome) => outcome,
            Err(e) => ExecutionOutcome::InternalError { message: e.to_string(), retryable: e.retryable() },
        }
    }
}

// Runtime precondition (not enforced by this code, documented for ops):
// containerd's runc config must NOT set SystemdCgroup = true, so the
// cgroupsPath we set above stays a plain, predictable filesystem path
// under self.config.cgroup_root.
