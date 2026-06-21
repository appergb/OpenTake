//! `CoreEvent` + `EventBus` — the one-way notification channel from the Rust
//! core to its observers (the Tauri bridge, autosave, telemetry).
//!
//! Upstream propagates state changes for free via SwiftUI `@Observable`
//! (`EditorViewModel` is `@Observable`). OpenTake crosses a logical process
//! boundary (core in Rust, UI in a WebView), so the change signal must be made
//! explicit: every committing edit emits [`CoreEvent::TimelineChanged`] carrying
//! the new monotonic `version`, and `src-tauri` forwards it to the front end as
//! a `timeline_changed` event so the read-only mirror can re-fetch
//! (`core-SPEC.md` §3, §4).
//!
//! ## Why a plain callback bus and not `tokio::broadcast`
//!
//! The spec sketch reached for `tokio::broadcast`, but the only contract the
//! core needs is "fan a value out to N observers". A `Vec` of boxed callbacks
//! behind a `Mutex` delivers exactly that with **zero runtime dependency**,
//! keeps emission synchronous and panic-free (no subscribers is a no-op), and is
//! trivially testable (a test subscriber just pushes into a shared `Vec`). The
//! Tauri bridge's callback simply calls `app_handle.emit(...)`. If a future need
//! for async multi-consumer buffering appears it can be layered on without
//! touching the [`CoreEvent`] contract.

use std::sync::{Arc, Mutex};

use serde::Serialize;

/// A state change the front end (or any observer) may need to react to.
///
/// Serialized with an internal `kind` tag so the Tauri bridge can forward it as
/// a tagged JSON payload. Only the timeline/project lifecycle events are modeled
/// here; preview/export/generation events belong to later phases and are added
/// when their backends land (`core-SPEC.md` §3.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEvent {
    /// The authoritative timeline was replaced (a committing edit, an undo, or a
    /// redo). `version` is the new monotonic document version; observers with a
    /// stale mirror re-fetch via `get_timeline` (`core-SPEC.md` §2.3 step 5,
    /// §4.1 rule 3).
    TimelineChanged {
        /// The document version after the change. Strictly increasing.
        version: u64,
    },

    /// A project bundle was opened (or a fresh one created). Carries the bundle
    /// path (empty for an unsaved `new_project`) and the version the document
    /// starts at (always 0). The front end fetches the first snapshot itself, so
    /// `open` does not emit `TimelineChanged` (`core-SPEC.md` §5.4 step 6).
    ProjectOpened {
        /// Absolute bundle path, or empty string for an unsaved new project.
        path: String,
        /// The document version right after open (0).
        version: u64,
    },

    /// A project bundle was written to disk. Carries the path that was saved.
    ProjectSaved {
        /// The bundle path that was written.
        path: String,
    },
}

/// An opaque handle for a registered subscriber. Pass it to
/// [`EventBus::unsubscribe`] to stop receiving events.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct SubscriptionId(u64);

type Listener = Box<dyn Fn(&CoreEvent) + Send + 'static>;

struct Inner {
    next_id: u64,
    listeners: Vec<(SubscriptionId, Listener)>,
}

/// A cloneable, thread-safe fan-out of [`CoreEvent`]s to registered callbacks.
///
/// Clones share one subscriber list (it is `Arc`-backed), matching the way every
/// `AppCore` clone observes the same stream. Emission is synchronous: callbacks
/// run on the emitting thread, in registration order.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Mutex<Inner>>,
}

impl Default for EventBus {
    fn default() -> Self {
        EventBus::new()
    }
}

impl EventBus {
    /// A bus with no subscribers.
    pub fn new() -> Self {
        EventBus {
            inner: Arc::new(Mutex::new(Inner {
                next_id: 0,
                listeners: Vec::new(),
            })),
        }
    }

    /// Register `listener`; returns a [`SubscriptionId`] for later removal.
    ///
    /// The listener must be `Send` so the bus stays usable across threads (the
    /// core runs commands under a `Mutex` that may be touched from any thread).
    pub fn subscribe(&self, listener: impl Fn(&CoreEvent) + Send + 'static) -> SubscriptionId {
        let mut inner = self.inner.lock().expect("event bus mutex poisoned");
        let id = SubscriptionId(inner.next_id);
        inner.next_id += 1;
        inner.listeners.push((id, Box::new(listener)));
        id
    }

    /// Remove a previously registered subscriber. Unknown ids are ignored.
    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut inner = self.inner.lock().expect("event bus mutex poisoned");
        inner.listeners.retain(|(existing, _)| *existing != id);
    }

    /// Deliver `event` to every current subscriber, in registration order.
    /// A no-op (never panics) when there are no subscribers.
    pub fn emit(&self, event: &CoreEvent) {
        let inner = self.inner.lock().expect("event bus mutex poisoned");
        for (_, listener) in &inner.listeners {
            listener(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_with_no_subscribers_is_noop() {
        let bus = EventBus::new();
        bus.emit(&CoreEvent::TimelineChanged { version: 1 });
    }

    #[test]
    fn subscriber_receives_events_in_order() {
        let bus = EventBus::new();
        let seen: Arc<Mutex<Vec<CoreEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        bus.subscribe(move |ev| sink.lock().unwrap().push(ev.clone()));

        bus.emit(&CoreEvent::TimelineChanged { version: 1 });
        bus.emit(&CoreEvent::TimelineChanged { version: 2 });

        let got = seen.lock().unwrap().clone();
        assert_eq!(
            got,
            vec![
                CoreEvent::TimelineChanged { version: 1 },
                CoreEvent::TimelineChanged { version: 2 },
            ]
        );
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let sink = Arc::clone(&count);
        let id = bus.subscribe(move |_| *sink.lock().unwrap() += 1);

        bus.emit(&CoreEvent::ProjectSaved { path: "p".into() });
        bus.unsubscribe(id);
        bus.emit(&CoreEvent::ProjectSaved { path: "p".into() });

        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn core_event_serializes_with_kind_tag() {
        let json = serde_json::to_string(&CoreEvent::TimelineChanged { version: 7 }).unwrap();
        assert_eq!(json, r#"{"kind":"timeline_changed","version":7}"#);
    }
}
