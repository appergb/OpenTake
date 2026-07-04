//! opentake-agent — single capability layer, multiple front-ends.
//!
//! Tool layer (= upstream ToolExecutor) + MCP server (rmcp) + in-app chat client
//! + short-id system + system prompts. UI / in-app agent / external MCP are peer clients.
//!
//! Modules landed so far (`agent-SPEC.md` §9.1):
//! - [`tools`]: tool names, descriptions/schemas, args + precise-path errors,
//!   short-id system, neutral result type, `get_timeline` compact encoder.
//! - [`signal`]: Context Signal generation + attachment (§6).
//! - [`plugin`]: Workflow Plugin model/registry/rules (§7).
//! - [`prompt`]: layered base system prompt + assembly (§6.5).
//! - [`mcp`]: the `CoreHandle` bridge to `opentake-core` + the uniform
//!   tool-dispatch shell that wraps every tool (§8).
//!
//! The rmcp MCP server transport (§1) and the in-app chat client (§5) layer over
//! the [`mcp`] dispatch shell and land in subsequent phases.

pub mod mcp;
pub mod plugin;
pub mod prompt;
pub mod signal;
pub mod tools;
