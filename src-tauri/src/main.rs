// Prevents an extra console window on Windows in release. DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Phase 0 scaffold. The real shell will:
//   - hold an opentake_core::EditorState
//   - expose #[tauri::command] thin wrappers (edit_apply / project_open|save /
//     undo|redo / get_timeline / seek / import_media / export_start)
//   - emit events (timeline_changed{version} / preview_frame / progress)
// See ../README.md and docs/ARCHITECTURE.md §2.

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
