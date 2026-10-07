// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `parla search …` and friends run and exit without opening the app.
    if let Some(code) = parla_lib::cli::run_from_args() {
        std::process::exit(code);
    }
    parla_lib::run()
}
