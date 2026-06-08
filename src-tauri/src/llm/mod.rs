use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// A chat message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant" | "system" | "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A tool call the LLM wants to invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String, // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

/// A tool definition for LLM function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Response from a chat completion.
#[derive(Debug)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// Receiver for streaming tokens.
pub type StreamReceiver = tokio::sync::mpsc::Receiver<String>;

/// LLM provider trait.
#[async_trait]
pub trait ChatLlm: Send + Sync {
    /// Send messages and return a full response.
    async fn chat(&self, messages: &[ChatMessage], tools: &[ToolDef]) -> Result<ChatResponse, LlmError>;

    /// Streamed version (tokens sent via channel for early TTS playback).
    async fn chat_stream(&self, messages: &[ChatMessage], tools: &[ToolDef]) -> Result<StreamReceiver, LlmError>;
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("LLM API error: {0}")]
    ApiError(String),
    #[error("rate limited")]
    RateLimited,
    #[error("context length exceeded")]
    ContextLengthExceeded,
}
