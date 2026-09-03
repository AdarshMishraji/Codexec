use crate::error::EngineError;
use codexec_exec_contract::ExecutionRequest;
use std::fmt::Write;

/// Single-quote shell-escapes one argv element: wraps it in `'...'`,
/// replacing any internal `'` with `'\''`. Never string-interpolated as a
/// shell command line — this is what keeps argv-based compile/run
/// commands injection-safe.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn quote_argv(argv: &[String]) -> String {
    argv.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
}

/// Generates the in-container wrapper script that separates compile
/// output from run output within a single container (avoiding the
/// double-round-trip and artifact-handoff cost of two containerd tasks).
/// The wrapper's phase-marker file is trusted only for phase/output
/// separation bookkeeping — resource enforcement always comes from the
/// kernel-enforced cgroup files, read from the host.
pub fn render(req: &ExecutionRequest) -> Result<String, EngineError> {
    let mut s = String::new();
    writeln!(s, "#!/bin/sh").ok();
    writeln!(s, "set -u").ok();
    // The source file (and any compiled artifact, for compiled languages)
    // lives alongside it in /sandbox/in - not /sandbox itself, which is
    // shared with the separate /sandbox/out output-capture directory.
    writeln!(s, "cd /sandbox/in").ok();
    writeln!(s, "echo compiling > /sandbox/out/phase").ok();
    writeln!(s).ok();

    if let Some(compile_cmd) = &req.compile_cmd {
        let compile_secs = req.compile_time_limit_ms.div_ceil(1000).max(1);
        writeln!(s, "if [ -f /sandbox/in/has_compile ]; then").ok();
        writeln!(
            s,
            "  timeout -s KILL {compile_secs} sh -c 'exec \"$@\"' -- {} \\",
            quote_argv(compile_cmd)
        )
        .ok();
        writeln!(s, "    > /sandbox/out/compile_stdout.txt 2> /sandbox/out/compile_stderr.txt").ok();
        writeln!(s, "  compile_status=$?").ok();
        writeln!(s, "  echo \"$compile_status\" > /sandbox/out/compile_exit_code").ok();
        writeln!(s, "  if [ \"$compile_status\" -ne 0 ]; then").ok();
        writeln!(s, "    echo compile_failed > /sandbox/out/phase").ok();
        writeln!(s, "    exit 0").ok();
        writeln!(s, "  fi").ok();
        writeln!(s, "fi").ok();
        writeln!(s).ok();
    }

    let run_secs = req.wall_time_limit_ms.div_ceil(1000).max(1);
    writeln!(s, "echo running > /sandbox/out/phase").ok();
    writeln!(
        s,
        "timeout -s KILL {run_secs} sh -c 'exec \"$@\" < /sandbox/in/stdin.txt' -- {} \\",
        quote_argv(&req.run_cmd)
    )
    .ok();
    writeln!(s, "  > /sandbox/out/run_stdout.txt 2> /sandbox/out/run_stderr.txt").ok();
    writeln!(s, "run_status=$?").ok();
    writeln!(s, "echo \"$run_status\" > /sandbox/out/run_exit_code").ok();
    writeln!(s, "echo done > /sandbox/out/phase").ok();
    writeln!(s, "exit 0").ok();

    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_neutralizes_injection_attempts() {
        let evil = vec!["python3".to_string(), "-c".to_string(), "print('hi'); rm -rf /".to_string()];
        let quoted = quote_argv(&evil);
        // The whole malicious arg must stay inside a single quoted token.
        assert!(quoted.contains("'print('\\''hi'\\''); rm -rf /'"));
    }
}
