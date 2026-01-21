
extern crate ropey;

use std::fs::File;
use std::io;

use ropey::{iter::Chars, Rope, RopeSlice};


struct Interpreter {
    state: Rope,
    registers: Vec<Vec<String>>,    // the registers
    idx: usize,     // the index to theplace the interpreter is reading atm
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

fn find_next_scope() {
    unimplemented!()
}

fn evaluate_scope() {
    unimplemented!()
}




