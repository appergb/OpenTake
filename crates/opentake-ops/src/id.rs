//! Id generation for newly created entities (clips, tracks, link groups,
//! folders).
//!
//! Upstream mints `UUID().uuidString` inline (split right-halves, placed clips,
//! linked-audio partners, link groups, new tracks, folders). This crate stays a
//! zero-business-dependency leaf — no `uuid` — so id creation is injected via the
//! [`IdGen`] trait. The owning layer (project/core, which already depends on
//! `uuid`) supplies a UUID-backed generator in production; tests use the
//! deterministic [`SeqIdGen`] so split/link/place ids are assertable.

use std::cell::Cell;

/// Mints fresh, unique string ids on demand.
pub trait IdGen {
    /// A brand-new id, unique within this generator's lifetime.
    fn next_id(&self) -> String;
}

/// Deterministic, monotonic id generator: `"{prefix}{n}"` with `n` starting at
/// `1`. Interior-mutable so it threads through `&self` editing calls. Default
/// prefix is `"id-"`.
#[derive(Debug)]
pub struct SeqIdGen {
    prefix: String,
    counter: Cell<u64>,
}

impl SeqIdGen {
    /// New generator counting from 1 with the given id prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        SeqIdGen {
            prefix: prefix.into(),
            counter: Cell::new(0),
        }
    }

    /// How many ids have been minted so far (useful for assertions).
    pub fn count(&self) -> u64 {
        self.counter.get()
    }
}

impl Default for SeqIdGen {
    fn default() -> Self {
        SeqIdGen::new("id-")
    }
}

impl IdGen for SeqIdGen {
    fn next_id(&self) -> String {
        let n = self.counter.get() + 1;
        self.counter.set(n);
        format!("{}{}", self.prefix, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_id_gen_is_monotonic_from_one() {
        let g = SeqIdGen::new("c-");
        assert_eq!(g.next_id(), "c-1");
        assert_eq!(g.next_id(), "c-2");
        assert_eq!(g.next_id(), "c-3");
        assert_eq!(g.count(), 3);
    }

    #[test]
    fn default_prefix_is_id_dash() {
        let g = SeqIdGen::default();
        assert_eq!(g.next_id(), "id-1");
    }
}
