use crate::linked_chars::LinkedChars;

use crate::scope::evaluate_scope;

use std::io::{self, Write};

// An Interpreter gets passed a LinkedChars and is tasked to evaluate it until there are no further changes.
// It will save regex matches into its own registers.
// Its children may use the contents of these registers by using the ^ operator on register calls.
pub struct Interpreter<'a> {
    pub state: LinkedChars,

    // Example: { ab : (.)(.) : { ^$2 ^$1 : ba : it was ab; : it was not ab} }
    //          ^parent start   ^child start                                ^both end
    pub parent: Option<&'a Interpreter<'a>>,
    pub registers: Vec<String>,
    pub functions: Vec<Function>,
}

// Helper to easily switch parsing logic between round and curly braces.
#[derive(PartialEq)]
enum Brace {
    Round,
    Curly,
}

impl Brace {
    fn opening(&self) -> char {
        match self {
            Brace::Round => '(',
            Brace::Curly => '{',
        }
    }
    fn closing(&self) -> char {
        match self {
            Brace::Round => ')',
            Brace::Curly => '}',
        }
    }
}

#[derive(Debug, PartialEq)]
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
    Chill, // Nothing else to do, the interpreter can return
}

#[derive(Debug, PartialEq)]
struct Job {
    start: usize, // The start index (points to the node BEFORE the stuff to be replaced)
    end: usize,   // The end index (points to the exact last node of the stuff to be replaced)
    task: Task,
}

// Finds the matching closing brace.
// Expects start_idx to be the exact index of the opening brace.
fn find_closing_brace(linked_chars: &LinkedChars, start_idx: usize, brace_type: Brace) -> usize {
    let mut number_opened = 1; // The brace at start_idx is already open
    let opening_char = brace_type.opening();
    let closing_char = brace_type.closing();

    for (i, node) in linked_chars.enumerate_with_start(start_idx) {
        if node.c == opening_char {
            number_opened += 1;
        } else if node.c == closing_char {
            number_opened -= 1;
        }

        // When the counter drops back to 0, we found the matching partner
        if number_opened == 0 {
            return i;
        }
    }
    panic!("Matching closing brace not found");
}

// returns the register number and the index to the last digit
// any non digit char can terminate the register call
// (register number, idx_to_last_digit)
fn find_register_number(linked_chars: &LinkedChars, start_idx: usize) -> (usize, usize) {
    let mut register_number = 0;
    let mut last_found_digit_idx = 0;
    for (i, node) in linked_chars.enumerate_with_start(start_idx) {
        match node.c {
            '0'..='9' => {
                let new_digit = node.c.to_digit(10).unwrap() as usize;
                register_number = 10 * register_number + new_digit;
                last_found_digit_idx = i;
            }

            _ => {
                break;
            }
        }
    }
    if last_found_digit_idx == 0 {
        panic!("no digit found after register call")
    };
    (register_number, last_found_digit_idx)
}

// Scans for a function name after a 'def' keyword.
// Returns: (Extracted Name, Index of the node BEFORE the '{', Index of the '{')
fn find_function_name(linked_chars: &LinkedChars, start_idx: usize) -> (String, usize, usize) {
    let mut chars_buffer = Vec::new();
    let mut prev_idx = start_idx;

    for (i, node) in linked_chars.enumerate_with_start(start_idx) {
        match node.c {
            '{' => {
                // The function name ended, we just found the start of the scope
                if chars_buffer.is_empty() {
                    panic!("no function name provided, def must be followed by function name");
                }
                return (chars_buffer.into_iter().collect(), prev_idx, i);
            }
            ' ' => (), // Ignore whitespace to allow for formatting

            // TODO: check for illegal chars in function name here
            c => chars_buffer.push(c),
        }
        prev_idx = i;
    }
    panic!("Never found the scope with the actual function def");
}

// returns the next job to do
// start should point to the node which comes BEFORE the first relevant one
// end should point to the last relevant node
// Example: here is a scope:  _{ foo : bar : no match }
//                            ^start                  ^end
// start points to the _, end point to the }
fn get_new_job(linked_chars: &LinkedChars, reader_idx: usize) -> Job {
    let mut chars_buffer = Vec::new(); // Holds the read chars

    // Index to the char preceding the oldest non whitespace char we saw.
    // We use Option<usize> because 0 is a valid index (the Dummy node), so we need
    // `None` to represent the "unset" state.
    let mut oldest_non_whitespace: Option<usize> = None;

    let mut oldest_uptick: Option<usize> = None;

    // Idx to exactly one char back. We return this so that the replacing function works correctly.
    // Note that the replace function will replace everything AFTER the index we pass, non-inclusive.
    let mut prev_idx = reader_idx;

    let mut number_consecutive_uptick = 0;

    for (i, node) in linked_chars.enumerate_with_start(reader_idx) {
        match node.c {
            '(' => {
                // Ignore stray parentheses if we haven't read a function name yet
                if chars_buffer.is_empty() {
                    chars_buffer.push(node.c);
                    if oldest_non_whitespace.is_none() {
                        oldest_non_whitespace = Some(prev_idx);
                    }
                    number_consecutive_uptick = 0;
                    continue;
                }
                // This is a function call. Find the closing brace.
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Round);

                // Use prev_idx to include the '(' itself in the extracted string
                let full_string = linked_chars.interval_to_string(prev_idx, closing_brace_idx);
                let name: String = chars_buffer.iter().collect();

                let task = match name.as_str() {
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
                    start: oldest_non_whitespace.unwrap_or(0),
                    end: closing_brace_idx,
                    task,
                };
            }

            '{' => {
                // this is a scope
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Curly);
                // Use prev_idx to include the '{' itself in the extracted string
                // TODO: In the scope evaluation, the braces are striped away anyway
                // we could just not put them in at all
                let full_string = linked_chars.interval_to_string(prev_idx, closing_brace_idx);

                return Job {
                    start: prev_idx,
                    end: closing_brace_idx,
                    task: Task::Scope {
                        content: full_string,
                    },
                };
            }

            ' ' => {
                let name: String = chars_buffer.iter().collect();
                if name == "def" {
                    let (function_name, opening_brace_prev, opening_brace_idx) =
                        find_function_name(linked_chars, i);
                    let closing_brace_idx =
                        find_closing_brace(linked_chars, opening_brace_idx, Brace::Curly);

                    // Extract everything including the braces
                    let definition_string =
                        linked_chars.interval_to_string(opening_brace_prev, closing_brace_idx);

                    return Job {
                        start: oldest_non_whitespace.unwrap_or(0),
                        end: closing_brace_idx,
                        task: Task::DefineFunction {
                            name: function_name,
                            definition: definition_string,
                        },
                    };
                } else {
                    // Reset the buffer and start marker if we hit a space and it wasn't 'def'
                    chars_buffer.clear();
                    oldest_non_whitespace = None;
                    number_consecutive_uptick = 0;
                    oldest_uptick = None;
                }
            }

            '^' => {
                number_consecutive_uptick += 1;
                if oldest_uptick.is_none() {
                    oldest_uptick = Some(prev_idx);
                }
            }

            '#' => {
                // the new char for register calls, as not to conflict with regex syntax
                // find the register which should be called
                let (register_number, idx_to_terminating_char) =
                    find_register_number(linked_chars, i);
                // println!("{}, {}", register_number, idx_to_terminating_char);

                return Job {
                    start: oldest_uptick.unwrap_or(prev_idx),
                    end: idx_to_terminating_char,
                    task: Task::RegisterCall {
                        level: number_consecutive_uptick,
                        index: register_number,
                    },
                };
            }

            c => {
                chars_buffer.push(c);
                number_consecutive_uptick = 0;
                if oldest_non_whitespace.is_none() {
                    oldest_non_whitespace = Some(prev_idx);
                }
            }
        }

        // Always step the previous index forward at the end of the iteration
        prev_idx = i;
    }

    if reader_idx != 0 {
        // We reached the end of the text but started in the middle. Loop around!
        return get_new_job(linked_chars, 0);
    }

    Job {
        task: Task::Chill,
        start: 0,
        end: 0,
    }
}

#[derive(Clone)]
pub struct Function {
    name: String,
    body: String,
}

impl Interpreter<'_> {
    pub fn evaluate(&mut self) {
        // find jobs and apply the resp. changes until we get Chill back
        // After doing a Job, put the reading head at the start of the returned job.
        // This way, we read the output of the last evaluation back in immediately (for recursion).
        let mut reading_head = 0;
        'outer: loop {
            let job = get_new_job(&self.state, reading_head);
            reading_head = job.start; // always read the replacement back in 
            match job.task {
                Task::Chill => break, // we are done

                Task::Scope { content: scope } => {
                    // evaluate the scope
                    let result = evaluate_scope(scope, self);
                    // modify the state
                    self.state.replace_between(job.start, job.end, result);
                }

                Task::RegisterCall { level, index } => {
                    let result =
                        LinkedChars::from_iter(self.get_register_at_level(level, index).chars());
                    self.state.replace_between(job.start, job.end, result);
                }

                Task::DefineFunction { name, definition } => {
                    // when looking for a function, we will look through this vector in reverse.
                    // This way a new definition will shadow a potential old one
                    self.functions.push(Function {
                        name,
                        body: definition,
                    });
                    self.state.remove_between(job.start, job.end);
                }

                Task::FunctionCall {
                    function_name,
                    input,
                } => {
                    let found_function = self
                        .functions
                        .iter()
                        .find(|func| func.name == function_name);

                    match found_function {
                        Some(function) => {
                            let clean_input = if input.starts_with('(') && input.ends_with(')') {
                                &input[1..input.len() - 1]
                            } else {
                                &input
                            };

                            let clean_body =
                                if function.body.starts_with('{') && function.body.ends_with('}') {
                                    &function.body[1..function.body.len() - 1]
                                } else {
                                    &function.body
                                };

                            let scope = format!("{{ {} :: {} }}", clean_input, clean_body);

                            let result = evaluate_scope(scope, self);
                            self.state.replace_between(job.start, job.end, result);
                        }
                        None => {
                            panic!(
                                "Called an undefined function, tried to call {}",
                                function_name
                            );
                        }
                    }
                }

                Task::GetInput { prompt } => {
                    print!("{}", prompt);
                    io::stdout().flush().unwrap();
                    let mut response = String::new();

                    io::stdin()
                        .read_line(&mut response)
                        .expect("Failed to read line");

                    let clean_response = response.trim().to_string();
                    let ls = LinkedChars::from_iter(clean_response.chars());
                    self.state.replace_between(job.start, job.end, ls);
                }

                Task::PrintOutput { content } => {
                    println!("{}", content);
                    self.state.remove_between(job.start, job.end);
                }
            }
        }
    }

    fn get_register_at_level(&self, level: usize, index: usize) -> String {
        let mut current: &Interpreter = self;
        for _ in 0..level {
            if let Some(parent_ref) = current.parent {
                current = parent_ref;
            } else {
                panic!("no parent scope found, register call is too high.");
            }
        }
        // We successfully went up `level` times. Return the register found here.
        current.registers[index].clone()
    }
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_closing_brace_flat() {
        let lc = LinkedChars::from_iter("(abc)".chars());
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round);
        assert_eq!(closing_idx, 5);
    }

    #[test]
    fn test_find_closing_brace_nested() {
        let lc = LinkedChars::from_iter("(a(b)c)".chars());
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round);
        assert_eq!(closing_idx, 7);
    }

    #[test]
    #[should_panic(expected = "Matching closing brace not found")]
    fn test_find_closing_brace_missing() {
        let lc = LinkedChars::from_iter("(abc".chars());
        find_closing_brace(&lc, 1, Brace::Round);
    }

    #[test]
    fn test_find_function_name() {
        let lc = LinkedChars::from_iter("  my_func  {".chars());
        let (name, prev_idx, brace_idx) = find_function_name(&lc, 0);
        assert_eq!(name, "my_func");
        assert_eq!(
            prev_idx, 11,
            "Must find the node exactly before the curly brace"
        );
        assert_eq!(brace_idx, 12);
    }

    #[test]
    fn test_get_new_job_function_call() {
        let lc = LinkedChars::from_iter("  foo(bar)".chars());
        let job = get_new_job(&lc, 0);

        let expected_job = Job {
            start: 2,
            end: 10,
            task: Task::FunctionCall {
                function_name: "foo".to_string(),
                input: "(bar)".to_string(),
            },
        };
        assert_eq!(job, expected_job);
    }

    #[test]
    fn test_get_new_job_built_in_functions() {
        let lc = LinkedChars::from_iter("print_output(123)".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.start, 0);
        assert_eq!(job.end, 17);
        assert_eq!(
            job.task,
            Task::PrintOutput {
                content: "(123)".to_string()
            }
        );
    }

    #[test]
    fn test_get_new_job_scope() {
        let lc = LinkedChars::from_iter("  { a }".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.start, 2);
        assert_eq!(
            job.task,
            Task::Scope {
                content: "{ a }".to_string()
            }
        );
    }

    #[test]
    fn test_get_new_job_def_function() {
        let lc = LinkedChars::from_iter("def my_func { body }".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.start, 0);
        if let Task::DefineFunction { name, definition } = job.task {
            assert_eq!(name, "my_func");
            assert_eq!(definition, "{ body }");
        } else {
            panic!("Expected DefineFunction task");
        }
    }

    #[test]
    fn test_get_new_job_chill() {
        let lc = LinkedChars::from_iter("just_some_text".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.task, Task::Chill);
    }

    #[test]
    fn test_get_new_job_loop_around() {
        let lc = LinkedChars::from_iter("  foo()".chars());
        let job = get_new_job(&lc, 5);

        assert_eq!(job.start, 2);
        if let Task::FunctionCall { function_name, .. } = job.task {
            assert_eq!(function_name, "foo");
        } else {
            panic!("Expected FunctionCall");
        }
    }

    // function call tests

    #[test]
    fn define_and_call_function() {
        let lc = LinkedChars::from_iter(
            "def f { a => hello, world! || b => goodby, moon! }f(a) f(b)".chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };
        interpreter.evaluate();
        assert_eq!(
            interpreter.state.make_string(),
            "hello, world! goodby, moon!".to_string()
        );
    }

    #[test]
    fn define_and_call_function_nested() {
        let lc = LinkedChars::from_iter(
            "def f { a => hello, world! || b => g(b) }def g { a => f(b) || b => f(a) }f(b)".chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };
        interpreter.evaluate();
        assert_eq!(interpreter.state.make_string(), "hello, world!".to_string());
    }

    #[test]
    fn define_and_call_function_longer() {
        let lc = LinkedChars::from_iter(
            "def longer { 
                    (.*)(.)&(.*)(.) => longer(^#1&^#3)
                ||  .+&             => >
                ||    &.+           => <
                ||    &             => =}longer(abc&cde) longer(ab&c) longer(a&ab)"
                .chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };
        interpreter.evaluate();
        assert_eq!(interpreter.state.make_string(), "= > <".to_string());
    }
}
