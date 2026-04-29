pub mod a2a;
pub mod agent;
pub mod builder;
pub mod config;
pub mod evolution;
pub mod feedback;
// pub mod hermes; // Removed: replaced by agent::reasoning module
pub mod layer_router;
pub mod life_model;
pub mod llm;
pub mod mcp;
pub mod mcp_audit;
pub mod memory;
pub mod memory_cache;
pub mod ollama;
pub mod plugins;
pub mod privacy;
pub mod reflex_engine;
pub mod router;
pub mod scheduler;
pub mod skills;
pub mod tool_manifest;
pub mod tool_permissions;
pub mod vectors;
pub mod versioning;

#[cfg(test)]
pub(crate) static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
