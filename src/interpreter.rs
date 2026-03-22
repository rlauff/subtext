use std::thread::Scope;

use crate::linkes_chars::LinkedChars;

enum Task {
    Scope {
        content: String,
    },
    FunctionCall {
        function_name: String,
        input: String,
    },
    DefineFunction {
        name: String,
        definition: String,
    },
    RegisterCall {
        level: usize,
        index: usize,
    },
    GetInput {
        prompt: String,
    },
    PrintOutput {
        content: String,
    },
    Chill, // nothing else to do, the interpreter can return
}

struct Job {
    start: usize, // the star index of the stuff to be replaced
    end: usize,   // end index
    task: Task,
}

fn get_new_job(linked_chars: &LinkedChars, reader_idx: usize) -> Job {
    let mut chars_buffer = Vec::new(); // holds the read chars
    for (i, node) in linked_chars.enumerate_with_start(reader_idx) {
        match node.c {
            '(' => {
                // this is a function call. Find the closing brace
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Round);
                let full_string = linked_chars.interval_to_string(i, closing_brace_idx);
                let task = match chars_buffer.iter().collect::<String>().as_str() {
                    "get_input" => Task::GetInput {
                        prompt: full_string,
                    },
                    "print_output" => Task::PrintOutput {
                        content: full_string,
                    },
                    other_name => Task::FunctionCall {
                        function_name: other_name.to_string(),
                        input: full_string,
                    },
                };
                return Job {
                    start: i,
                    end: closing_brace_idx,
                    task,
                };
            }
            '{' => {
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Curly);
                let full_string = linked_chars.interval_to_string(i, closing_brace_idx);
                return Job {
                    start: i,
                    end: closing_brace_idx,
                    task: Task::Scope {
                        content: full_string,
                    },
                };
            }
            ' ' => {
                if chars_buffer.iter().collect::<String>().as_str() == "def" {
                    // find function name and return the job
                } else {
                    // its not a def, just delete the buffer and read on
                    chars_buffer.clear();
                }
            }
            c => chars_buffer.push(c),
        }
    }
    unimplemented!()
}

#[derive(PartialEq)]
enum Brace {
    Curly,
    Round,
}

// returns the index to the node containing the closing brace
// panics if there is no closing brace
fn find_closing_brace(linked_chars: &LinkedChars, opening_brace_idx: usize, brace: Brace) -> usize {
    let mut number_opened = 1;
    for (idx, node) in linked_chars.enumerate_with_start(opening_brace_idx) {
        match node.c {
            '{' => {
                if brace == Brace::Curly {
                    number_opened += 1
                }
            }
            '(' => {
                if brace == Brace::Round {
                    number_opened += 1
                }
            }
            '}' => {
                if brace == Brace::Curly {
                    number_opened -= 1
                }
            }
            ')' => {
                if brace == Brace::Round {
                    number_opened -= 1
                }
            }
            _ => (),
        }
        if number_opened == 0 {
            return idx;
        }
    }
    panic!("No closing brace found"); // TODO: add proper error handling
}

// an Interpreter gets passed a LinkedChars and is tasked to evaluate it until there are no further
// changes
// It will save regex matches into its own registers
// its children may use the contents of these registers by using the ^ operator on register calls
//
struct Interpreter<'a> {
    state: LinkedChars,
    parent: &'a Interpreter<'a>, //
    registers: Vec<String>,
}
