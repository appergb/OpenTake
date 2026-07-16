//! Suppress panic payloads only while an LLM-facing dispatch worker is active.
//! The installed hook delegates unchanged for every other thread. A panic is
//! still surfaced as a `JoinError`; MCP/chat map that error to a fixed response
//! and emit structured flags after the worker joins.

use std::cell::Cell;
use std::sync::Once;

static INSTALL_REDACTING_HOOK: Once = Once::new();

thread_local! {
    static REDACT_PANIC_PAYLOAD: Cell<bool> = const { Cell::new(false) };
}

struct RedactionGuard {
    previous: bool,
}

impl Drop for RedactionGuard {
    fn drop(&mut self) {
        REDACT_PANIC_PAYLOAD.set(self.previous);
    }
}

fn install_redacting_hook() {
    INSTALL_REDACTING_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let redact = REDACT_PANIC_PAYLOAD.with(Cell::get);
            if !redact {
                previous(info);
            }
        }));
    });
}

pub(crate) fn with_redacted_dispatch_panic<T>(task: impl FnOnce() -> T) -> T {
    install_redacting_hook();
    let previous = REDACT_PANIC_PAYLOAD.replace(true);
    let _guard = RedactionGuard { previous };
    task()
}
