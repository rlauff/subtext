use crate::linked_chars::LinkedChars;

use crate::scope::evaluate_scope;

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

    // Idx to exactly one char back. We return this so that the replacing function works correctly.
    // Note that the replace function will replace everything AFTER the index we pass, non-inclusive.
    let mut prev_idx = reader_idx;

    for (i, node) in linked_chars.enumerate_with_start(reader_idx) {
        match node.c {
            '(' => {
                // Ignore stray parentheses if we haven't read a function name yet
                if chars_buffer.is_empty() {
                    chars_buffer.push(node.c);
                    if oldest_non_whitespace.is_none() {
                        oldest_non_whitespace = Some(prev_idx);
                    }
                } else {
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
            }

            '{' => {
                // this is a scope
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Curly);
                // Use prev_idx to include the '{' itself in the extracted string
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
                }
            }

            c => {
                chars_buffer.push(c);
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

impl Interpreter<'_> {
    pub fn evaluate(&mut self) {
        // TODO: find jobs and apply the resp. changes until we get Chill back
        // After doing a Job, put the reading head at the start of the returned job.
        // This way, we read the output of the last evaluation back in immediately (for recursion).
        let mut reading_head = 0;
        loop {
            let job = get_new_job(&self.state, reading_head);
            reading_head = job.start; // always read the replacement back in 
            match job.task {
                Task::Chill => break, // we are done

                Task::Scope { content: scope } => {
                    // TODO make sure that the start and end of the job point to the right ndoes
                    // evaluate the scope
                    let result_of_evaluation = evaluate_scope(scope, self);
                    // modify the state
                    self.state
                        .replace_between(job.start, job.end, result_of_evaluation);
                }

                _ => unimplemented!(),
            }
        }
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
}
