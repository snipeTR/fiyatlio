// src-tauri/src/main.rs
//
// Thin launcher. Everything meaningful lives in the library crate so tests can
// reach it. The `windows_subsystem` attribute suppresses the console window in
// Windows release builds while keeping stdout available during development.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fiyatlio_lib::run();
}
