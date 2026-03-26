use crate::{interpreter::Interpreter, linked_chars::LinkedChars};

use std::env;
use std::fs;

pub mod linked_chars;

pub mod interpreter;

pub mod scope;

fn main() {
    let file_path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Error: No file path provided.");
            eprintln!("Usage: cargo run -- <file_path>");
            return;
        }
    };
    // read String from the passed file TODO do propper error handling
    let input_string = fs::read_to_string(file_path).expect("Failed to read file");
    // create the root interpreter
    let mut root_interpreter = Interpreter {
        state: LinkedChars::from_iter(input_string.chars()),
        registers: vec![],
        functions: vec![],
        parent: None,
    };
    // evaluate it
    root_interpreter.evaluate();
}
