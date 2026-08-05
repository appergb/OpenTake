// Prevents an extra console window on Windows in release. DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if opentake_tauri_lib::run_safe_asset_helper_if_requested() {
        return;
    }
    opentake_tauri_lib::run();
}
