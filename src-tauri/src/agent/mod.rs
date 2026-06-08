//! Agent core abstraction — pluggable backend for LLM orchestration and tool calling.
//!
//! Defines the [`AgentEngine`] trait and provides implementations that delegate
//! to external agent backends such as **oh-my-pi** (`omp --mode rpc`).
//!
//! ## Architecture
//!
//! ```text
//! Companion (Tauri backend)
//!   └── agent::AgentEngine trait
//!         ├── OmpRpcClient    ← oh-my-pi RPC subprocess (recommended)
//!         └── DirectLlm       ← fallback: direct LLM API (no tool orchestration)
//! ```
//!
//! The [`AgentEngine`] abstracts over conversation management, tool selection,
//! and LLM invocation — so Companion's upper layers (perception, presentation)
//! never interact with the LLM or tools directly.

pub mod omp_rpc;

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

/// Response from the agent engine (non-streaming).
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub text: String,
    pub tool_calls: Vec<String>, // tool names that were invoked
}

/// Agent engine: sends a message and returns a complete response.
#[async_trait]
pub trait AgentEngine: Send + Sync {
    /// Send a user message and wait for the full agent reply.
    async fn chat(&self, message: &str, history: &[ConversationMessage]) -> Result<AgentResponse, AgentError>;

    /// Streamed variant — each string is either a text token or a tool-call marker.
    async fn chat_stream(
        &self,
        message: &str,
        history: &[ConversationMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError>;
}

/// Events produced by the streaming chat API.
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    /// A text token (for real-time TTS or display).
    Token(String),
    /// The agent started executing a tool.
    ToolStarted { name: String },
    /// The agent finished executing a tool.
    ToolCompleted { name: String, result: String },
    /// The full response is complete.
    Done,
    /// An error occurred.
    Error(String),
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
