//! Agent core abstraction — pluggable backend for LLM orchestration and tool calling.
//!
//! Defines the [`AgentEngine`] trait and provides implementations that delegate
//! to external agent backends via a persistent Bun sidecar (NDJSON JSON-RPC).
//!
//! ## Architecture
//!
//! ```text
//! Companion (Tauri backend)
//!   └── agent::AgentEngine trait
//!         └── OmpAgentSidecar    ← Bun sidecar (NDJSON JSON-RPC over stdio)
//! ```
//!
//! The [`AgentEngine`] abstracts over conversation management, tool selection,
//! and LLM invocation — so Companion's upper layers (perception, presentation)
//! never interact with the LLM or tools directly.

pub mod omp_sidecar;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Role in a conversation message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A single turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
}

/// Response from the agent engine.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub text: String,
}

/// Agent engine: sends a message and returns a complete response.
#[async_trait]
pub trait AgentEngine: Send + Sync {
    /// Send a user message and wait for the full agent reply.
    async fn chat(&self, message: &str, history: &[ConversationMessage], system_prompt: Option<&str>) -> Result<AgentResponse, AgentError>;
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent subprocess not running")]
    NotRunning,
    #[error("agent subprocess crashed: {0}")]
    SubprocessCrashed(String),
    #[error("RPC communication error: {0}")]
    RpcError(String),
    #[error("agent returned an error: {0}")]
    AgentReturnedError(String),
    #[error("timeout waiting for agent response")]
    Timeout,
}
