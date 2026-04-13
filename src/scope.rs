use std::vec;

use crate::error::{ErrorKind, SubtextError};
use crate::interpreter::*;
use crate::linked_chars::*;

use regex::Regex;

/// Helper function: Splits a string at the very first occurrence of a string delimiter,
/// BUT only if the delimiter is not enclosed in braces (depth = 0).
fn split_once_at_top_level(
    input: &str,
    delimiter: &str,
) -> Result<(String, Option<String>), SubtextError> {
    let mut stack: Vec<(char, usize)> = Vec::new();

    for (i, c) in input.char_indices() {
        match c {
            '{' | '(' => stack.push((c, i)),
            '}' | ')' => {
                let matches = match stack.pop() {
                    Some((open, _)) => (open == '{' && c == '}') || (open == '(' && c == ')'),
                    None => false,
                };

                if !matches {
                    return Err(SubtextError::new(ErrorKind::UnmatchedClosingBrace {
                        found: c,
                        position: i,
                    }));
                }
            }
            _ if stack.is_empty() && input[i..].starts_with(delimiter) => {
                // We found the multi-character delimiter at the top level!
                let left = input[..i].to_string();
                let right = input[i + delimiter.len()..].to_string();
                return Ok((left, Some(right)));
            }
            _ => {}
        }
        // Advance the index by the byte length of the current UTF-8 character
    }

    if let Some((open, pos)) = stack.last() {
        let expected = if *open == '{' { '}' } else { ')' };
        return Err(SubtextError::new(ErrorKind::UnmatchedOpeningBrace {
            expected_closing: expected,
            opened_at: *pos,
        }));
    }

    Ok((input.to_string(), None))
}

/// Helper function: Splits a string at ALL occurrences of a string delimiter
/// at the top level (depth = 0). Useful for separating the '||' arms.
fn split_all_at_top_level(input: &str, delimiter: &str) -> Result<Vec<String>, SubtextError> {
    let mut result = Vec::new();
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut i = 0;
    let mut last_split = 0;

    while i < input.len() {
        let c = input[i..].chars().next().unwrap();

        match c {
            '{' | '(' => stack.push((c, i)),
            '}' | ')' => {
                let matches = match stack.pop() {
                    Some((open, _)) => (open == '{' && c == '}') || (open == '(' && c == ')'),
                    None => false,
                };

                if !matches {
                    return Err(SubtextError::new(ErrorKind::UnmatchedClosingBrace {
                        found: c,
                        position: i,
                    }));
                }
            }
            _ if stack.is_empty() && input[i..].starts_with(delimiter) => {
                // Delimiter found at top level, slice the string from the last split point
                result.push(input[last_split..i].to_string());
                // Skip past the delimiter
                i += delimiter.len();
                last_split = i;
                continue;
            }
            _ => {}
        }
        i += c.len_utf8();
    }

    if let Some((open, pos)) = stack.last() {
        let expected = if *open == '{' { '}' } else { ')' };
        return Err(SubtextError::new(ErrorKind::UnmatchedOpeningBrace {
            expected_closing: expected,
            opened_at: *pos,
        }));
    }

    // Add the final remaining part of the string
    result.push(input[last_split..].to_string());
    Ok(result)
}

pub fn evaluate_scope(
    scope: String,
    parent_interpreter: &Interpreter,
    function_name: Option<&str>,
) -> Result<(LinkedChars, Option<Vec<LinkedChars>>), SubtextError> {
    let trimmed_scope = scope.trim();

    // 1. Safely remove the outermost braces.
    let inner_content = if trimmed_scope.starts_with('{') && trimmed_scope.ends_with('}') {
        &trimmed_scope[1..trimmed_scope.len() - 1]
    } else {
        trimmed_scope
    };

    // 2. Separate input and the rest (the arms) using '::' at the top level
    let (input_string, rest) = split_once_at_top_level(inner_content, "::")
        .map_err(|err| parent_interpreter.attach_backtrace_without_highlight(err))?;

    // 3. Evaluate the input string until there are no further changes
    let input_state = LinkedChars::from_iter(input_string.chars());
    let mut input_interpreter = Interpreter {
        history: parent_interpreter
            .history
            .as_ref()
            .map(|_| vec![input_state.clone()]),
        state: input_state,
        parent: Some(parent_interpreter),
        registers: vec![],
        functions: vec![],
    };
    input_interpreter.evaluate()?;
    let input = input_interpreter.state.make_string().trim().to_string();

    //3.5 If there is no :: we have a scope which  returns the processed input
    let rest = match rest {
        Some(r) => r,
        None => return Ok((input_interpreter.state, input_interpreter.history)),
    };

    // 4. Split the rest into individual arms (separated by '||')
    let arms = split_all_at_top_level(&rest, "||")
        .map_err(|err| parent_interpreter.attach_backtrace_without_highlight(err))?;

    for arm in arms {
        // 5. Split each arm into pattern and output (separated by '=>')
        let (pattern_string, output_string) = match split_once_at_top_level(&arm, "=>")
            .map_err(|err| parent_interpreter.attach_backtrace_without_highlight(err))?
        {
            (left, Some(right)) => (left, right),
            (_, None) => {
                return Err(parent_interpreter.attach_backtrace_without_highlight(
                    SubtextError::new(ErrorKind::MalformedArmMissingArrow {
                        arm_content: arm.trim().to_string(),
                    }),
                ));
            }
        };
        // uncomment to activate evaluation in patterns
        // the problem with this is that regex patterns will contain braces, which messes
        // up the rest of the parsing
        //
        // Evaluate the pattern string
        // let mut pattern_interpreter = Interpreter {
        //     state: LinkedChars::from_iter(pattern_string.chars()),
        //     parent: Some(parent_interpreter),
        //     registers: vec![],
        //     functions: parent_interpreter.functions.clone(),
        //  };
        // pattern_interpreter.evaluate();
        let pattern = pattern_string.trim().to_string();
        let output_string = output_string.trim().to_string();

        // 6. Create Regex and attempt to match against the evaluated input
        let re = Regex::new(&pattern).map_err(|err| {
            parent_interpreter.attach_backtrace_without_highlight(SubtextError::new(
                ErrorKind::InvalidRegex {
                    pattern: pattern.clone(),
                    reason: err.to_string(),
                },
            ))
        })?;
        if let Some(caps) = re.captures(&input) {
            // Populate registers (Capture Groups from the Regex)
            let registers: Vec<String> = caps
                .iter()
                .skip(1)
                .filter_map(|match_opt| match_opt.map(|m| m.as_str().to_string()))
                .collect();

            // 7. Evaluate the output since we have a successful match
            let output_state = LinkedChars::from_iter(output_string.chars());
            let mut output_interpreter = Interpreter {
                history: parent_interpreter
                    .history
                    .as_ref()
                    .map(|_| vec![output_state.clone()]),
                state: output_state,
                parent: Some(parent_interpreter),
                registers,
                functions: vec![],
            };
            output_interpreter.evaluate()?;
            // strip outer layer of protecting braces before returning output
            output_interpreter.state.strip_outer_protection_layer();

            // Return the fully evaluated output state
            // We should put a wrapper around input and output history like { input => } and { => output }.
            match input_interpreter.history.as_ref() {
                Some(input_history) => match output_interpreter.history.as_ref() {
                    Some(output_history) => {
                        let mut combined_history = input_history
                            .clone()
                            .into_iter()
                            .map(|state| {
                                let to_string = match function_name {
                                    Some(name) => {
                                        format!("{}( {} )", name, state.make_string().trim())
                                    }
                                    None => format!("{{ {} => }}", state.make_string().trim()),
                                };
                                LinkedChars::from_iter(to_string.chars())
                            })
                            .collect::<Vec<LinkedChars>>();
                        combined_history.extend(output_history.clone());
                        return Ok((output_interpreter.state, Some(combined_history)));
                    }
                    // this None case should never happen
                    None => {
                        return Err(parent_interpreter.attach_backtrace_without_highlight(
                            SubtextError::new(ErrorKind::InternalInvariant {
                                message: "missing history in output interpreter".to_string(),
                            }),
                        ));
                    }
                },
                None => return Ok((output_interpreter.state, None)),
            }
        }
    }

    // If no patterns match
    Err(
        parent_interpreter.attach_backtrace_without_highlight(SubtextError::new(
            ErrorKind::NoMatchingArm {
                input,
                scope_content: inner_content.trim().to_string(),
            },
        )),
    )
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    // Helper to quickly spin up a dummy parent interpreter for our tests
    fn dummy_interpreter() -> Interpreter<'static> {
        Interpreter {
            state: LinkedChars::new(),
            parent: None,
            registers: vec![],
            functions: vec![],
            history: None,
        }
    }

    #[test]
    fn test_new_syntax_simple_match() {
        let parent = dummy_interpreter();
        let scope = "{ hello :: hello => world }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "world");
    }

    #[test]
    fn test_new_syntax_multiple_arms() {
        let parent = dummy_interpreter();
        let scope = "{ test :: foo => bad || test => success }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "success");
    }

    #[test]
    fn test_nested_scopes_with_new_syntax() {
        let parent = dummy_interpreter();
        // Inner evaluates to "b". Outer matches "b" and outputs "c".
        let scope = "{ { a :: a => b } :: b => c }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "c");
    }

    // --- Complex Regex Tests (Testing the advantage of the new syntax) ---

    #[test]
    fn test_regex_with_colons_and_semicolons() {
        let parent = dummy_interpreter();
        // The regex uses a colon inside a non-capturing group `(?:...)` and matches a literal time.
        // Input: "12:30". Regex: "(?:12|24):[0-5][0-9]".
        // With the old single colon syntax, this would have broken the parser immediately!
        let scope = "{ 12:30 :: (?:12|24):[0-5][0-9] => match_time }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "match_time");
    }

    #[test]
    fn test_regex_with_or_operator_collision_check() {
        let parent = dummy_interpreter();
        // The regex uses `|` (OR operator). Our arm separator is `||`.
        // We want to make sure a single `|` in the regex doesn't accidentally trigger an arm split.
        let scope = "{ apple :: banana|apple => fruit || dog|cat => animal }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "fruit");
    }

    #[test]
    fn test_evaluate_with_register_call() {
        let parent = dummy_interpreter();
        let scope = "{ world hello, :: (.....) (......) => #2 #1! }".to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "hello, world!");
    }

    #[test]
    fn test_evaluate_with_register_call_nested() {
        let parent = dummy_interpreter();
        let scope =
            "{ world hello, moon! :: (.....) (......) (.*) => #2 #1! { Goodby, :: (.*) => #1 ^#3 } }"
                .to_string();
        let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
        assert_eq!(result.0.make_string().trim(), "hello, world! Goodby, moon!");
    }

    // --- Error Case Tests ---

    #[test]
    fn test_no_match_returns_error() {
        let parent = dummy_interpreter();
        let scope = "{ input :: unknown => output }".to_string();
        let result = evaluate_scope(scope, &parent, None);
        assert!(result.is_err(), "Expected NoMatchingArm error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NoMatchingArm { .. }));
    }

    #[test]
    fn test_invalid_regex_returns_error() {
        let parent = dummy_interpreter();
        let scope = "{ input :: [ => output }".to_string();

        let result = evaluate_scope(scope, &parent, None);

        assert!(result.is_err(), "Expected InvalidRegex error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::InvalidRegex { .. }));
    }

    #[test]
    fn test_unmatched_closing_brace_in_arm() {
        let parent = dummy_interpreter();
        let scope = "{ input :: ) => output }".to_string();

        let result = evaluate_scope(scope, &parent, None);

        assert!(result.is_err(), "Expected UnmatchedClosingBrace error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnmatchedClosingBrace { .. }));
    }

    #[test]
    fn test_unmatched_opening_brace_in_scope() {
        let parent = dummy_interpreter();
        let scope = "{ input :: (abc => output }".to_string();

        let result = evaluate_scope(scope, &parent, None);

        assert!(result.is_err(), "Expected UnmatchedOpeningBrace error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnmatchedOpeningBrace { .. }));
    }

    #[test]
    fn test_malformed_arm() {
        let parent = dummy_interpreter();
        // Second arm is missing the `=>` separator
        let scope = "{ a :: b => c || broken_arm_without_arrow }".to_string();
        let result = evaluate_scope(scope, &parent, None);
        assert!(result.is_err(), "Expected MalformedArmMissingArrow error");
        let err = result.unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::MalformedArmMissingArrow { .. }
        ));
    }
}
