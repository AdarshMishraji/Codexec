use crate::error::EngineError;
use crate::workspace::RunWorkspace;
use codexec_exec_contract::ExecutionRequest;
use oci_spec::runtime::{
    LinuxBuilder, LinuxCpuBuilder, LinuxMemoryBuilder, LinuxPidsBuilder, LinuxResourcesBuilder, MountBuilder,
    ProcessBuilder, RootBuilder, SpecBuilder,
};
use std::path::Path;

/// Builds the OCI runtime spec for one submission's container: a
/// read-only, shared image rootfs (all writes go through the bind-mounted
/// /sandbox, which the worker fully controls and cleans up - since nothing
/// ever writes to the image root, every concurrent submission for the same
/// language can safely point `root.path` at the exact same unpacked
/// directory, no per-run copy or overlay needed), cgroup limits derived
/// from the request (cpu_limit_cores -> quota/period rate cap;
/// memory_limit_kb -> hard memory.max), and an explicit cgroupsPath so the
/// caller can locate the same cgroup afterward for stats.
pub fn build_spec(req: &ExecutionRequest, ws: &RunWorkspace, rootfs: &Path, cgroups_path: &str) -> Result<String, EngineError> {
    let period_us: u64 = 100_000;
    let quota_us: i64 = (req.cpu_limit_cores * period_us as f64).round() as i64;

    let resources = LinuxResourcesBuilder::default()
        .memory(
            LinuxMemoryBuilder::default()
                .limit(req.memory_limit_kb as i64 * 1024)
                .build()
                .map_err(|e| EngineError::Internal(e.to_string()))?,
        )
        .cpu(
            LinuxCpuBuilder::default()
                .quota(quota_us)
                .period(period_us)
                .build()
                .map_err(|e| EngineError::Internal(e.to_string()))?,
        )
        .pids(LinuxPidsBuilder::default().limit(512i64).build().map_err(|e| EngineError::Internal(e.to_string()))?)
        .build()
        .map_err(|e| EngineError::Internal(e.to_string()))?;

    // oci-spec's default namespace list already includes a Cgroup
    // namespace (so the container can't see the host's cgroup tree) -
    // adding a second one produces "duplicated ns" from runc.
    let namespaces = oci_spec::runtime::get_default_namespaces();

    let linux = LinuxBuilder::default()
        .cgroups_path(cgroups_path)
        .resources(resources)
        .namespaces(namespaces)
        .masked_paths(oci_spec::runtime::get_default_maskedpaths())
        .readonly_paths(oci_spec::runtime::get_default_readonly_paths())
        .build()
        .map_err(|e| EngineError::Internal(e.to_string()))?;

    let mut mounts = oci_spec::runtime::get_default_mounts();
    mounts.push(
        MountBuilder::default()
            .destination("/sandbox")
            .typ("bind")
            .source(ws.host_dir())
            .options(vec!["rbind".into(), "rw".into()])
            .build()
            .map_err(|e| EngineError::Internal(e.to_string()))?,
    );

    let process = ProcessBuilder::default()
        .args(vec!["/bin/sh".to_string(), "/sandbox/in/__run.sh".to_string()])
        .env(vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into()])
        .cwd("/sandbox")
        .no_new_privileges(true)
        .build()
        .map_err(|e| EngineError::Internal(e.to_string()))?;

    let spec = SpecBuilder::default()
        .process(process)
        .root(RootBuilder::default().path(rootfs).readonly(true).build().map_err(|e| EngineError::Internal(e.to_string()))?)
        .hostname("codexec")
        .mounts(mounts)
        .linux(linux)
        .build()
        .map_err(|e| EngineError::Internal(e.to_string()))?;

    serde_json::to_string(&spec).map_err(EngineError::from)
}
