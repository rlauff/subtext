use crate::linked_tokens::*;

pub struct Parser {
    pub state: ParseState,
    pub buffer: Vec<char>,      // the buffer holding a potential function name or def statement etc
    pub depth: usize,           // the current depth of ra potential register call
    pub index: Vec<char>,       // the index chars of a register call
    pub global: bool,           // wether the defined function is marked as global
}

pub enum ParseState {
    // whenever we see a non-whitespace character, there might be a function call
    // if we are in a string that looks like 'def', we might be defining a function
    // keep what came since the last whitespace in a buffer
    // if the next character is a '(', it is a function call
    // if we complete 'def' and then see a whitespace, it is a function definition
    // if we see a whitespace, we reset the buffer
    Normal,  
    // if we see a '^', we might be calling a register
    // if the next chacacters are also '^', we increase the depth of the register call
    // if we encounter a '$', we start the register call
    PotRegisterCall,
    // If we find a '$' we are calling a register
    // if we came from PotRegisterCall, we take the depth for the register call from there
    // else the depth is 0
    // we expect to see digits after the '$' to indicate the index of the register
    // the string of digits is terminated by a whitespace
    InRegisterCallParseIndex,
    // we have encountered a 'def' and a whitespace after it, now we expect a function name
    ParsingDefFunctionName,
    // if we find a '/', we might be starting a comment
    PotComment,
    // we are in a comment, ignore everything until the end of the line
    InComment,
    Escape,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            state: ParseState::Normal,
            buffer: Vec::new(),
            depth: 0,
            index: Vec::new(),
            global: false,
        }
    }

    pub fn reset_buffers(&mut self) {
        self.buffer.clear();
        self.depth = 0;
        self.index.clear();
        self.global = false;
    }
}