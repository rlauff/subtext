use crate::error::{BacktraceFrame, ErrorKind, SubtextError};
use crate::linked_chars::LinkedChars;

use crate::scope::evaluate_scope;

use std::fs;
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
        requested_index: usize,
        position: usize,
    },
    GetInput {
        prompt: String,
    },
    PrintOutput {
        content: String,
    },
    GetFile {
        path: String,
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
fn find_closing_brace(
    linked_chars: &LinkedChars,
    start_idx: usize,
    brace_type: Brace,
) -> Result<usize, SubtextError> {
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
            return Ok(i);
        }
    }
    Err(SubtextError::new(ErrorKind::UnmatchedOpeningBrace {
        expected_closing: closing_char,
        opened_at: start_idx,
    }))
}

// returns the register number and the index to the last digit
// any non digit char can terminate the register call
// (register number, idx_to_last_digit)
fn find_register_number(
    linked_chars: &LinkedChars,
    start_idx: usize,
) -> Result<(usize, usize), SubtextError> {
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
        return Err(SubtextError::new(ErrorKind::MissingRegisterDigit {
            position: start_idx,
        }));
    };
    if register_number == 0 {
        return Err(SubtextError::new(ErrorKind::RegisterIndexStartsAtOne {
            position: start_idx,
        }));
    }
    Ok((register_number, last_found_digit_idx))
}

// Scans for a function name after a 'def' keyword.
// Returns: (Extracted Name, Index of the node BEFORE the '{', Index of the '{')
fn find_function_name(
    linked_chars: &LinkedChars,
    start_idx: usize,
) -> Result<(String, usize, usize), SubtextError> {
    let mut chars_buffer = Vec::new();
    let mut prev_idx = start_idx;

    for (i, node) in linked_chars.enumerate_with_start(start_idx) {
        match node.c {
            '{' => {
                // The function name ended, we just found the start of the scope
                if chars_buffer.is_empty() {
                    return Err(SubtextError::new(ErrorKind::MissingFunctionName {
                        position: start_idx,
                    }));
                }
                return Ok((chars_buffer.into_iter().collect(), prev_idx, i));
            }
            c if c.is_whitespace() => (), // Ignore whitespace to allow for formatting

            // TODO: check for illegal chars in function name here
            c => chars_buffer.push(c),
        }
        prev_idx = i;
    }
    Err(SubtextError::new(ErrorKind::MissingFunctionBody {
        position: start_idx,
    }))
}

// returns the next job to do
// start should point to the node which comes BEFORE the first relevant one
// end should point to the last relevant node
// Example: here is a scope:  _{ foo : bar : no match }
//                            ^start                  ^end
// start points to the _, end point to the }
fn get_new_job(linked_chars: &LinkedChars, reader_idx: usize) -> Result<Job, SubtextError> {
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
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Round)?;

                // Use prev_idx to include the '(' itself in the extracted string
                let full_string = linked_chars.interval_to_string(prev_idx, closing_brace_idx)?;
                let name: String = chars_buffer.iter().collect();

                let task = match name.as_str() {
                    "get_input" => Task::GetInput {
                        prompt: full_string,
                    },
                    "print_output" => Task::PrintOutput {
                        content: full_string,
                    },
                    "get_file" => Task::GetFile { path: full_string },
                    other_name => Task::FunctionCall {
                        function_name: other_name.to_string(),
                        input: full_string,
                    },
                };

                return Ok(Job {
                    start: oldest_non_whitespace.unwrap_or(0),
                    end: closing_brace_idx,
                    task,
                });
            }

            '{' => {
                // this is a scope
                let closing_brace_idx = find_closing_brace(linked_chars, i, Brace::Curly)?;
                // Use prev_idx to include the '{' itself in the extracted string
                // TODO: In the scope evaluation, the braces are striped away anyway
                // we could just not put them in at all
                let full_string = linked_chars.interval_to_string(prev_idx, closing_brace_idx)?;

                return Ok(Job {
                    start: prev_idx,
                    end: closing_brace_idx,
                    task: Task::Scope {
                        content: full_string,
                    },
                });
            }

            c if c.is_whitespace() => {
                let name: String = chars_buffer.iter().collect();
                if name == "def" {
                    let (function_name, opening_brace_prev, opening_brace_idx) =
                        find_function_name(linked_chars, i)?;
                    let closing_brace_idx =
                        find_closing_brace(linked_chars, opening_brace_idx, Brace::Curly)?;

                    // Extract everything including the braces
                    let definition_string =
                        linked_chars.interval_to_string(opening_brace_prev, closing_brace_idx)?;

                    return Ok(Job {
                        start: oldest_non_whitespace.unwrap_or(0),
                        end: closing_brace_idx,
                        task: Task::DefineFunction {
                            name: function_name,
                            definition: definition_string,
                        },
                    });
                } else {
                    // Reset the buffer and start marker if we hit whitespace and it wasn't 'def'
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
                    find_register_number(linked_chars, i)?;
                // if the terminating char is a space, then we point to the char after that
                let end_idx =
                    if let Some(next_node_idx) = linked_chars.get(idx_to_terminating_char).next {
                        if linked_chars.get(next_node_idx).c == ' ' {
                            next_node_idx
                        } else {
                            idx_to_terminating_char
                        }
                    } else {
                        idx_to_terminating_char
                    };

                return Ok(Job {
                    start: oldest_uptick.unwrap_or(prev_idx),
                    end: end_idx,
                    task: Task::RegisterCall {
                        level: number_consecutive_uptick,
                        requested_index: register_number,
                        position: i,
                    },
                });
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

    Ok(Job {
        task: Task::Chill,
        start: 0,
        end: 0,
    })
}

#[derive(Clone)]
pub struct Function {
    name: String,
    body: String,
}

impl Interpreter<'_> {
    fn apply_replacements(
        &mut self,
        start: usize,
        end: usize,
        replacements: Vec<LinkedChars>,
        history: &mut Vec<LinkedChars>,
    ) {
        let base_state = self.state.clone();
        let mut last_state: Option<LinkedChars> = None;
        for replacement in replacements {
            let mut new_state = base_state.clone();
            new_state.replace_between(start, end, &replacement);
            history.push(new_state.clone());
            last_state = Some(new_state);
        }

        if let Some(state) = last_state {
            self.state = state;
        }
    }

    fn apply_removal(&mut self, start: usize, end: usize, history: &mut Vec<LinkedChars>) {
        let mut new_state = self.state.clone();
        new_state.remove_between(start, end);
        history.push(new_state.clone());
        self.state = new_state;
    }

    pub fn evaluate(&mut self) -> Result<Vec<LinkedChars>, SubtextError> {
        // find jobs and apply the resp. changes until we get Chill back
        // After doing a Job, put the reading head at the start of the returned job.
        // This way, we read the output of the last evaluation back in immediately (for recursion).
        let mut reading_head = 0;
        let mut history = Vec::new();
        loop {
            let job = match get_new_job(&self.state, reading_head) {
                Ok(job) => job,
                Err(err) => {
                    return Err(self.attach_backtrace_if_empty(err, None));
                }
            };
            reading_head = job.start; // always read the replacement back in 
            match job.task {
                Task::Chill => break, // we are done

                Task::Scope { content: scope } => {
                    // evaluate the scope
                    let results = evaluate_scope(scope, self)
                        .map_err(|err| self.attach_backtrace_if_empty(err, None))?;
                    self.apply_replacements(job.start, job.end, results, &mut history);
                }

                Task::RegisterCall {
                    level,
                    requested_index,
                    position,
                } => {
                    let register_value = self
                        .get_register_at_level(level, requested_index)
                        .map_err(|err| self.attach_backtrace_if_empty(err, Some(position)))?;
                    let result = LinkedChars::from_iter(register_value.chars());
                    self.apply_replacements(job.start, job.end, vec![result], &mut history);
                }

                Task::DefineFunction { name, definition } => {
                    // when looking for a function, we will look through this vector in reverse.
                    // This way a new definition will shadow a potential old one
                    self.functions.push(Function {
                        name,
                        body: definition,
                    });
                    self.apply_removal(job.start, job.end, &mut history);
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

                            let results = evaluate_scope(scope, self)
                                .map_err(|err| self.attach_backtrace_if_empty(err, None))?;
                            self.apply_replacements(job.start, job.end, results, &mut history);
                        }
                        None => {
                            return Err(self.attach_backtrace_if_empty(
                                SubtextError::new(ErrorKind::UndefinedFunction {
                                    name: function_name,
                                }),
                                None,
                            ));
                        }
                    }
                }

                Task::GetInput { prompt } => {
                    print!("{}", prompt);
                    io::stdout().flush().map_err(|err| {
                        self.attach_backtrace_if_empty(
                            SubtextError::new(ErrorKind::OutputWriteError {
                                reason: err.to_string(),
                            }),
                            None,
                        )
                    })?;
                    let mut response = String::new();

                    io::stdin().read_line(&mut response).map_err(|err| {
                        self.attach_backtrace_if_empty(
                            SubtextError::new(ErrorKind::InputReadError {
                                reason: err.to_string(),
                            }),
                            None,
                        )
                    })?;

                    let clean_response = response.trim().to_string();
                    let ls = LinkedChars::from_iter(clean_response.chars());
                    self.apply_replacements(job.start, job.end, vec![ls], &mut history);
                }

                Task::GetFile { path } => {
                    let clean_path = if path.starts_with('(') && path.ends_with(')') {
                        &path[1..path.len() - 1]
                    } else {
                        &path
                    };

                    let file_content = match fs::read_to_string(clean_path) {
                        Ok(content) => content,
                        Err(err) => {
                            let io_error = SubtextError::new(ErrorKind::FileReadError {
                                path: clean_path.to_string(),
                                reason: err.to_string(),
                            });
                            return Err(self.attach_backtrace_if_empty(io_error, None));
                        }
                    };

                    let trimmed_content = file_content.trim().to_string();
                    let ls = LinkedChars::from_iter(trimmed_content.chars());
                    self.state.replace_between(job.start, job.end, &ls);
                }

                Task::PrintOutput { content } => {
                    let mut inner_content = if content.starts_with('(') && content.ends_with(')') {
                        content[1..content.len() - 1].to_string()
                    } else {
                        content
                    };

                    if !inner_content.starts_with('\'') {
                        // in this case we evaluate first
                        let lc = LinkedChars::from_iter(inner_content.chars());
                        let mut interpreter = Interpreter {
                            state: lc,
                            registers: self.registers.clone(),
                            parent: self.parent,
                            functions: self.functions.clone(),
                        };
                        interpreter.evaluate()?;
                        inner_content = interpreter.state.make_string();
                    }

                    println!("{}", inner_content);
                    self.apply_removal(job.start, job.end, &mut history);
                }
            }
        }
        Ok(history)
    }

    fn get_register_at_level(
        &self,
        level: usize,
        requested_index: usize,
    ) -> Result<String, SubtextError> {
        let mut current: &Interpreter = self;
        let mut depth_reached = 0;
        for _ in 0..level {
            if let Some(parent_ref) = current.parent {
                current = parent_ref;
                depth_reached += 1;
            } else {
                return Err(SubtextError::new(ErrorKind::MissingParentScope {
                    requested_level: level,
                    actual_depth: depth_reached,
                }));
            }
        }
        if requested_index == 0 {
            return Err(SubtextError::new(ErrorKind::InternalInvariant {
                message: "register index must be >= 1".to_string(),
            }));
        }
        let zero_based = requested_index - 1;
        // We successfully went up `level` times. Return the register found here.
        if let Some(value) = current.registers.get(zero_based) {
            return Ok(value.clone());
        }

        let suggestion = self.find_register_suggestion(level, requested_index);
        Err(SubtextError::new(ErrorKind::RegisterOutOfBounds {
            requested: requested_index,
            available: current.registers.len(),
            suggestion,
        }))
    }

    fn find_register_suggestion(&self, level: usize, requested_index: usize) -> Option<String> {
        let mut current = self;
        for _ in 0..level {
            current = current.parent?;
        }

        if requested_index == 0 {
            return None;
        }
        let zero_based = requested_index - 1;

        let mut extra = 1;
        let mut ancestor = current.parent;
        while let Some(parent_ref) = ancestor {
            if zero_based < parent_ref.registers.len() {
                let prefix = "^".repeat(level + extra);
                return Some(format!("{}#{}", prefix, requested_index));
            }
            ancestor = parent_ref.parent;
            extra += 1;
        }
        None
    }

    fn build_backtrace(&self, highlight: Option<usize>) -> Vec<BacktraceFrame> {
        let mut frames = Vec::new();
        let mut current: Option<&Interpreter> = Some(self);
        let mut depth = 0;

        while let Some(interpreter) = current {
            let snippet = if depth == 0 {
                interpreter.state.make_snippet(highlight, 80)
            } else {
                interpreter.state.make_snippet(None, 80)
            };

            frames.push(BacktraceFrame {
                depth,
                full_state: interpreter.state.clone(),
                state_snippet: snippet,
                registers: interpreter.registers.clone(),
                defined_functions: interpreter
                    .functions
                    .iter()
                    .map(|func| func.name.clone())
                    .collect(),
            });

            current = interpreter.parent;
            depth += 1;
        }

        frames
    }

    pub(crate) fn attach_backtrace_if_empty(
        &self,
        mut err: SubtextError,
        highlight: Option<usize>,
    ) -> SubtextError {
        if err.backtrace.is_empty() {
            let derived_highlight = highlight.or_else(|| self.highlight_from_error_kind(&err.kind));
            err.backtrace = self.build_backtrace(derived_highlight);
        }
        err
    }

    pub(crate) fn attach_backtrace_without_highlight(&self, mut err: SubtextError) -> SubtextError {
        if err.backtrace.is_empty() {
            err.backtrace = self.build_backtrace(None);
        }
        err
    }

    fn highlight_from_error_kind(&self, kind: &ErrorKind) -> Option<usize> {
        match kind {
            ErrorKind::UnmatchedOpeningBrace { opened_at, .. } => Some(*opened_at),
            ErrorKind::UnmatchedClosingBrace { position, .. } => Some(*position),
            ErrorKind::MissingRegisterDigit { position } => Some(*position),
            ErrorKind::MissingFunctionName { position } => Some(*position),
            ErrorKind::MissingFunctionBody { position } => Some(*position),
            ErrorKind::RegisterIndexStartsAtOne { position } => Some(*position),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[test]
    fn test_find_closing_brace_flat() {
        let lc = LinkedChars::from_iter("(abc)".chars());
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round).unwrap();
        assert_eq!(closing_idx, 5);
    }

    #[test]
    fn test_find_closing_brace_nested() {
        let lc = LinkedChars::from_iter("(a(b)c)".chars());
        let closing_idx = find_closing_brace(&lc, 1, Brace::Round).unwrap();
        assert_eq!(closing_idx, 7);
    }

    #[test]
    fn test_find_closing_brace_missing() {
        let lc = LinkedChars::from_iter("(abc".chars());
        let result = find_closing_brace(&lc, 1, Brace::Round);
        assert!(result.is_err(), "Expected missing closing brace to error");
        let err = result.unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnmatchedOpeningBrace { .. }),
            "Expected UnmatchedOpeningBrace, got {:?}",
            err.kind
        );
    }

    #[test]
    fn test_find_function_name() {
        let lc = LinkedChars::from_iter("  my_func  {".chars());
        let (name, prev_idx, brace_idx) = find_function_name(&lc, 0).unwrap();
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
        let job = get_new_job(&lc, 0).unwrap();

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
        let job = get_new_job(&lc, 0).unwrap();

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
        let job = get_new_job(&lc, 0).unwrap();

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
        let job = get_new_job(&lc, 0).unwrap();

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
        let job = get_new_job(&lc, 0).unwrap();

        assert_eq!(job.task, Task::Chill);
    }

    #[test]
    fn test_get_new_job_loop_around() {
        let lc = LinkedChars::from_iter("  foo()".chars());
        let job = get_new_job(&lc, 5).unwrap();

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
        interpreter.evaluate().expect("Evaluation failed");
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
        interpreter.evaluate().expect("Evaluation failed");
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
        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string(), "= > <".to_string());
    }

    #[test]
    fn define_function_with_newlines() {
        let lc = LinkedChars::from_iter("def\nadd_positive { a => ok } add_positive(a)".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "ok");
    }

    #[test]
    fn define_function_body_without_input_separator() {
        let lc = LinkedChars::from_iter("def f { (a) => ok } f(a)".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "ok");
    }

    #[test]
    fn test_missing_register_digit_error() {
        let lc = LinkedChars::from_iter("#".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected MissingRegisterDigit error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::MissingRegisterDigit { .. }));
        assert!(
            !err.backtrace.is_empty(),
            "Expected backtrace to be present"
        );
    }

    #[test]
    fn test_missing_function_name_error() {
        let lc = LinkedChars::from_iter("def { a => b }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected MissingFunctionName error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::MissingFunctionName { .. }));
    }

    #[test]
    fn test_missing_function_body_error() {
        let lc = LinkedChars::from_iter("def name".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected MissingFunctionBody error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::MissingFunctionBody { .. }));
    }

    #[test]
    fn test_undefined_function_error() {
        let lc = LinkedChars::from_iter("foo()".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected UndefinedFunction error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UndefinedFunction { .. }));
        assert!(
            !err.backtrace.is_empty(),
            "Expected backtrace to be present"
        );
    }

    #[test]
    fn test_register_out_of_bounds_error() {
        let lc = LinkedChars::from_iter("{ a :: (a) => #3 }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected RegisterOutOfBounds error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::RegisterOutOfBounds { .. }));
    }

    #[test]
    fn test_register_call_trailing_whitespace_is_ignored() {
        let lc = LinkedChars::from_iter("{ a :: (a) => #1 1 }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "a1");
    }

    #[test]
    fn test_register_suggestion_from_parent() {
        let lc = LinkedChars::from_iter("{ ab :: (a)(b) => { ok :: ok => #2 } }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected RegisterOutOfBounds error");
        let err = result.unwrap_err();
        match err.kind {
            ErrorKind::RegisterOutOfBounds { suggestion, .. } => {
                assert_eq!(suggestion, Some("^#2".to_string()));
            }
            other => panic!("Unexpected error kind: {:?}", other),
        }
    }

    #[test]
    fn test_register_index_starts_at_one() {
        let lc = LinkedChars::from_iter("#0".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected RegisterIndexStartsAtOne error");
        let err = result.unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::RegisterIndexStartsAtOne { .. }
        ));
    }

    #[test]
    fn test_missing_parent_scope_error() {
        let lc = LinkedChars::from_iter("^^#1".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected MissingParentScope error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::MissingParentScope { .. }));
    }
}
