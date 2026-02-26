mod bearing;
mod cli;
mod model;
#[allow(dead_code)]
mod observe;
mod storage;
mod tui;

use std::{env, process};

use storage::Storage;

fn main() {
    let root = Storage::default_root().unwrap_or_else(|| {
        eprintln!("Could not determine home directory");
        process::exit(1);
    });

    let storage = match Storage::new(root) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to initialize storage: {e}");
            process::exit(1);
        }
    };

    // If invoked with a subcommand, run the CLI. Otherwise, launch the TUI.
    let has_args = env::args().count() > 1;

    if has_args {
        if let Err(e) = cli::run(&storage) {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    } else if let Err(e) = tui::run(&storage) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
