#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("image not found in containerd store: {0}")]
    ImageNotFound(String),
    #[error("containerd error: {0}")]
    Containerd(String),
    #[error("sandbox setup failed: {0}")]
    SandboxSetup(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl EngineError {
    /// Whether the NATS/queue layer should let redelivery retry this
    /// submission, vs. treat it as terminal. A missing image won't fix
    /// itself on retry; a transient containerd hiccup might.
    pub fn retryable(&self) -> bool {
        !matches!(self, EngineError::ImageNotFound(_))
    }
}

impl From<containerd_client::tonic::Status> for EngineError {
    fn from(status: containerd_client::tonic::Status) -> Self {
        if status.code() == containerd_client::tonic::Code::NotFound {
            EngineError::ImageNotFound(status.message().to_string())
        } else {
            EngineError::Containerd(status.message().to_string())
        }
    }
}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError::SandboxSetup(e.to_string())
    }
}

impl From<serde_json::Error> for EngineError {
    fn from(e: serde_json::Error) -> Self {
        EngineError::Internal(e.to_string())
    }
}
