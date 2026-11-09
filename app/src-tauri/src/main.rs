//! The atm app's entry point; everything is in the library, so tests reach it.

// No console window next to the app's on Windows, in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    atm_app::run();
}
