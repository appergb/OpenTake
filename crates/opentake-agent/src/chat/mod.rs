//! In-app chat client (`agent-SPEC.md` §5). The UI / external MCP / in-app
//! chat are peer clients of the same [`crate::mcp::dispatch::Dispatcher`]; this
//! layer adds the conversational loop (LLM streaming + tool-call round-trips)
//! on top of that shared pipeline, so a "tighten the silences" message lands on
//! the exact same `tighten_silences` → `ripple_delete_ranges` path an external
//! agent would use.
//!
//! Modules:
//! - [`session`]: the conversation model (`ChatSession` / `ChatMessage` /
//!   `ToolCall`), serde camelCase for the front-end mirror.
//! - [`llm`]: BYOK streaming client (OpenAI + Anthropic) over the existing
//!   key-store boundary.
//! - [`loop`]: the turn loop — assemble system prompt + Context Signal, stream
//!   the model, dispatch tool calls through the shared `Dispatcher`, re-feed
//!   results until the model produces a final text turn.

pub mod llm;
pub mod r#loop;
pub mod session;
pub mod store;

pub use llm::{
    no_key_guide, provider_from_choice, stream_chat, ChatRequest, LlmError, LlmProvider, ToolSchema,
};
pub use r#loop::{ChatLoop, ChatTurnGate, EmitLoop, LoopError, LoopEvent};
pub use session::{AgentContentBlock, ChatMessage, ChatSession, Role, ToolCall};
pub use store::{ChatSessionStore, ChatSessionStoreError};
