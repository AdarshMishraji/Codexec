use crate::cgroup::CgroupStats;
use codexec_exec_contract::ExecutionOutcome;

pub struct RunOutputs {
    pub phase: String,
    pub compile_stderr: String,
    pub compile_exit_code: i32,
    pub run_stdout: String,
    pub run_stderr: String,
    pub run_exit_code: i32,
}

/// Classification precedence matters: exit code 137 (128+SIGKILL) alone is
/// ambiguous — it's produced both by our own wall-clock/CPU-time kill and
/// by an OOM kill. `memory.events`' `oom_kill` counter is the only
/// reliable disambiguator, so it's checked first, ahead of "we know we
/// killed it for a timeout".
#[allow(clippy::too_many_arguments)]
pub fn classify(
    outputs: &RunOutputs,
    timed_out: bool,
    stats: &CgroupStats,
    cpu_time_used_ms: u64,
    memory_limit_kb: u64,
    wall_time_ms: u64,
    max_output_bytes: usize,
) -> ExecutionOutcome {
    let (stdout, stdout_truncated) = truncate(&outputs.run_stdout, max_output_bytes);
    let (stderr, stderr_truncated) = truncate(&outputs.run_stderr, max_output_bytes);

    if outputs.phase == "compile_failed" {
        let (compile_stderr, compile_truncated) = truncate(&outputs.compile_stderr, max_output_bytes);
        return ExecutionOutcome::CompileError {
            stderr: compile_stderr,
            stderr_truncated: compile_truncated,
            exit_code: outputs.compile_exit_code,
            cpu_time_used_ms,
            wall_time_ms,
        };
    }

    if stats.oom_kill > 0 {
        return ExecutionOutcome::MemoryLimitExceeded {
            stdout,
            stderr,
            cpu_time_used_ms,
            memory_used_kb: (stats.memory_peak_bytes / 1024).max(memory_limit_kb),
        };
    }

    if timed_out && outputs.phase == "running" {
        return ExecutionOutcome::TimeLimitExceeded { stdout, stderr, cpu_time_used_ms, wall_time_ms };
    }

    if timed_out && outputs.phase == "compiling" {
        return ExecutionOutcome::CompileError {
            stderr: "compilation timed out".to_string(),
            stderr_truncated: false,
            exit_code: -1,
            cpu_time_used_ms,
            wall_time_ms,
        };
    }

    ExecutionOutcome::Completed {
        exit_code: outputs.run_exit_code,
        stdout,
        stdout_truncated,
        stderr,
        stderr_truncated,
        cpu_time_used_ms,
        memory_used_kb: stats.memory_peak_bytes / 1024,
        wall_time_ms,
    }
}

fn truncate(s: &str, max_bytes: usize) -> (String, bool) {
    if s.len() <= max_bytes {
        (s.to_string(), false)
    } else {
        let mut end = max_bytes;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        (s[..end].to_string(), true)
    }
}
