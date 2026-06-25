//! Workflow-plugin layer (`agent-SPEC.md` §7): the `plugin.json` model, the
//! registry that loads/validates/activates plugins from disk, and the plugin
//! `workflow.rules` validation that layers on top of the built-in rules. Pure
//! Agent-layer state — no `opentake-core` editing logic is touched.

pub mod model;
pub mod registry;
pub mod rules;
