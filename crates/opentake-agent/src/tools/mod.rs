//! Tool layer (= upstream `ToolExecutor`): names, descriptions/schemas, strongly
//! typed args + the LLM-facing precise-path error engine, the short-id system,
//! the neutral result type, and the `get_timeline` compact encoder
//! (`agent-SPEC.md` §2-4, §8.3). Transport-free so every piece is unit-testable
//! offline; the rmcp conversion lives in `mcp::server` (not yet landed).

pub mod args;
pub mod descriptions;
pub mod encode_timeline;
pub mod errors;
pub mod names;
pub mod result;
pub mod short_id;
