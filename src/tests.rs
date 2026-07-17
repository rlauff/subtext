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
    fn test_new_and_is_empty() {
        // A newly created LinkedChars should be empty (only contains dummy node).
        let lc = LinkedChars::new();
        assert!(lc.is_empty(), "New LinkedChars should be empty");
        assert_eq!(
            lc.arena.len(),
            1,
            "Arena should only contain the dummy node"
        );
    }

    #[test]
    fn test_new_from_iter_and_interval_to_string() {
        // Tests if building from a string works and if we can extract it perfectly.
        // Node 0 is dummy, Node 1='a', Node 2='b', Node 3='c'.
        let lc = LinkedChars::from_iter("abc".chars());
        assert!(
            !lc.is_empty(),
            "LinkedChars should not be empty after creation from iter"
        );

        // start_idx 0 means we start reading AFTER the dummy node.
        // end_idx 3 means we stop exactly after reading 'c'.
        let result = lc
            .interval_to_string(0, 3)
            .expect("interval_to_string failed");
        assert_eq!(result, "abc", "Extracted string should match input");
    }

    #[test]
    fn test_remove_between_middle() {
        // "hello" -> nodes 1(h), 2(e), 3(l), 4(l), 5(o).
        let mut lc = LinkedChars::from_iter("hello".chars());

        // Remove "ell". start_idx must be node 1 ('h'). end_idx must be node 4 (second 'l').
        // The chain should become: dummy(0) -> 'h'(1) -> 'o'(5).
        lc.remove_between(1, 4);

        // Reconstruct full string starting after dummy (0) up to the last known node (5).
        let result = lc
            .interval_to_string(0, 5)
            .expect("interval_to_string failed");
        assert_eq!(result, "ho", "Expected 'ell' to be removed, leaving 'ho'");
    }

    #[test]
    fn test_replace_between_with_longer_string() {
        // "hi" -> dummy(0), 'h'(1), 'i'(2).
        let mut lc = LinkedChars::from_iter("hi".chars());
        let replacement = LinkedChars::from_iter("ello".chars());

        // Replace 'i' (node 2) with "ello".
        // start_idx is 1 ('h'), end_idx is 2 ('i').
        lc.replace_between(1, 2, &replacement);

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
        lc.replace_between(1, 5, &empty_replacement);

        let result = lc
            .interval_to_string(0, 6)
            .expect("interval_to_string failed");
        assert_eq!(result, "de", "Replacing with empty should leave 'de'");
    }

    #[test]
    fn test_interval_to_string_error_on_invalid_bounds() {
        let lc = LinkedChars::from_iter("abc".chars());
        // Node 99 does not exist. The function should panic to prevent silent logic errors.
        let result = lc.interval_to_string(0, 99);
        assert!(
            result.is_err(),
            "Expected interval_to_string to return an error"
        );
    }

    #[test]
    fn test_index_to_char_pos() {
        let lc = LinkedChars::from_iter("abcd".chars());
        assert_eq!(lc.index_to_char_pos(1), Some(0));
        assert_eq!(lc.index_to_char_pos(2), Some(1));
        assert_eq!(lc.index_to_char_pos(4), Some(3));
        assert_eq!(lc.index_to_char_pos(99), None);
    }

    #[test]
    fn test_make_snippet_with_highlight() {
        let lc = LinkedChars::from_iter("hello world".chars());
        let snippet = lc.make_snippet(Some(6), 80);
        assert!(snippet.contains("hello world"));
        assert!(snippet.contains('^'));
    }

    #[test]
    fn test_make_snippet_without_highlight() {
        let lc = LinkedChars::from_iter("hello world".chars());
        let snippet = lc.make_snippet(None, 5);
        assert_eq!(snippet, "hello");
    }

    #[test]
    fn test_strip_outer_protection_layer() {
        let mut lc = LinkedChars::from_iter("[a[b]c]".chars());
        assert_eq!(lc.make_string(), "[a[b]c]");
        lc.strip_outer_protection_layer();
        assert_eq!(lc.make_string(), "a[b]c");
    }

    // ==========================================
    // Tests for Interpreter
    // ==========================================

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
            history: None,
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
            history: None,
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
            history: None,
        };
        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string(), "= > <".to_string());
    }

    #[test]
    fn define_and_call_functions_with_ghost_chars() {
        let lc = LinkedChars::from_iter(
            "def to_zeros {
                1(.*) => 0~to_zeros(^#1)
            ||       => }
            def inc_bin {
                (.*)0(1*) => #1~1~to_zeros(^#2)
            ||  (1+)      => 1~to_zeros(^#1)
            ||            => 1 }
            inc_bin(1011)"
                .chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
        };
        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "1100".to_string());
    }

    #[test]
    fn define_function_with_newlines() {
        let lc = LinkedChars::from_iter("def\nadd_positive { a => ok } add_positive(a)".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "ok");
    }

    #[test]
    fn function_call_using_ghost_char() {
        let lc = LinkedChars::from_iter("def f { (a) => ok } f(a)".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
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
            history: None,
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
            history: None,
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
            history: None,
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
            history: None,
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
            history: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected RegisterOutOfBounds error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::RegisterOutOfBounds { .. }));
    }

    #[test]
    fn test_register_call_trailing_whitespace_is_not_ignored() {
        let lc = LinkedChars::from_iter("{ a :: (a) => #1 1 }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "a 1");
    }

    #[test]
    fn test_register_calling_ghost_char() {
        let lc = LinkedChars::from_iter("{ a :: (a) => #1~1 }".chars());
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
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
            history: None,
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
            history: None,
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
            history: None,
        };

        let result = interpreter.evaluate();
        assert!(result.is_err(), "Expected MissingParentScope error");
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::MissingParentScope { .. }));
    }

    #[test]
    fn function_lookup_in_parent() {
        let lc = LinkedChars::from_iter(
            "def swap { (.)(.) => #2#1 } def swap_back { (.)(.) => #2#1 } swap(swap_back(ab))"
                .chars(),
        );
        let mut interpreter = Interpreter {
            state: lc,
            registers: vec![],
            functions: vec![],
            parent: None,
            history: None,
        };

        interpreter.evaluate().expect("Evaluation failed");
        assert_eq!(interpreter.state.make_string().trim(), "ab");
    }

    // ==========================================
    // Tests for Scope Evaluation
    // ==========================================

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
