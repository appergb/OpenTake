//! System-prompt layer (`agent-SPEC.md` §6.5): the layered base prompt (ported
//! from upstream `AgentInstructions`, product name adjusted, contract-critical
//! sentences kept verbatim) and the assembly step that appends the active
//! plugin's `instructions.md` + track roles + workflow rules.

pub mod assemble;
pub mod base;
