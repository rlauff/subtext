use crate::{interpreter::Interpreter, linked_chars::LinkedChars};

use std::env;
use std::fs;

pub mod linked_chars;

pub mod interpreter;

pub mod scope;

pub mod error;

fn main() {
    let file_path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Error: No file path provided.");
            eprintln!("Usage: cargo run -- <file_path>");
            return;
        }
    };
    // Read String from the passed file
    let input_string = match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(err) => {
            let io_error =
                crate::error::SubtextError::new(crate::error::ErrorKind::FileReadError {
                    path: file_path,
                    reason: err.to_string(),
                });
            eprintln!("{}", io_error);
            return;
        }
    };
    // create the root interpreter
    let mut root_interpreter = Interpreter {
        state: LinkedChars::from_iter(input_string.chars()),
        registers: vec![],
        functions: vec![],
        parent: None,
        history: None,
    };
    // evaluate it
    if let Err(err) = root_interpreter.evaluate() {
        eprintln!("{}", err);
    }
}
