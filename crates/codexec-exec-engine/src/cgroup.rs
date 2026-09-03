use std::path::Path;

#[derive(Debug, Clone, Copy, Default)]
pub struct CgroupStats {
    pub cpu_usage_usec: u64,
    pub memory_current_bytes: u64,
    pub memory_peak_bytes: u64,
    pub oom_kill: u64,
}

fn parse_kv_line(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key} ")).and_then(|v| v.trim().parse().ok()))
}

/// Reads cgroup v2 interface files directly rather than going through
/// containerd's `Tasks.Metrics` RPC: that RPC's response wraps a
/// cgroups-metrics protobuf that `containerd-client` does not vendor
/// (it lives in containerd's internal, not-officially-stable wire
/// format). Since the worker and containerd are colocated, reading these
/// files is exactly the data containerd's own shim would hand back, with
/// zero proto-version-skew risk.
pub fn read_cgroup_stats(cgroup_dir: &Path) -> std::io::Result<CgroupStats> {
    let cpu_stat = std::fs::read_to_string(cgroup_dir.join("cpu.stat"))?;
    let cpu_usage_usec = parse_kv_line(&cpu_stat, "usage_usec").unwrap_or(0);

    let memory_current_bytes =
        std::fs::read_to_string(cgroup_dir.join("memory.current"))?.trim().parse().unwrap_or(0);

    // memory.peak: Linux 5.19+. Missing file => older kernel, fall back to
    // memory.current as a weaker approximation rather than failing the run.
    let memory_peak_bytes = std::fs::read_to_string(cgroup_dir.join("memory.peak"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(memory_current_bytes);

    let events = std::fs::read_to_string(cgroup_dir.join("memory.events")).unwrap_or_default();
    let oom_kill = parse_kv_line(&events, "oom_kill").unwrap_or(0);

    Ok(CgroupStats { cpu_usage_usec, memory_current_bytes, memory_peak_bytes, oom_kill })
}

/// Resets memory.peak's high-water mark (Linux 6.9+ — any write resets
/// it). Used at the compile/run phase boundary to isolate run-phase peak
/// memory from combined compile+run accounting. Best-effort: an older
/// kernel returns EINVAL, silently ignored — the documented degradation
/// is reporting combined-phase peak instead of a crash.
pub fn try_reset_memory_peak(cgroup_dir: &Path) {
    let _ = std::fs::write(cgroup_dir.join("memory.peak"), b"0");
}
