
extern crate ropey;

use std::fs::File;
use std::io;
use std::collections::HashMap;

use log::{info, warn, error};
use simplelog::*;

use thiserror::Error;

use regex::Regex;

mod errors;
use errors::FindingScopeError;

mod linked_tokens;

mod parser;

mod eval;

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

    let s = "def add_positive {
    (\\d*)(\\d)\\+(\\d*)(\\d)&?(c?)&?(\\d*) // Aa+Bb&c&R, happy path where neither summand is empty
    :   add_positive{$1 +$3 &{ sum_two_digits{^$2 ^$4 ^$5 } : (\\d)(c?) : $2 &^$6 $1 }} ; // call sum_two_digits(abc) and place result in $1: the digit and $2: potential carry, then build the result string
    :   (:?\\+(\\d*)|(\\d*)\\x)&?(c?)&?(\\d*) // one summand is empty
    :   { ^$2   : c : add_positive{^$1 +1&&^$3 } ;   // there is still a carry, replace the empty summand by 1
    :   : $1 $3 }   ;                   // no more carry, can just write the result
    // if none of the arms match we have an error, we want the scope insider the input of the error to be parsed, so we cannot write error(...) directly
    // this is why we are using the dirty approach of letting the error keyword be generated itself by a scope { : : error}
    // this might be changed to a more elegant
    : : { : : error}(Something went wrong when calling add_positive on input $0 ) ;
}
";
    let lt = linked_tokens::LinkedTokens::from_string(s).unwrap();
    println!("{}", lt);
    println!("{}", lt.to_raw_string());
}


