use std::env;
use std::fs;

use subtext::{
    error::{ErrorKind, SubtextError},
    run_code_logic,
};

fn main() {
    let file_path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Error: No file path provided.");
            eprintln!("Usage: cargo run -- <file_path>");
            return;
        }
    };

    // String aus der übergebenen Datei lesen
    let input_string = match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(err) => {
            let io_error = SubtextError::new(ErrorKind::FileReadError {
                path: file_path,
                reason: err.to_string(),
            });
            eprintln!("{}", io_error);
            return;
        }
    };

    // Rufe die zentrale Ausführungslogik aus der lib.rs auf
    if let Err(err) = run_code_logic(input_string) {
        eprintln!("{}", err);
    }
}
