#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    let _guard = chuwitchwindow::logger::setup_logger();
    chuwitchwindow::run();
}
