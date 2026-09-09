#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("image not found in local cache: {0}")]
    ImageNotFound(String),
    #[error("runc error: {0}")]
    Runc(String),
    #[error("image pull/unpack error: {0}")]
    ImagePull(String),
    #[error("sandbox setup failed: {0}")]
    SandboxSetup(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl EngineError {
    /// Whether the NATS/queue layer should let redelivery retry this
    /// submission, vs. treat it as terminal. A missing image won't fix
    /// itself on retry; a transient runc/host hiccup might.
    pub fn retryable(&self) -> bool {
        !matches!(self, EngineError::ImageNotFound(_))
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
