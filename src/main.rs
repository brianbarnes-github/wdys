#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod run;

fn main() {
    #[cfg(windows)]
    run::main();
    #[cfg(not(windows))]
    eprintln!("wdys only runs on Windows");
}
