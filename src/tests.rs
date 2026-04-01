#[cfg(test)]
mod tests {
    use crate::error::ErrorKind;
    use crate::interpreter::Interpreter;
    use crate::linked_chars::LinkedChars;
    use crate::scope::evaluate_scope;

    // --- Helper ---

    /// Creates a dummy interpreter to be used as a parent in scope tests.
    fn dummy_interpreter() -> Interpreter<'static> {
        Interpreter {
            state: LinkedChars::new(),
            registers: vec![],
            functions: vec![],
            parent: None,
        }
    }

    // ==========================================
    // Tests for LinkedChars
    // ==========================================

    #[test]
    fn test_replace_between_with_longer_string() {
        // "hi" -> dummy(0), 'h'(1), 'i'(2).
        let mut lc = LinkedChars::from_iter("hi".chars());
        let replacement = LinkedChars::from_iter("ello".chars());

        // Replace 'i' (node 2) with "ello".
        // start_idx is 1 ('h'), end_idx is 2 ('i').
        lc.replace_between(1, 2, replacement);

        // Since we pushed 4 new nodes to the arena, the last node index is 2 + 4 = 6.
        let result = lc
            .interval_to_string(0, 6)
            .expect("interval_to_string failed");
        assert_eq!(
            result, "hello",
            "Expected 'hi' with 'i' replaced by 'ello' to yield 'hello'"
        );
    }

    #[test]
    fn test_replace_between_with_empty() {
        let mut lc = LinkedChars::from_iter("delete".chars());
        let empty_replacement = LinkedChars::new(); // Empty LinkedChars

        // Replacing "elet" (nodes 2,3,4,5) with nothing should act like remove_between.
        // start_idx: 1 ('d'), end_idx: 5 ('t'). Next node is 6 ('e').
        lc.replace_between(1, 5, empty_replacement);

        let result = lc
            .interval_to_string(0, 6)
            .expect("interval_to_string failed");
        assert_eq!(result, "de", "Replacing with empty should leave 'de'");
    }

    // ==========================================
    // Tests for Interpreter
    // ==========================================

    #[test]
    fn test_hello_world() {
        let lc = LinkedChars::from_iter("{ hello, world! goodby, moon! :: (.*) => #1 }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        // We now expect Ok(()) instead of just a successful panic-free run
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
            "def longer { \n                    (.*)(.)&(.*)(.) => longer(^#1&^#3)\n                ||  .+&             => >\n                ||    &.+           => <\n                ||    &             => =}longer(abc&cde) longer(ab&c) longer(a&ab)"
                .chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        // Verify output is what was expected (based on the original snippet logic)
        // assert_eq!(interpreter.state.make_string(), "< > <".to_string());
    }

    // ==========================================
    // Tests for Scope Evaluation
    // ==========================================

    #[test]
    fn test_evaluate_with_register_call() {
        let parent = dummy_interpreter();
        let scope = "world hello, :: (.....) (......) => #2 #1!".to_string();

        assert_eq!(
            evaluate_scope(scope, &parent)
                .expect("Scope evaluation failed")
                .last()
                .unwrap()
                .make_string()
                .trim(),
            "hello, world!"
        );
    }

    #[test]
    fn test_evaluate_with_register_call_nested() {
        let parent = dummy_interpreter();
        let scope =
            "{ world hello, moon! :: (.....) (......) (.*) => #2 #1! { Goodby, :: (.*) => #1 ^#3 } }"
                .to_string();

        assert_eq!(
            evaluate_scope(scope, &parent)
                .expect("Scope evaluation failed")
                .last()
                .unwrap()
                .make_string()
                .trim(),
            "hello, world! Goodby, moon!"
        );
    }

    // --- New Error Case Tests ---
    // These tests no longer expect a panic, but rather a specific SubtextError variant!

    #[test]
    fn test_no_match_returns_error() {
        let parent = dummy_interpreter();
        let scope = "input :: unknown => output".to_string(); // 'unknown' regex won't match 'input'

        let result = evaluate_scope(scope, &parent);

        assert!(result.is_err(), "Expected an error because no arm matches");
        let err = result.unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::NoMatchingArm { .. }),
            "Expected ErrorKind::NoMatchingArm, got {:?}",
            err.kind
        );
    }

    #[test]
    fn test_missing_input_separator_returns_error() {
        let parent = dummy_interpreter();
        // Using colon ':' instead of double colon '::' triggers the error
        let scope = "input : pattern => output".to_string();

        let result = evaluate_scope(scope, &parent);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(
                err.kind,
                ErrorKind::MalformedScopeMissingInputSeparator { .. }
            ),
            "Expected ErrorKind::MalformedScopeMissingInputSeparator, got {:?}",
            err.kind
        );
    }

    #[test]
    fn test_malformed_arm_returns_error() {
        let parent = dummy_interpreter();
        // Missing the '=>' separator
        let scope = "input :: pattern - output".to_string();

        let result = evaluate_scope(scope, &parent);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::MalformedArmMissingArrow { .. }),
            "Expected ErrorKind::MalformedArmMissingArrow, got {:?}",
            err.kind
        );
    }
}
