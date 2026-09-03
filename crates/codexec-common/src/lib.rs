//! Shared types, DB models, and config loading for the codexec platform.
//! No NATS or containerd dependencies here by design — this crate is the
//! one thing every other crate can depend on without pulling in a transport
//! or a sandboxing stack.

pub mod config;
pub mod grading;
pub mod models;
pub mod registry;
