use crate::admission::AdmissionControl;
use crate::cgroup::{read_cgroup_stats, try_reset_memory_peak, CgroupStats};
use crate::classify::{classify, RunOutputs};
use crate::error::EngineError;
use crate::workspace::RunWorkspace;
use crate::{image, spec, EngineConfig};

use codexec_exec_contract::{ExecutionEngine, ExecutionOutcome, ExecutionRequest};

use async_trait::async_trait;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

pub struct RuncExecutionEngine {
    config: EngineConfig,
    admission: AdmissionControl,
}

impl RuncExecutionEngine {
    pub fn new(config: EngineConfig) -> Self {
        let admission = AdmissionControl::new(config.total_cpu_cores, config.total_memory_bytes);
        Self { config, admission }
    }

    fn cgroups_path_relative(container_id: &str) -> String {
        format!("/codexec/{container_id}")
    }

    fn host_cgroup_dir(&self, container_id: &str) -> PathBuf {
        self.config.cgroup_root.join("codexec").join(container_id)
    }

    /// Every `runc` invocation must agree on the same `--root` state
    /// directory - it's how runc finds a container it created earlier by
    /// id (for `start`/`state`/`kill`/`delete`) rather than a fresh, empty
    /// view of state.
    ///
    /// Deliberately does NOT use `Command::output()`/`wait_with_output()`:
    /// `runc create` forks a container-init helper that stays alive
    /// (blocked, waiting for `runc start`'s exec-fifo signal) and inherits
    /// whatever stdout/stderr fds we hand the direct `runc create`
    /// process. If those are pipes, that lingering grandchild holds the
    /// write end open forever, so `output()`'s "read until EOF" never
    /// completes even though `runc create` itself already exited -
    /// confirmed live (the very first `run_runc` call for "create" simply
    /// never returned). Redirecting to plain temp files instead sidesteps
    /// this entirely: writes land immediately, and reading them back after
    /// the direct child's own exit status is available doesn't need
    /// anyone else to close anything.
    async fn run_runc(&self, args: &[&str]) -> Result<std::process::Output, EngineError> {
        let runc_root = self.config.runc_root.display().to_string();

        let mut stdout_file = tempfile::tempfile().map_err(|e| EngineError::Runc(format!("temp file: {e}")))?;
        let mut stderr_file = tempfile::tempfile().map_err(|e| EngineError::Runc(format!("temp file: {e}")))?;
        let stdout_fd = stdout_file.try_clone().map_err(|e| EngineError::Runc(format!("temp file clone: {e}")))?;
        let stderr_fd = stderr_file.try_clone().map_err(|e| EngineError::Runc(format!("temp file clone: {e}")))?;

        let result = Command::new("runc")
            .arg("--root")
            .arg(&runc_root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_fd))
            .stderr(Stdio::from(stderr_fd))
            .status()
            .await
            .map_err(|e| EngineError::Runc(format!("failed to exec runc {args:?}: {e}")));

        let status = match result {
            Ok(status) => status,
            Err(e) => {
                tracing::debug!(?args, error = %e, "runc invocation failed to spawn");
                return Err(e);
            }
        };

        let mut stdout = Vec::new();
        let _ = stdout_file.seek(SeekFrom::Start(0));
        let _ = stdout_file.read_to_end(&mut stdout);
        let mut stderr = Vec::new();
        let _ = stderr_file.seek(SeekFrom::Start(0));
        let _ = stderr_file.read_to_end(&mut stderr);

        tracing::debug!(
            ?args,
            ?status,
            stdout = %String::from_utf8_lossy(&stdout),
            stderr = %String::from_utf8_lossy(&stderr),
            "runc invocation"
        );

        Ok(std::process::Output { status, stdout, stderr })
    }

    async fn run_runc_checked(&self, args: &[&str]) -> Result<(), EngineError> {
        let output = self.run_runc(args).await?;
        if !output.status.success() {
            return Err(EngineError::Runc(format!("runc {args:?} failed: {}", String::from_utf8_lossy(&output.stderr))));
        }
        Ok(())
    }

    /// `runc state <id>`'s `status` field, or `None` once the container id
    /// is no longer known to runc at all (which we also treat as exited -
    /// see the call site).
    async fn runc_status(&self, container_id: &str) -> Option<String> {
        let output = self.run_runc(&["state", container_id]).await.ok()?;
        if !output.status.success() {
            return None;
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        json.get("status").and_then(|s| s.as_str()).map(str::to_string)
    }

    async fn prepare(&self, req: &ExecutionRequest) -> Result<RunContext, EngineError> {
        let container_id = format!("codexec-{}", req.run_id);
        let workspace = RunWorkspace::create(&self.config.workspace_root, req.run_id, req)?;
        let rootfs = image::ensure_present(&self.config.image_cache_root, &req.image_ref).await?;

        let cgroups_path = Self::cgroups_path_relative(&container_id);
        let spec_json = spec::build_spec(req, &workspace, &rootfs, &cgroups_path)?;
        // A runc "bundle" is just a directory containing config.json -
        // nothing else in it is inspected, so co-locating it with the
        // /sandbox workspace (already bind-mounted at /sandbox by the spec
        // itself) needs no extra directory or cleanup of its own.
        tokio::fs::write(workspace.host_dir().join("config.json"), spec_json).await?;

        Ok(RunContext { container_id, workspace })
    }

    async fn execute_inner(&self, req: &ExecutionRequest, ctx: &RunContext) -> Result<ExecutionOutcome, EngineError> {
        let bundle = ctx.workspace.host_dir().display().to_string();

        // create+start (not the simpler `runc run`, deliberately): `run`
        // bundles create+start+wait+delete into one call and auto-deletes
        // the container - including its cgroup - the instant the process
        // exits, before we ever get a chance to read final cgroup stats
        // (confirmed live: cgroup_dir had vanished by the time `run`'s own
        // process returned). create+start decouples "process exited" from
        // "state deleted" exactly the way containerd's Task model did for
        // us before this engine dropped containerd - we control the
        // `delete` timing ourselves, after reading stats.
        self.run_runc_checked(&["create", "--bundle", &bundle, &ctx.container_id]).await?;
        self.run_runc_checked(&["start", &ctx.container_id]).await?;

        let start = Instant::now();
        let outer_budget = Duration::from_millis(
            req.compile_time_limit_ms + req.wall_time_limit_ms + self.config.kill_grace_period_ms,
        );
        let cgroup_dir = self.host_cgroup_dir(&ctx.container_id);

        let (timed_out, stats) = self.wait_poll_and_maybe_kill(ctx, req, &cgroup_dir, start, outer_budget).await;

        let wall_time_ms = start.elapsed().as_millis() as u64;
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

    /// The one control loop driving a running submission: waits for
    /// natural completion (`runc state` reporting the container stopped,
    /// or its id no longer existing), enforces the wall-clock timeout, and
    /// enforces the user-facing `cpu_time_limit_ms` budget by polling
    /// cpu.stat and killing once the run-phase's own CPU time (baselined
    /// at the compile/run boundary, isolating it from combined
    /// compile+run accounting) crosses it - cgroups have no native "kill
    /// after N CPU-seconds" primitive, only a rate throttle, so polling is
    /// the only way to enforce it. Returns the last cgroup snapshot taken
    /// before any kill/delete, since the cgroup disappears once we
    /// `runc delete` in `cleanup`.
    async fn wait_poll_and_maybe_kill(
        &self,
        ctx: &RunContext,
        req: &ExecutionRequest,
        cgroup_dir: &Path,
        start: Instant,
        outer_budget: Duration,
    ) -> (bool, CgroupStats) {
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

            let stats = read_cgroup_stats(cgroup_dir).unwrap_or_default();

            // Natural completion: the container's own process (our
            // wrapper script) has exited on its own.
            match self.runc_status(&ctx.container_id).await {
                Some(status) if status != "stopped" => {}
                _ => return (false, stats),
            }

            let base_usec = baseline.map(|b| b.cpu_usage_usec).unwrap_or(0);
            let run_cpu_ms = stats.cpu_usage_usec.saturating_sub(base_usec) / 1000;
            if phase == "running" && run_cpu_ms >= req.cpu_time_limit_ms {
                let stats = self.force_kill_and_settle(&ctx.container_id, cgroup_dir).await.unwrap_or(stats);
                return (true, stats);
            }

            if start.elapsed() >= outer_budget {
                let stats = self.force_kill_and_settle(&ctx.container_id, cgroup_dir).await.unwrap_or(stats);
                return (true, stats);
            }
        }
    }

    /// Signals the actual container process via runc's own state tracking
    /// (container-id, not a raw pid) - `--all` reaches the whole process
    /// tree. Waits briefly for the kill to actually land (SIGKILL isn't
    /// synchronous) before taking the final cgroup reading, so the
    /// snapshot reflects the process's true end state rather than a
    /// mid-teardown moment.
    async fn force_kill_and_settle(&self, container_id: &str, cgroup_dir: &Path) -> Option<CgroupStats> {
        let _ = self.run_runc(&["kill", container_id, "KILL", "--all"]).await;
        for _ in 0..25 {
            match self.runc_status(container_id).await {
                Some(status) if status != "stopped" => tokio::time::sleep(Duration::from_millis(20)).await,
                _ => break,
            }
        }
        read_cgroup_stats(cgroup_dir).ok()
    }

    async fn cleanup(&self, ctx: &RunContext) {
        let _ = self.run_runc(&["delete", "--force", &ctx.container_id]).await;
        // ctx.workspace (RunWorkspace) drops when `ctx` is dropped by the
        // caller -> synchronous recursive rm, runs even on panic unwind.
        // The shared image rootfs under image_cache_root is never touched
        // here - it's read-only and outlives every individual submission.
    }
}

struct RunContext {
    container_id: String,
    workspace: RunWorkspace,
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
impl ExecutionEngine for RuncExecutionEngine {
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
            tracing::warn!(run_id = %req.run_id, error = %e, "runc cleanup timed out");
        }

        match result {
            Ok(outcome) => outcome,
            Err(e) => ExecutionOutcome::InternalError { message: e.to_string(), retryable: e.retryable() },
        }
    }
}

// Runtime precondition (not enforced by this code, documented for ops):
// runc's cgroupfs manager (not systemd) must be in effect, so the
// cgroupsPath we set above stays a plain, predictable filesystem path
// under self.config.cgroup_root. This is runc's default when nothing
// requests the systemd cgroup driver, so it holds as long as nothing else
// on the host forces SystemdCgroup behavior for runc invocations under
// this --root.
