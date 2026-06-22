//! In-process MCP execution shell (`agent-SPEC.md` §8): the [`CoreHandle`] bridge
//! to `opentake-core` and the uniform [`Dispatcher`] that wraps every tool.
//!
//! This is the transport-free core of the MCP server: one pipeline that resolves
//! a tool name, snapshots the document, expands inbound short-id prefixes,
//! decodes typed args with precise-path errors, runs the tool body (editing tools
//! build an [`opentake_ops::command::EditCommand`] and apply it through the
//! handle; read tools serialize state), attaches a `context_signal` block, and
//! shortens outbound ids. The rmcp server / HTTP handler is a thin shim over this
//! and lands in a later phase.

pub mod convert;
pub mod core_handle;
pub mod dispatch;
pub mod server;
