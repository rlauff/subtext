use ropey::Rope;


struct Interpreter {
    state: Rope,
    registers: Vec<Vec<String>>,    // the registers
    idx: usize,     // the index to theplace the interpreter is reading atm
}


fn main() {
    // create the interpreter

    loop {
        // parse to find the next scope

        // evaluate it
    }
}

fn find_next_scope() {
    unimplemented!()
}

fn evaluate_scope() {
    unimplemented!()
}




