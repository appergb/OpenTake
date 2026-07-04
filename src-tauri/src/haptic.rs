//! Trackpad haptic feedback for timeline snaps — the light "alignment" tick
//! upstream fires (`NSHapticFeedbackManager.perform(.alignment)`) whenever a
//! clip edge / playhead snaps into place. macOS only; off other platforms the
//! front end plays a short tick sound instead, so the command stays callable
//! everywhere.

/// `snap_haptic`: perform one light alignment haptic on the trackpad. The AppKit
/// feedback call is dispatched to the main thread (AppKit work belongs there)
/// and is best-effort — any failure is swallowed so a snap never errors the UI.
#[cfg(target_os = "macos")]
#[tauri::command]
pub fn snap_haptic(app: tauri::AppHandle) {
    let _ = app.run_on_main_thread(|| {
        use objc2_app_kit::{
            NSHapticFeedbackManager, NSHapticFeedbackPattern, NSHapticFeedbackPerformanceTime,
            NSHapticFeedbackPerformer,
        };
        NSHapticFeedbackManager::defaultPerformer().performFeedbackPattern_performanceTime(
            NSHapticFeedbackPattern::Alignment,
            NSHapticFeedbackPerformanceTime::Now,
        );
    });
}

/// Non-macOS: trackpad haptics aren't available; the front end plays a short
/// tick sound instead. Kept as a no-op command so the call site is uniform.
#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn snap_haptic() {}
