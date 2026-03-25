use crate::interpreter::*;
use crate::linked_chars::*;

use regex::Regex;

/// Helper function: Splits a string at the very first occurrence of a string delimiter,
/// BUT only if the delimiter is not enclosed in braces (depth = 0).
fn split_once_at_top_level(input: &str, delimiter: &str) -> Option<(String, String)> {
    let mut depth = 0;
    let mut i = 0;

    while i < input.len() {
        let c = input[i..].chars().next().unwrap(); // Safe because i < input.len()

        match c {
            '{' | '(' => depth += 1, // We enter an inner scope
            '}' | ')' => depth -= 1, // We leave an inner scope
            _ if depth == 0 && input[i..].starts_with(delimiter) => {
                // We found the multi-character delimiter at the top level!
                let left = input[..i].to_string();
                let right = input[i + delimiter.len()..].to_string();
                return Some((left, right));
            }
            _ => {}
        }
        // Advance the index by the byte length of the current UTF-8 character
        i += c.len_utf8();
    }
    None
}

/// Helper function: Splits a string at ALL occurrences of a string delimiter
/// at the top level (depth = 0). Useful for separating the '||' arms.
fn split_all_at_top_level(input: &str, delimiter: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    let mut last_split = 0;

    while i < input.len() {
        let c = input[i..].chars().next().unwrap();

        match c {
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            _ if depth == 0 && input[i..].starts_with(delimiter) => {
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

    // Add the final remaining part of the string
    result.push(input[last_split..].to_string());
    result
}

pub fn evaluate_scope(scope: String, parent_interpreter: &Interpreter) -> LinkedChars {
    let trimmed_scope = scope.trim();

    // 1. Safely remove the outermost braces.
    let inner_content = if trimmed_scope.starts_with('{') && trimmed_scope.ends_with('}') {
        &trimmed_scope[1..trimmed_scope.len() - 1]
    } else {
        trimmed_scope
    };

    // 2. Separate input and the rest (the arms) using '::' at the top level
    if let Some((input_string, rest)) = split_once_at_top_level(inner_content, "::") {
        // 3. Evaluate the input string until there are no further changes
        let mut input_interpreter = Interpreter {
            state: LinkedChars::from_iter(input_string.chars()),
            parent: Some(parent_interpreter),
            registers: vec![],
            functions: parent_interpreter.functions.clone(),
        };
        input_interpreter.evaluate();
        let input = input_interpreter.state.make_string().trim().to_string();

        // 4. Split the rest into individual arms (separated by '||')
        let arms = split_all_at_top_level(&rest, "||");

        for arm in arms {
            // 5. Split each arm into pattern and output (separated by '=>')
            if let Some((pattern_string, output_string)) = split_once_at_top_level(&arm, "=>") {
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

                // 6. Create Regex and attempt to match against the evaluated input
                let re =
                    Regex::new(&pattern).unwrap_or_else(|_| panic!("Invalid Regex: {}", pattern));
                if let Some(caps) = re.captures(&input) {
                    // Populate registers (Capture Groups from the Regex)
                    let registers: Vec<String> = caps
                        .iter()
                        .filter_map(|match_opt| match_opt.map(|m| m.as_str().to_string()))
                        .collect();

                    // 7. Evaluate the output since we have a successful match
                    let mut output_interpreter = Interpreter {
                        state: LinkedChars::from_iter(output_string.chars()),
                        parent: Some(parent_interpreter),
                        registers,
                        functions: parent_interpreter.functions.clone(),
                    };
                    output_interpreter.evaluate();

                    // Return the fully evaluated output state
                    return output_interpreter.state;
                }
            } else {
                panic!(
                    "Found an arm which does not contain the '=>' separator: {}",
                    arm
                );
            }
        }
    } else {
        panic!("No '::' found at the top level, meaning there is no input defined");
    }

    // If no patterns match
    panic!("None of the arms matched the input!");
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // Helper to quickly spin up a dummy parent interpreter for our tests
    fn dummy_interpreter() -> Interpreter<'static> {
        Interpreter {
            state: LinkedChars::new(),
            parent: None,
            registers: vec![],
            functions: vec![],
        }
    }

    #[test]
    fn test_new_syntax_simple_match() {
        let parent = dummy_interpreter();
        let scope = "{ hello :: hello => world }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "world");
    }

    #[test]
    fn test_new_syntax_multiple_arms() {
        let parent = dummy_interpreter();
        let scope = "{ test :: foo => bad || test => success }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "success");
    }

    #[test]
    fn test_nested_scopes_with_new_syntax() {
        let parent = dummy_interpreter();
        // Inner evaluates to "b". Outer matches "b" and outputs "c".
        let scope = "{ { a :: a => b } :: b => c }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "c");
    }

    // --- Complex Regex Tests (Testing the advantage of the new syntax) ---

    #[test]
    fn test_regex_with_colons_and_semicolons() {
        let parent = dummy_interpreter();
        // The regex uses a colon inside a non-capturing group `(?:...)` and matches a literal time.
        // Input: "12:30". Regex: "(?:12|24):[0-5][0-9]".
        // With the old single colon syntax, this would have broken the parser immediately!
        let scope = "{ 12:30 :: (?:12|24):[0-5][0-9] => match_time }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "match_time");
    }

    #[test]
    fn test_regex_matching_complex_urls() {
        let parent = dummy_interpreter();
        // Testing colons and slashes inside the input AND the regex pattern
        let scope =
            "{ https://google.com :: https?://[a-z]+\\.[a-z]{2,3} => valid_url }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "valid_url");
    }

    #[test]
    fn test_regex_with_or_operator_collision_check() {
        let parent = dummy_interpreter();
        // The regex uses `|` (OR operator). Our arm separator is `||`.
        // We want to make sure a single `|` in the regex doesn't accidentally trigger an arm split.
        let scope = "{ apple :: banana|apple => fruit || dog|cat => animal }".to_string();
        let result = evaluate_scope(scope, &parent);
        assert_eq!(result.make_string().trim(), "fruit");
    }

    // --- Error Case Tests ---

    #[test]
    #[should_panic(expected = "None of the arms matched")]
    fn test_no_match_panics() {
        let parent = dummy_interpreter();
        let scope = "{ input :: unknown => output }".to_string();
        evaluate_scope(scope, &parent);
    }

    #[test]
    #[should_panic(expected = "No '::' found at the top level")]
    fn test_missing_input_separator() {
        let parent = dummy_interpreter();
        let scope = "{ input : pattern : output }".to_string(); // using old syntax triggers error
        evaluate_scope(scope, &parent);
    }

    #[test]
    #[should_panic(expected = "Found an arm which does not contain the '=>' separator")]
    fn test_malformed_arm() {
        let parent = dummy_interpreter();
        // Second arm is missing the `=>` separator
        let scope = "{ a :: b => c || broken_arm_without_arrow }".to_string();
        evaluate_scope(scope, &parent);
    }
}
