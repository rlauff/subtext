use crate::linkes_chars::LinkedChars;

// Helper enum to easily switch between searching for round or curly braces.
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

#[derive(PartialEq, Debug)]
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

#[derive(PartialEq, Debug)]
struct Job {
    start: usize, // The index of the node BEFORE the stuff to be replaced
    end: usize,   // The end index of the stuff to be replaced
    task: Task,
}

// Finds the matching closing brace. Expects start_idx to be the index of the opening brace.
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

        // When all opened braces are closed, we found our match
        if number_opened == 0 {
            return i;
        }
    }
    panic!("Matching closing brace not found");
}

// Extracts the function name and returns (name, index_of_opening_curly_brace).
fn find_function_name(linked_chars: &LinkedChars, start_idx: usize) -> (String, usize) {
    let mut chars_buffer = Vec::new();
    for (i, node) in linked_chars.enumerate_with_start(start_idx) {
        match node.c {
            '{' => {
                if chars_buffer.is_empty() {
                    panic!("No function name provided, 'def' must be followed by a function name");
                }
                return (chars_buffer.into_iter().collect(), i);
            }
            ' ' => (), // Ignore whitespace to allow for formatting
            c => chars_buffer.push(c),
        }
    }
    panic!("Never found the scope with the actual function def");
}

fn get_new_job(linked_chars: &LinkedChars, reader_idx: usize) -> Job {
    let mut chars_buffer = Vec::new(); // Holds the read chars

    // Index to the node PRECEDING the oldest non-whitespace char.
    // Crucial for the replace_between function to hook in correctly.
    let mut oldest_non_whitespace = 0;

    // Tracks the index of the node immediately preceding the current iteration step.
    let mut prev_idx = reader_idx;

    for (i, node) in linked_chars.enumerate_with_start(reader_idx) {
        match node.c {
            '(' => {
                // This is a function call. Find the closing brace.
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
                    start: oldest_non_whitespace,
                    end: closing_brace_idx,
                    task,
                };
            }

            '{' => {
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Curly);
                let full_string = linked_chars.interval_to_string(i, closing_brace_idx);

                return Job {
                    start: prev_idx, // Use the node immediately before the '{'
                    end: closing_brace_idx,
                    task: Task::Scope {
                        content: full_string,
                    },
                };
            }

            ' ' => {
                if chars_buffer.iter().collect::<String>().as_str() == "def" {
                    let (function_name, opening_brace_idx) = find_function_name(linked_chars, i);
                    let closing_brace_idx =
                        find_closing_brace(linked_chars, opening_brace_idx, Brace::Curly);
                    let definition_string = linked_chars.interval_to_string(i, closing_brace_idx);

                    return Job {
                        start: oldest_non_whitespace,
                        end: closing_brace_idx,
                        task: Task::DefineFunction {
                            name: function_name,
                            definition: definition_string,
                        },
                    };
                } else {
                    chars_buffer.clear();
                    // Reset the start marker when clearing the buffer!
                    oldest_non_whitespace = 0;
                }
            }

            c => {
                chars_buffer.push(c);
                if oldest_non_whitespace == 0 {
                    // Mark the node right BEFORE this token sequence started
                    oldest_non_whitespace = prev_idx;
                }
            }
        }

        // Update prev_idx at the end of every loop iteration to ensure it always lags exactly 1 step behind
        prev_idx = i;
    }

    if reader_idx != 0 {
        // If we reached the end but didn't start at 0, try again from the beginning
        return get_new_job(linked_chars, 0);
    }

    Job {
        task: Task::Chill,
        start: 0,
        end: 0,
    }
}

// An Interpreter gets passed a LinkedChars and is tasked to evaluate it until there are no further changes.
// It will save regex matches into its own registers.
// Its children may use the contents of these registers by using the ^ operator on register calls.
struct Interpreter<'a> {
    state: LinkedChars,
    parent: Option<&'a Interpreter<'a>>,
    registers: Vec<String>,
}

impl Interpreter<'_> {
    fn evaluate(&mut self) {
        // TODO: find jobs and apply the resp. changes until we get Chill back.
        // After doing a Job, put the reading head at the start of the returned job.
        // This way, we read the output of the last evaluation back in immediately (for recursion).
    }
}

// -----------------------------------------------------------------------------
// Unit Tests for Interpreter Logic
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_closing_brace_flat() {
        // Code: "(abc)". Node 1 is '('.
        let lc = LinkedChars::new_from_iter("(abc)".chars());
        // start_idx is 1 (the opening brace). It should find the closing brace at node 5.
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round);
        assert_eq!(
            closing_idx, 5,
            "Should find the closing brace exactly at node 5"
        );
    }

    #[test]
    fn test_find_closing_brace_nested() {
        // Code: "(a(b)c)". Node 1 is '(', node 7 is ')'.
        let lc = LinkedChars::new_from_iter("(a(b)c)".chars());
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round);
        assert_eq!(
            closing_idx, 7,
            "Should correctly ignore nested braces and find the outer one"
        );
    }

    #[test]
    #[should_panic(expected = "Matching closing brace not found")]
    fn test_find_closing_brace_missing() {
        let lc = LinkedChars::new_from_iter("(abc".chars());
        find_closing_brace(&lc, 1, Brace::Round); // Should panic because there is no ')'
    }

    #[test]
    fn test_find_function_name() {
        // We parsed "def", now we scan the rest: "  my_func  {...}"
        // Dummy(0), ' '(1), ' '(2), 'm'(3)...
        let lc = LinkedChars::new_from_iter("  my_func  {".chars());
        // start scanning from dummy. Expected: name="my_func", index of '{' = 12
        let (name, brace_idx) = find_function_name(&lc, 0);
        assert_eq!(
            name, "my_func",
            "Should skip spaces and extract the function name"
        );
        assert_eq!(
            brace_idx, 12,
            "Should return the exact index of the curly brace"
        );
    }

    #[test]
    fn test_get_new_job_function_call() {
        // Code: "foo(bar)". Node 0=Dummy, 1='f', 2='o', 3='o', 4='(', 5='b', 6='a', 7='r', 8=')'
        let lc = LinkedChars::new_from_iter("foo(bar)".chars());
        let job = get_new_job(&lc, 0);

        let expected_job = Job {
            start: 0, // Points to Dummy, right BEFORE 'foo'
            end: 8,   // Points exactly at ')'
            task: Task::FunctionCall {
                function_name: "foo".to_string(),
                input: "(bar)".to_string(), // Input includes the braces based on interval_to_string logic
            },
        };
        assert_eq!(
            job, expected_job,
            "Should correctly parse a standard function call"
        );
    }

    #[test]
    fn test_get_new_job_built_in_functions() {
        // Code: "print_output(123)". Length: 17 chars. Node 17 is ')'.
        let lc = LinkedChars::new_from_iter("print_output(123)".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.start, 0, "Start is dummy node");
        assert_eq!(job.end, 17, "End is the closing brace");
        assert_eq!(
            job.task,
            Task::PrintOutput {
                content: "(123)".to_string()
            },
            "Should recognize the reserved keyword 'print_output'"
        );
    }

    #[test]
    fn test_get_new_job_scope() {
        // Code: "  { a }". Node 0=Dummy, 1=' ', 2=' ', 3='{', ...
        let lc = LinkedChars::new_from_iter("  { a }".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(
            job.start, 2,
            "Start must be the node immediately PRECEDING the opening brace '{'"
        );
        assert_eq!(
            job.task,
            Task::Scope {
                content: "{ a }".to_string()
            },
            "Should correctly identify a standalone scope"
        );
    }

    #[test]
    fn test_get_new_job_def_function() {
        // Code: "def my_func { body }".
        let lc = LinkedChars::new_from_iter("def my_func { body }".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.start, 0, "Start points to Dummy node BEFORE 'def'");
        // Ensure that the task is interpreted as DefineFunction.
        if let Task::DefineFunction { name, definition } = job.task {
            assert_eq!(name, "my_func");
            assert_eq!(definition, "{ body }");
        } else {
            panic!("Expected DefineFunction task, got something else");
        }
    }

    #[test]
    fn test_get_new_job_chill() {
        // Code with no action items.
        let lc = LinkedChars::new_from_iter("just_some_text".chars());
        let job = get_new_job(&lc, 0);

        assert_eq!(job.task, Task::Chill, "If no job is found, return Chill");
    }

    #[test]
    fn test_get_new_job_loop_around() {
        // Code: "  foo()", but we start reading from index 5 ('o').
        // The reader hits the end, realizes it didn't start at 0, loops back, and finds "foo()".
        let lc = LinkedChars::new_from_iter("  foo()".chars());
        let job = get_new_job(&lc, 5);

        assert_eq!(job.start, 2, "Start is right before 'f'");
        if let Task::FunctionCall { function_name, .. } = job.task {
            assert_eq!(
                function_name, "foo",
                "Loop around should successfully find the job"
            );
        } else {
            panic!("Expected FunctionCall");
        }
    }
}
