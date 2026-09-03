use crate::error::EngineError;
use crate::wrapper;
use codexec_exec_contract::ExecutionRequest;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Per-submission host directory bind-mounted into the container at
/// `/sandbox`. `TempDir`'s `Drop` is synchronous recursive removal, so
/// cleanup happens even on panic unwind — unlike the containerd-side
/// gRPC deletes, which need an explicit `.await` (see lifecycle.rs).
pub struct RunWorkspace {
    dir: tempfile::TempDir,
}

impl RunWorkspace {
    pub fn create(root: &Path, run_id: Uuid, req: &ExecutionRequest) -> Result<Self, EngineError> {
        std::fs::create_dir_all(root)?;
        let dir = tempfile::Builder::new()
            .prefix(&format!("run-{run_id}-"))
            .tempdir_in(root)
            .map_err(|e| EngineError::SandboxSetup(format!("failed to create workspace dir: {e}")))?;

        std::fs::create_dir_all(dir.path().join("in"))?;
        std::fs::create_dir_all(dir.path().join("out"))?;

        std::fs::write(dir.path().join("in").join(&req.source_filename), &req.source_code)?;
        std::fs::write(dir.path().join("in/stdin.txt"), &req.stdin)?;
        std::fs::write(dir.path().join("in/__run.sh"), wrapper::render(req)?)?;
        if req.compile_cmd.is_some() {
            std::fs::write(dir.path().join("in/has_compile"), "")?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path().join("out"), std::fs::Permissions::from_mode(0o777))?;
            std::fs::set_permissions(dir.path().join("in/__run.sh"), std::fs::Permissions::from_mode(0o755))?;
        }

        Ok(Self { dir })
    }

    pub fn host_dir(&self) -> &Path {
        self.dir.path()
    }

    pub fn phase_file(&self) -> PathBuf {
        self.dir.path().join("out/phase")
    }

    pub fn out_dir(&self) -> PathBuf {
        self.dir.path().join("out")
    }
}
