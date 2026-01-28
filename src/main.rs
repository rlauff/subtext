
extern crate ropey;

use std::fs::File;
use std::io;
use std::collections::HashMap;

use regex::Regex;
use ropey::{iter::Chars, Rope, RopeSlice};

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
    verbose: bool,                                  // whether to print out debug info
    logging: bool,                                  // whether to log to a log file
}


fn main() {
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
        verbose: false,
        logging: false,

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
enum FindingScopeError {
    FoundEndingBraceBeforeStartingBrace(String),
    NoEndingBrace(String),
    MalformedOrMissingInput(String),
    MalformedOrMissingPattern(String),
    MalformedOrMissingOutput(String),
    ArmsNotSeparatedBySemicolon(String),
}

/// searches the given rope starting at start_idx to find the next matching {} pair or function call
fn find_next_scope(i: &Interpreter,r: &Rope, start_idx: usize) -> Result<Option<Scope>, FindingScopeError> {
    for c in r.chars_at(start_idx) {
        match c {
            '{' => {    // found the start of a scope
                let mut brace_count = 1;
                let mut end_idx = start_idx + 1;
                for c2 in r.chars_at(end_idx) {
                    match c2 {
                        '{' => brace_count += 1,
                        '}' => {
                            brace_count -= 1;
                            if brace_count == 0 {   // found the end of the scope
                                // build the scope object
                                // TODO
                            }
                        }
                        _ => (),   // continue searching
                        }
                        end_idx += 1;
                    }
                    // if this for loop ends, we have not found a closing brace matching the opening one
                    match i.verbose {
                        true => return Err(FindingScopeError::NoEndingBrace(
                            format!("No matching closing brace found when searching for a scope in {}", 
                                r.chars_at(start_idx).as_str()))),
                        false => return Err(FindingScopeError::NoEndingBrace(
                            "No matching closing brace found when searching for a scope."
                            .to_string())),
                    }
                }
            '}' => {    // found a closing brace before an opening one
                match i.verbose {
                    true => return Err(FindingScopeError::FoundEndingBraceBeforeStartingBrace(
                        format!("Encountered a closing brace without a matching opening brace when searching for a scope in {}", 
                            r.chars_at(start_idx).as_str()))),
                    false => return Err(FindingScopeError::FoundEndingBraceBeforeStartingBrace(
                        "Encountered a closing brace without a matching opening brace when searching for a scope."
                        .to_string())),
                }
            }
            _ => (),    // continue searching
        }
    }
    Ok(None)    // did not find any opening or closing braces -> there are no more scopes
}

fn evaluate_scope() {
    unimplemented!()
}




