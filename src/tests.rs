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

// ==========================================
// Tests for the standard library in std/
// ==========================================

/// Loads the whole standard library and evaluates `expr` after it.
/// Runs on a thread with a large stack, like the recursion-limit test above.
fn std_eval(expr: &str) -> String {
    let program = format!("get_file(std/all.stx)\n{}", expr);
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(&program).expect("std evaluation failed"))
        .unwrap()
        .join()
        .unwrap()
        .trim()
        .to_string()
}

/// Asserts that each call evaluates to the expected text.
fn assert_std(cases: &[(&str, &str)]) {
    for (call, expected) in cases {
        // the result is wrapped in < > so that empty results are visible
        assert_eq!(
            std_eval(&format!("<~{}>", call)),
            format!("<{}>", expected),
            "{}",
            call
        );
    }
}

#[test]
fn test_std_add_sub() {
    assert_std(&[
        ("add(19, 3)", "22"),
        ("add(999, 1)", "1000"),
        ("add(0, 0)", "0"),
        ("add(007, 3)", "10"),
        ("add(-5, 3)", "-2"),
        ("add(5, -8)", "-3"),
        ("add(-2, -3)", "-5"),
        ("add(1,2)", "3"),
        ("sub(10, 3)", "7"),
        ("sub(3, 10)", "-7"),
        ("sub(1000, 1)", "999"),
        ("sub(100, 100)", "0"),
        ("sub(-3, -10)", "7"),
        ("sub(-3, 4)", "-7"),
        ("sub(3, -4)", "7"),
    ]);
}

#[test]
fn test_std_inc_dec_cmp() {
    assert_std(&[
        ("inc(129)", "130"),
        ("inc(99)", "100"),
        ("inc(-1)", "0"),
        ("inc(-10)", "-9"),
        ("dec(130)", "129"),
        ("dec(10)", "9"),
        ("dec(1)", "0"),
        ("dec(0)", "-1"),
        ("dec(-9)", "-10"),
        ("cmp(3, 5)", "<"),
        ("cmp(5, 3)", ">"),
        ("cmp(12, 12)", "="),
        ("cmp(-5, 3)", "<"),
        ("cmp(-2, -10)", ">"),
        ("min(4, -2)", "-2"),
        ("max(4, -2)", "4"),
    ]);
}

#[test]
fn test_std_mul_div_mod() {
    assert_std(&[
        ("mul(123456789, 987654321)", "121932631112635269"),
        ("mul(-3, 4)", "-12"),
        ("mul(-3, -4)", "12"),
        ("mul(0, -5)", "0"),
        ("div(1234567, 89)", "13871"),
        ("mod(1234567, 89)", "48"),
        ("div(3, 10)", "0"),
        // floored division: the remainder has the sign of the divisor
        ("div(7, 2)", "3"),
        ("mod(7, 2)", "1"),
        ("div(-7, 2)", "-4"),
        ("mod(-7, 2)", "1"),
        ("div(7, -2)", "-4"),
        ("mod(7, -2)", "-1"),
        ("div(-7, -2)", "3"),
        ("mod(-7, -2)", "-1"),
        ("mod(-100, 5)", "0"),
    ]);
}

#[test]
fn test_std_div_by_zero_is_an_error() {
    let err = run("get_file(std/all.stx)\ndiv(1, 0)").unwrap_err();
    match err.kind {
        ErrorKind::NoMatchingArm { input, .. } => assert_eq!(input, "division by zero"),
        other => panic!("Expected NoMatchingArm, got {:?}", other),
    }
}

#[test]
fn test_std_strings() {
    assert_std(&[
        ("len(hello world)", "11"),
        ("len()", "0"),
        ("len(abcdefghij)", "10"),
        ("len(a  b)", "4"),
        ("rev(hello  world)", "dlrow  olleh"),
        ("rev()", ""),
        ("upper(Hello, World 42!)", "HELLO, WORLD 42!"),
        ("lower(Hello, World 42!)", "hello, world 42!"),
        ("repeat(ab, 3)", "ababab"),
        ("repeat(a b, 2)", "a ba b"),
        ("repeat(x, 0)", ""),
        ("slice(hello world, 0, 5)", "hello"),
        ("slice(hello world, 6, 100)", "world"),
        ("slice(hello, 3, 1)", ""),
        ("slice(hello, 9, 12)", ""),
    ]);
}

#[test]
fn test_std_search() {
    assert_std(&[
        ("eq(abc, abc)", "y"),
        ("eq(abc, abd)", "n"),
        ("eq(a.c, abc)", "n"),
        ("eq(1+1, 1+1)", "y"),
        ("eq(a b, a b)", "y"),
        ("eq(a, )", "n"),
        ("contains(hello world, o w)", "y"),
        ("contains(hello, z)", "n"),
        ("contains(ab, .)", "n"),
        ("contains(a.b, .)", "y"),
        ("index_of(hello world, o)", "4"),
        ("index_of(hello world, world)", "6"),
        ("index_of(hello, z)", "-1"),
        ("index_of(a, b, c, c)", "6"),
    ]);
}

#[test]
fn test_std_lists() {
    assert_std(&[
        ("split(a-b-c, -)", "|a|b|c"),
        ("split(hello big world, )", "|hello|big|world"),
        ("split(a.b, .)", "|a|b"),
        ("split(a--b--, --)", "|a|b|"),
        ("join(|a|b|c, +)", "a+b+c"),
        ("join(|hello|world, )", "hello world"),
        ("join(, -)", ""),
        ("count(|a|b|c)", "3"),
        ("count()", "0"),
        ("count(|a||c)", "3"),
        ("nth(|a|b|c, 0)", "a"),
        ("nth(|a|b|c, 2)", "c"),
        (
            "join(split(the quick brown fox, ), _)",
            "the_quick_brown_fox",
        ),
    ]);
}

#[test]
fn test_std_long_inputs_do_not_hit_the_recursion_limit() {
    // the string and list loops hand their next step back to the caller,
    // so they stay at constant depth even for inputs far longer than the limit
    let text = "ab cd".repeat(400);
    assert_eq!(std_eval(&format!("len({})", text)), "2000");
    assert_eq!(std_eval(&format!("count(split({}, ))", text)), "401");
}
