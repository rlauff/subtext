use crate::error::ErrorKind;
use crate::interpreter::Interpreter;
use crate::linked_chars::LinkedChars;
use crate::scope::evaluate_scope;

// --- Helper ---

/// Creates a dummy interpreter to be used as a parent in scope tests.
fn dummy_interpreter() -> Interpreter<'static> {
    Interpreter::root(LinkedChars::new())
}

fn run(program: &str) -> Result<String, crate::error::SubtextError> {
    let mut interpreter = Interpreter::root(LinkedChars::from_iter(program.chars()));
    interpreter.evaluate()?;
    Ok(interpreter.state.make_string())
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
    lc.replace_between(1, 2, &replacement);

    // Since we pushed 4 new nodes to the arena, the last node index is 2 + 4 = 6.
    let result = lc
        .interval_to_string(0, 6)
        .expect("interval_to_string failed");
    assert_eq!(result, "hello");
}

#[test]
fn test_replace_between_with_empty() {
    let mut lc = LinkedChars::from_iter("delete".chars());
    let empty_replacement = LinkedChars::new();

    // Replacing "elet" (nodes 2..=5) with nothing should act like remove_between.
    lc.replace_between(1, 5, &empty_replacement);

    let result = lc
        .interval_to_string(0, 6)
        .expect("interval_to_string failed");
    assert_eq!(result, "de");
}

// ==========================================
// Tests for Interpreter
// ==========================================

#[test]
fn test_hello_world() {
    let result = run("{ hello, world! goodby, moon! :: (.*) => #1 }").expect("Evaluation failed");
    assert_eq!(result, "hello, world! goodby, moon!");
}

#[test]
fn define_and_call_function_nested() {
    let result =
        run("def f { a => hello, world! || b => g(b) }def g { a => f(b) || b => f(a) }f(b)")
            .expect("Evaluation failed");
    assert_eq!(result, "hello, world!");
}

#[test]
fn define_and_call_function_longer() {
    let result = run(
        "def longer { \n                    (.*)(.)&(.*)(.) => longer(^#1&^#3)\n                ||  .+&             => >\n                ||    &.+           => <\n                ||    &             => =}longer(abc&cde) longer(ab&c) longer(a&ab)",
    )
    .expect("Evaluation failed");
    assert_eq!(result.trim(), "= > <");
}

// ==========================================
// Tests for Scope Evaluation
// ==========================================

#[test]
fn test_evaluate_with_register_call() {
    let parent = dummy_interpreter();
    let scope = "world hello, :: (.....) (......) => #2 #1!".to_string();

    let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
    assert_eq!(result.make_string().trim(), "hello, world!");
}

#[test]
fn test_evaluate_with_register_call_nested() {
    let parent = dummy_interpreter();
    let scope =
        "{ world hello, moon! :: (.....) (......) (.*) => #2 #1! { Goodby, :: (.*) => #1 ^#3 } }"
            .to_string();

    let result = evaluate_scope(scope, &parent, None).expect("Scope evaluation failed");
    assert_eq!(result.make_string().trim(), "hello, world! Goodby, moon!");
}

#[test]
fn test_scope_without_separator_returns_input() {
    let parent = dummy_interpreter();
    let result =
        evaluate_scope("{ just some text }".to_string(), &parent, None).expect("must be legal");
    assert_eq!(result.make_string().trim(), "just some text");
}

// --- Error Case Tests ---

#[test]
fn test_no_match_returns_error() {
    let parent = dummy_interpreter();
    let scope = "input :: unknown => output".to_string();

    let err = evaluate_scope(scope, &parent, None).unwrap_err();
    match err.kind {
        ErrorKind::NoMatchingArm { arms, .. } => assert_eq!(arms, vec!["unknown".to_string()]),
        other => panic!("Expected NoMatchingArm, got {:?}", other),
    }
}

#[test]
fn test_malformed_arm_returns_error() {
    let parent = dummy_interpreter();
    let scope = "input :: pattern - output".to_string();

    let err = evaluate_scope(scope, &parent, None).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::MalformedArmMissingArrow { .. }),
        "Expected MalformedArmMissingArrow, got {:?}",
        err.kind
    );
}

#[test]
fn test_no_match_reports_untrimmed_hint() {
    let parent = dummy_interpreter();
    // '^a +$' matches "a  " (untrimmed) but not the trimmed input "a".
    let scope = "a  :: ^a +$ => out".to_string();

    let err = evaluate_scope(scope, &parent, None).unwrap_err();
    match err.kind {
        ErrorKind::NoMatchingArm {
            untrimmed_match, ..
        } => assert_eq!(untrimmed_match, Some(0)),
        other => panic!("Expected NoMatchingArm, got {:?}", other),
    }
}

#[test]
fn test_invalid_regex_names_the_arm() {
    let parent = dummy_interpreter();
    let scope = "x :: y => a || [ => b".to_string();

    let err = evaluate_scope(scope, &parent, None).unwrap_err();
    match err.kind {
        ErrorKind::InvalidRegex { arm_index, .. } => assert_eq!(arm_index, 1),
        other => panic!("Expected InvalidRegex, got {:?}", other),
    }
}

#[test]
fn test_undefined_function_suggests_similar_name() {
    let err = run("def zeroes { (.*) => 0#1 } zeros(3)").unwrap_err();
    match err.kind {
        ErrorKind::UndefinedFunction {
            name, suggestion, ..
        } => {
            assert_eq!(name, "zeros");
            assert_eq!(suggestion, Some("zeroes".to_string()));
        }
        other => panic!("Expected UndefinedFunction, got {:?}", other),
    }
}

#[test]
fn test_undefined_function_ghost_hint() {
    // classic mistake: `1zeroes(…)` is parsed as one call name; the fix is `1~zeroes(…)`.
    let err = run("def zeroes { (.*) => x } 1zeroes(3)").unwrap_err();
    match err.kind {
        ErrorKind::UndefinedFunction { ghost_hint, .. } => {
            assert_eq!(ghost_hint, Some("zeroes".to_string()));
        }
        other => panic!("Expected UndefinedFunction, got {:?}", other),
    }
}

#[test]
fn test_recursion_limit() {
    let err = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run("def f { x => f(x) } f(x)").unwrap_err())
        .unwrap()
        .join()
        .unwrap();
    assert!(
        matches!(err.kind, ErrorKind::RecursionLimitExceeded { .. }),
        "Expected RecursionLimitExceeded, got {:?}",
        err.kind
    );
    // the backtrace must exist, and the renderer must compress the ~1000 recursion frames
    // (identical consecutive frames collapse to ×N; anything left over is capped/omitted)
    assert!(!err.backtrace.is_empty());
    let rendered = err.to_string();
    assert!(
        rendered.contains("identical frames") || rendered.contains("omitted"),
        "long backtraces must be compressed:\n{}",
        rendered
    );
    assert!(
        rendered.lines().count() < 30,
        "a 1000-frame backtrace must not render 1000 frames:\n{}",
        rendered
    );
}

#[test]
fn test_missing_parent_scope_suggests_level() {
    let err = run("{ in :: (in) => ^^^^^^#1 }").unwrap_err();
    match err.kind {
        ErrorKind::MissingParentScope { suggestion, .. } => {
            assert_eq!(suggestion, Some("#1".to_string()));
        }
        other => panic!("Expected MissingParentScope, got {:?}", other),
    }
}
