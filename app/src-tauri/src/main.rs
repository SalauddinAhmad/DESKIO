// A console window must not appear behind the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    bhuninstaller_lib::run()
}
