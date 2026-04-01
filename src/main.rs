use crate::{interpreter::Interpreter, linked_chars::LinkedChars};

use std::env;
use std::fs;
use std::io::{self, Write};

pub mod linked_chars;

pub mod interpreter;

pub mod scope;

pub mod error;

fn parse_args() -> Option<(String, bool)> {
    let mut step_mode = false;
    let mut file_path: Option<String> = None;

    for arg in env::args().skip(1) {
        if arg == "--step" {
            step_mode = true;
        } else if file_path.is_none() {
            file_path = Some(arg);
        }
    }

    file_path.map(|path| (path, step_mode))
}

fn wait_for_key() -> io::Result<()> {
    print!("Press Enter to continue...");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(())
}

fn main() {
    let (file_path, step_mode) = match parse_args() {
        Some(args) => args,
        None => {
            eprintln!("Error: No file path provided.");
            eprintln!("Usage: cargo run -- <file_path> [--step]");
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
    match root_interpreter.evaluate() {
        Ok(history) => {
            if step_mode {
                if history.is_empty() {
                    println!("{}", root_interpreter.state.make_string());
                    return;
                }
                for (idx, state) in history.iter().enumerate() {
                    if idx > 0
                        && let Err(err) = wait_for_key()
                    {
                        eprintln!("Failed to read input: {}", err);
                        break;
                    }
                    println!("{}", state.make_string());
                }
            }
        }
        Err(err) => eprintln!("{}", err),
    }
}
