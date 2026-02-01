
extern crate ropey;

use std::fs::File;
use std::io;
use std::collections::HashMap;

use log::{info, warn, error};
use simplelog::*;

use thiserror::Error;

use regex::Regex;
use ropey::{iter::Chars, Rope, RopeSlice};

mod errors;
use errors::FindingScopeError;

struct Function {
    name: String,           // the name of the function
    patterns: Vec<Regex>,   // the patterns of the arms to match agains, note that the regexes are precompiled
    outputs: Vec<Regex>,    // the outputs of the arms
}
struct Interpreter {
    state: Rope,
    function_table: HashMap<String, Function> ,     // the function table linking names to the Function object
    registers: Vec<Vec<String>>,                    // the registers
    idx: usize,                                     // the index to theplace the interpreter is reading atm
}


fn main() {
    // set up logging 
    let log = "file"; // Derived from args later

    if log == "file" {
        // Write to a file
        WriteLogger::init(
            LevelFilter::Info,
            Config::default(),
            File::create("my_rust_project.log").unwrap(),
        ).unwrap();
    } else if log == "terminal" {
        // Write to stdout/terminal
        TermLogger::init(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ).unwrap();
    } // else no logging

    // create the interpreter
    let filepath = std::env::args().nth(1).expect("Usage: pass path to a file");
    let mut i = Interpreter{ 
        state: Rope::from_reader(
                io::BufReader::new(
                File::open(&filepath)
                .unwrap()))
                .expect("Cannot read file: either it doesn't exist, file permissions don't allow reading, or is not utf8 text."),
        function_table: HashMap::new(),
        registers: vec![vec![]],
        idx: 0,

     };

    let mut change_made = false;
    loop {
        change_made = false;
        // parse to find the next scope

        // evaluate it

        // repeat
        if !change_made { break }
    }
    println!("\n\nFinal state:\n{}", i.state);
}

struct Scope<'a> {
    start_idx: usize,
    end_idx: usize,
    input: RopeSlice<'a>,
    patterns: Vec<Regex>,
    outputs: Vec<Regex>,
}

/// searches the given rope starting at start_idx to find the next matching {} pair or function call
fn find_next_scope<'a>(i: &'a Interpreter,r: &Rope, start_idx: usize) -> Result<Option<Scope<'a>>, FindingScopeError> {
    info!("Starting to search for next scope, starting at index {}", start_idx);
    for (start, c) in r.chars_at(start_idx).enumerate() {
        match c {
            '{' => {    // found the start of a scope
                let mut brace_count = 1;
                let mut end = start + start_idx + 1;
                for c2 in r.chars_at(end) {
                    match c2 {
                        '{' => brace_count += 1,
                        '}' => {
                            brace_count -= 1;
                            if brace_count == 0 {   // found the end of the scope
                                info!("Found matching closing brace at index {}", end);
                                // build the scope object, it is between start + start_idx and end
                                // extract the input
                                // the input starts after the opening brace and goes up to the first :
                                // a single whitespace is ignored after the brace and before the colon
                                
                                // TODO
                            }
                        }
                        _ => (),   // continue searching
                        }
                        end += 1;
                    }
                    // if this for loop ends, we have not found a closing brace matching the opening one
                    return Err(FindingScopeError::NoEndingBrace)
                }
            '}' => {    // found a closing brace before an opening one
                return Err(FindingScopeError::FoundEndingBraceBeforeStartingBrace)
            }
            _ => (),    // continue searching
        }
    }
    Ok(None)    // did not find any opening or closing braces -> there are no more scopes
}

fn evaluate_scope(scope: &Scope, interpreter: &mut Interpreter) {
    info!("Evaluate scope {}", interpreter.state.slice(scope.start_idx..scope.end_idx));
    unimplemented!()
}




