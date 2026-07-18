use crate::linked_chars::Snippet;
use std::fmt;

// Every ErrorKind now provides four pieces — code(), headline(), details(), help() — and a single renderer
// produces a uniform rustc-style layout:
//
//   error[<code>]: <headline>
//     <label>: <value>
//   help: <hint>
//   backtrace (innermost first):
//     0 │ <context>
//       │ <snippet>
//       │ <caret>
//       │ registers  #1=⟨…⟩
//       │ functions  a, b
//

/// Maximum characters shown for an embedded value (register contents, inputs, patterns).
const VALUE_MAX: usize = 60;
/// Snippet width used for backtrace frames.
pub const SNIPPET_MAX: usize = 80;
/// Backtrace capping: print the innermost / outermost frames, omit the middle.
const FRAMES_HEAD: usize = 5;
const FRAMES_TAIL: usize = 3;

/// A snapshot of the interpreter's state at a specific level in the call stack.
#[derive(Debug, Clone)]
pub struct BacktraceFrame {
    pub depth: usize,
    pub context: String,
    pub state_snippet: Snippet,
    pub registers: Vec<String>,
    pub defined_functions: Vec<String>,
}

/// Specific failure points that can occur during subtext execution.
#[derive(Debug, Clone)]
pub enum ErrorKind {
    // Syntax & Parsing Errors
    UnmatchedOpeningBrace {
        expected_closing: char,
        opened_at: usize, // arena index, used only for the caret
    },
    UnmatchedClosingBrace {
        found: char,
        position: usize, // NOTE: string offset when raised while splitting a scope; not printed
    },
    MissingRegisterDigit {
        position: usize,
    },
    RegisterIndexStartsAtOne {
        position: usize,
    },
    MissingFunctionName {
        position: usize,
    },
    MissingFunctionBody {
        position: usize,
    },
    MalformedArmMissingArrow {
        arm_content: String,
    },

    // Runtime & Evaluation Errors
    UndefinedFunction {
        name: String,
        visible: Vec<String>,
        suggestion: Option<String>,
        ghost_hint: Option<String>,
    },
    InvalidRegex {
        pattern: String,
        reason: String,
        arm_index: usize,
    },
    NoMatchingArm {
        input: String,
        arms: Vec<String>,
        untrimmed_match: Option<usize>,
    },

    // Smart Register Errors
    RegisterOutOfBounds {
        requested: usize,
        available: usize,
        suggestion: Option<String>,
    },
    MissingParentScope {
        requested_level: usize,
        actual_depth: usize,
        suggestion: Option<String>,
    },
    RecursionLimitExceeded {
        limit: usize,
    },

    // I/O Errors
    FileReadError {
        path: String,
        reason: String,
    },
    InputReadError {
        reason: String,
    },
    OutputWriteError {
        reason: String,
    },

    // Internal Safeguards
    InternalInvariant {
        message: String,
    },
}

pub(crate) fn fmt_value(s: &str, max_chars: usize) -> String {
    let sanitized: String = s.chars().map(crate::linked_chars::sanitize_char).collect();
    let count = sanitized.chars().count();
    if count <= max_chars {
        format!("⟨{}⟩", sanitized)
    } else {
        let cut: String = sanitized.chars().take(max_chars).collect();
        format!("⟨{}…⟩ ({} chars)", cut, count)
    }
}

pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub_cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + sub_cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

// One entry per error code; used by `subtext --explain`, by the web
// frontend, and by a completeness test. Keep in sync with ErrorKind::code().
pub const ALL_CODES: &[&str] = &[
    "unmatched-opening-brace",
    "unmatched-closing-brace",
    "missing-register-digit",
    "register-index-starts-at-one",
    "missing-function-name",
    "missing-function-body",
    "arm-missing-arrow",
    "undefined-function",
    "invalid-regex",
    "no-matching-arm",
    "register-out-of-bounds",
    "missing-parent-scope",
    "recursion-limit-exceeded",
    "file-read-error",
    "input-read-error",
    "output-write-error",
    "internal-invariant",
];

// the hints for `--explain`. A self-contained explanation per error
// code: what it means, why it typically happens, and a minimal example with the fix.
// Reachable via `subtext --explain <code>` on the CLI and the "explain" button in the web
// terminal
pub fn explain(code: &str) -> Option<&'static str> {
    Some(match code {
        "unmatched-opening-brace" => "The interpreter reached the end of the current text while still looking for a closing
brace or parenthesis. Every '{' must be closed by '}' and every '(' by ')' — including
inside function-call arguments, where the argument only ends at the parenthesis that
balances the one that opened the call.

A subtle variant: passing unbalanced bracket characters as *data*, e.g. f(a(b), makes the
call swallow the rest of the program while searching for the balancing ')'. Use different
symbols for data (e.g. < and >), or balance them.",

        "unmatched-closing-brace" => "A '}' or ')' appeared with no opener to match it. Since scopes and calls are delimited
purely by balanced brackets, a stray closer usually means an opener was deleted by an
earlier rewrite — remember that outputs replace text verbatim, so an arm output like
'}' will happily break the surrounding structure.",

        "missing-register-digit" => "A '#' must be followed by digits: '#1' is the first capture group of the arm that
matched. A bare '#' has no meaning. If you want a literal '#' in a pattern, escape it as
'\\#'; if you want one in an output, there is currently no escaping — restructure so the
'#' is produced by a match instead.",

        "register-index-starts-at-one" => "Registers are numbered like regex capture groups: #1 is the first group. There is no #0
(the 'whole match' register does not exist); capture the whole input explicitly with
'(.*)' and use #1.",

        "missing-function-name" => "'def' must be followed by a name and a body: def name { arms }. The name is a word
(letters, digits, underscores).",

        "missing-function-body" => "After 'def name' the interpreter expects '{' to open the function body. The body has the
same shape as a scope after the '::' — arms of 'pattern => output' separated by '||'.",

        "arm-missing-arrow" => "Every arm must contain the separator '=>': 'pattern => output'. An arm may have an empty
pattern (matches the empty string — the usual base case of a recursion: '|| =>') or an
empty output, but the arrow itself is mandatory. This error often means a '||' split the
scope somewhere you did not expect — for instance inside data that contains two
consecutive '|' characters.",

        "undefined-function" => "Something that looks like a call — a word directly followed by '(' — refers to a function
that is not defined in the current scope or any enclosing one. Three common causes:

1. A typo; the message suggests near matches.
2. Merged text: everything is text and text merges, so '0zeroes(x)' is a call to a
   function named '0zeroes'. Separate emitted text from a following call with the ghost
   char: '0~zeroes(x)'.
3. A register substitution glued a value onto the name, e.g. '#2mul(…)' after
   substitution becomes 'IImul(…)'. Note that a register call swallows one following
   ghost char (that is what makes '#1~1' work), so '#2~mul(…)' has the same problem —
   place the call first and the text after it instead: 'mul(…)#2'.",

        "invalid-regex" => "The pattern of the named arm is not a valid regular expression. Patterns use Rust's regex
crate: no backreferences, no lookaround. The regex engine's own message (with its caret)
is included. Remember that patterns are taken verbatim — there is no escaping layer on
top — so characters like '(' group and '|' alternate; a single '|' inside a pattern is
regex alternation, while '||' at the top level of a scope separates arms.",

        "no-matching-arm" => "The scope's input matched none of its arms' patterns. The message lists the evaluated
input (delimited by ⟨…⟩, with whitespace shown as ␤ and ␉) and every pattern that was
tried, in order.

The most common cause: input and output of a scope are trimmed before matching, but
*internal* whitespace is kept and must be matched explicitly. If some arm matches the
untrimmed input, the message says so. Also remember that patterns match anywhere in the
input unless anchored — use '^…$' when the whole input must match — and that arms are
tried top to bottom, so an earlier, more general arm can shadow a later one.",

        "register-out-of-bounds" => "A register with this index does not exist at the referenced level. Registers are created
by the capture groups of the arm that matched — three groups give #1..#3, and a group
that did not participate in the match yields no register. Each '^' prefix moves the
lookup one scope up: a call argument is evaluated one scope deeper than the arm output it
appears in, so inside 'f(…)' you usually want '^#1'; inside 'f(g(…))' the innermost
argument is two levels deep and needs '^^#1'. If the register exists further up, the
message suggests the corrected prefix.",

        "missing-parent-scope" => "A register call climbs more scopes ('^' prefixes) than actually enclose it. Count one
level per nesting of call arguments and scopes, starting at the arm output where the
registers live. If the register exists at a lower level, the message suggests the
corrected number of '^'.",

        "recursion-limit-exceeded" => "Evaluation nested deeper than the interpreter's recursion limit. Almost always this means
a recursion without a reachable base case — check that some arm terminates and that it is
reachable (arms are tried in order; an earlier arm that always matches shadows the base
case; unanchored patterns match more than you might think). Use debug(…) around the call
to watch the rewrites. The limit also bounds legitimate very deep recursions; if you hit
it with a correct program, restructure toward shallower nesting.",

        "file-read-error" => "get_file(path) could not read the file. On the web version there is no file system, so
get_file always fails there; it is meant for the command-line interpreter.",

        "input-read-error" => "get_input(prompt) could not read from standard input. On the web version there is no
stdin, so get_input always fails there; it is meant for the command-line interpreter.",

        "output-write-error" => "Writing to standard output failed. This is an environment problem (closed pipe, full
disk), not a problem with your program.",

        "internal-invariant" => "An internal consistency check of the interpreter failed. This is a bug in subtext, not in
your program — please report it together with the program that triggered it.",

        _ => return None,
    })
}

impl ErrorKind {
    // Stable, greppable identifiers, printed as `error[<code>]` and usable
    // in tests and by the web frontend.
    pub fn code(&self) -> &'static str {
        match self {
            ErrorKind::UnmatchedOpeningBrace { .. } => "unmatched-opening-brace",
            ErrorKind::UnmatchedClosingBrace { .. } => "unmatched-closing-brace",
            ErrorKind::MissingRegisterDigit { .. } => "missing-register-digit",
            ErrorKind::RegisterIndexStartsAtOne { .. } => "register-index-starts-at-one",
            ErrorKind::MissingFunctionName { .. } => "missing-function-name",
            ErrorKind::MissingFunctionBody { .. } => "missing-function-body",
            ErrorKind::MalformedArmMissingArrow { .. } => "arm-missing-arrow",
            ErrorKind::UndefinedFunction { .. } => "undefined-function",
            ErrorKind::InvalidRegex { .. } => "invalid-regex",
            ErrorKind::NoMatchingArm { .. } => "no-matching-arm",
            ErrorKind::RegisterOutOfBounds { .. } => "register-out-of-bounds",
            ErrorKind::MissingParentScope { .. } => "missing-parent-scope",
            ErrorKind::RecursionLimitExceeded { .. } => "recursion-limit-exceeded",
            ErrorKind::FileReadError { .. } => "file-read-error",
            ErrorKind::InputReadError { .. } => "input-read-error",
            ErrorKind::OutputWriteError { .. } => "output-write-error",
            ErrorKind::InternalInvariant { .. } => "internal-invariant",
        }
    }

    /// One-line summary printed after `error[<code>]:`.
    fn headline(&self) -> String {
        match self {
            ErrorKind::UnmatchedOpeningBrace {
                expected_closing, ..
            } => format!("reached the end while looking for '{}'", expected_closing),
            ErrorKind::UnmatchedClosingBrace { found, .. } => {
                format!("found '{}' with no matching opener", found)
            }
            ErrorKind::MissingRegisterDigit { .. } => "expected digits after '#'".to_string(),
            ErrorKind::RegisterIndexStartsAtOne { .. } => {
                "registers are 1-indexed; there is no '#0'".to_string()
            }
            ErrorKind::MissingFunctionName { .. } => {
                "expected a function name after 'def'".to_string()
            }
            ErrorKind::MissingFunctionBody { .. } => {
                "expected '{' to start a function body".to_string()
            }
            ErrorKind::MalformedArmMissingArrow { .. } => {
                "an arm is missing the output separator '=>'".to_string()
            }
            ErrorKind::UndefinedFunction { name, .. } => {
                format!("call to undefined function '{}'", name)
            }
            ErrorKind::InvalidRegex { arm_index, .. } => {
                format!("the pattern of arm {} is not a valid regex", arm_index + 1)
            }
            ErrorKind::NoMatchingArm { arms, .. } => {
                format!("none of the {} arm(s) matched the input", arms.len())
            }
            ErrorKind::RegisterOutOfBounds {
                requested,
                available,
                ..
            } => {
                if *available == 0 {
                    format!(
                        "register #{} requested, but no registers exist here",
                        requested
                    )
                } else {
                    format!(
                        "register #{} requested, but only #1..#{} exist here",
                        requested, available
                    )
                }
            }
            ErrorKind::MissingParentScope {
                requested_level,
                actual_depth,
                ..
            } => format!(
                "register call goes {} scope(s) up, but only {} parent scope(s) exist",
                requested_level, actual_depth
            ),
            ErrorKind::RecursionLimitExceeded { limit } => {
                format!("recursion limit of {} nested evaluations exceeded", limit)
            }
            ErrorKind::FileReadError { path, .. } => {
                format!("failed to read file '{}'", path)
            }
            ErrorKind::InputReadError { .. } => "failed to read input".to_string(),
            ErrorKind::OutputWriteError { .. } => "failed to write output".to_string(),
            ErrorKind::InternalInvariant { .. } => {
                "an interpreter invariant was violated (this is a bug in subtext)".to_string()
            }
        }
    }

    /// Indented `label: value` lines below the headline.
    fn details(&self) -> Vec<(String, String)> {
        match self {
            ErrorKind::MalformedArmMissingArrow { arm_content } => {
                vec![("arm".to_string(), fmt_value(arm_content, VALUE_MAX))]
            }
            ErrorKind::UndefinedFunction { visible, .. } => {
                if visible.is_empty() {
                    vec![]
                } else {
                    vec![("visible functions".to_string(), visible.join(", "))]
                }
            }
            ErrorKind::InvalidRegex {
                pattern, reason, ..
            } => {
                let mut details = vec![("pattern".to_string(), fmt_value(pattern, VALUE_MAX))];
                // the regex crate's own multi-line error (which contains its
                // own caret diagram) is indented verbatim instead of flattened to one line.
                for (i, line) in reason.lines().enumerate() {
                    let label = if i == 0 { "reason" } else { "" };
                    details.push((label.to_string(), line.to_string()));
                }
                details
            }
            ErrorKind::NoMatchingArm { input, arms, .. } => {
                let mut details = vec![("input".to_string(), fmt_value(input, VALUE_MAX))];
                for (i, pattern) in arms.iter().enumerate() {
                    details.push((format!("arm {}", i + 1), fmt_value(pattern, VALUE_MAX)));
                }
                details
            }
            ErrorKind::FileReadError { reason, .. }
            | ErrorKind::InputReadError { reason }
            | ErrorKind::OutputWriteError { reason } => {
                vec![("reason".to_string(), reason.clone())]
            }
            ErrorKind::InternalInvariant { message } => {
                vec![("details".to_string(), message.clone())]
            }
            _ => vec![],
        }
    }

    // rustc-style labeled spans. Width and label of the caret drawn in
    // the innermost backtrace frame, e.g. `^^^ requested #9, only #1..#2 exist here`. The
    // width is clamped to the snippet by the renderer.
    fn caret_info(&self) -> (usize, Option<String>) {
        match self {
            ErrorKind::UnmatchedOpeningBrace { .. } => {
                (1, Some("opened here, never closed".to_string()))
            }
            ErrorKind::MissingRegisterDigit { .. } => {
                (1, Some("expected digits after this".to_string()))
            }
            ErrorKind::RegisterIndexStartsAtOne { .. } => (2, Some("there is no #0".to_string())),
            ErrorKind::MissingFunctionName { .. } => {
                (3, Some("expected a name after this".to_string()))
            }
            ErrorKind::MissingFunctionBody { .. } => {
                (1, Some("expected '{' after this".to_string()))
            }
            ErrorKind::UndefinedFunction { name, .. } => (
                name.chars().count().max(1),
                Some("not defined in any enclosing scope".to_string()),
            ),
            ErrorKind::NoMatchingArm { .. } => (1, Some("no arm matched here".to_string())),
            ErrorKind::InvalidRegex { arm_index, .. } => (
                1,
                Some(format!(
                    "arm {} of this scope has the bad pattern",
                    arm_index + 1
                )),
            ),
            ErrorKind::MalformedArmMissingArrow { .. } => (1, Some("in this scope".to_string())),
            ErrorKind::RegisterOutOfBounds {
                requested,
                available,
                ..
            } => {
                let len = 1 + requested.to_string().chars().count();
                let label = if *available == 0 {
                    format!("requested #{}, but no registers exist here", requested)
                } else {
                    format!(
                        "requested #{}, only #1..#{} exist here",
                        requested, available
                    )
                };
                (len, Some(label))
            }
            ErrorKind::MissingParentScope {
                requested_level,
                actual_depth,
                ..
            } => {
                // the stored position anchors at the '#', so the span covers
                // '#n' and the label carries the level information.
                (
                    2,
                    Some(format!(
                        "goes {} scope(s) up, only {} exist",
                        requested_level, actual_depth
                    )),
                )
            }
            _ => (1, None),
        }
    }

    fn notes(&self) -> Vec<String> {
        match self {
            ErrorKind::NoMatchingArm {
                untrimmed_match: Some(i),
                ..
            } => vec![format!(
                "arm {} matches the input before trimming; input and output of a scope are trimmed before matching",
                i + 1
            )],
            ErrorKind::RecursionLimitExceeded { .. } => {
                vec!["this usually means a recursion without a reachable base case".to_string()]
            }
            _ => vec![],
        }
    }

    /// Optional `help:` line.
    fn help(&self) -> Option<String> {
        match self {
            ErrorKind::MissingRegisterDigit { .. } => {
                Some("register calls look like '#1'; use '\\#' in a pattern to match a literal '#'".to_string())
            }
            ErrorKind::RegisterIndexStartsAtOne { .. } => {
                Some("use '#1' for the first capture group".to_string())
            }
            ErrorKind::MalformedArmMissingArrow { .. } => {
                Some("arms are written as 'pattern => output'".to_string())
            }
            ErrorKind::UndefinedFunction {
                suggestion,
                ghost_hint,
                ..
            } => match (suggestion, ghost_hint) {
                (_, Some(ghost)) => Some(format!(
                    "'{}' is defined; use the ghost char '~' to separate it from the preceding text: '…~{}(…)'",
                    ghost, ghost
                )),
                (Some(s), None) => Some(format!("did you mean '{}'?", s)),
                (None, None) => None,
            },
            ErrorKind::NoMatchingArm { .. } => Some(
                "whitespace inside the input (shown as ␤ ␉) must be matched explicitly; anchor patterns with ^…$ if the whole input must match"
                    .to_string(),
            ),
            ErrorKind::RegisterOutOfBounds { suggestion, .. } => suggestion
                .as_ref()
                .map(|hint| format!("a parent scope has this register; did you mean '{}'?", hint)),
            ErrorKind::MissingParentScope { suggestion, .. } => match suggestion {
                Some(hint) => Some(format!(
                    "a lower scope has this register; did you mean '{}'?",
                    hint
                )),
                None => Some("reduce the number of '^' prefixes on the register call".to_string()),
            },
            ErrorKind::RecursionLimitExceeded { .. } => Some(
                "the program probably recurses without reaching a base case; use debug(…) to trace the rewrites"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

/// The main error struct holding the specific error kind and the backtrace.
#[derive(Debug, Clone)]
pub struct SubtextError {
    pub kind: ErrorKind,
    pub backtrace: Vec<BacktraceFrame>,
}

impl SubtextError {
    /// Creates a new error without a backtrace yet.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Vec::new(),
        }
    }
}

fn write_frame(
    f: &mut fmt::Formatter<'_>,
    frame: &BacktraceFrame,
    repeat: usize,
    caret_info: &(usize, Option<String>),
) -> fmt::Result {
    let head = format!("{:>3} │ ", frame.depth);
    let gutter = " ".repeat(head.len() - 2) + "│ ";

    let repeat_note = if repeat > 1 {
        format!("   (×{} identical frames)", repeat)
    } else {
        String::new()
    };
    writeln!(f, "{}{}{}", head, frame.context, repeat_note)?;

    if !frame.state_snippet.text.is_empty() {
        let pre = if frame.state_snippet.clipped_start {
            "…"
        } else {
            ""
        };
        let post = if frame.state_snippet.clipped_end {
            "…"
        } else {
            ""
        };
        writeln!(f, "{}{}{}{}", gutter, pre, frame.state_snippet.text, post)?;
        if let Some(caret) = frame.state_snippet.caret {
            // the '…' prefix is one char wide, keep the caret aligned under the snippet
            let offset = caret + usize::from(frame.state_snippet.clipped_start);
            // the caret is as wide as the offending
            // token (clamped to the snippet) and carries a short message.
            let text_len = frame.state_snippet.text.chars().count();
            let width = caret_info.0.min(text_len.saturating_sub(caret)).max(1);
            let label = match &caret_info.1 {
                Some(label) => format!(" {}", label),
                None => String::new(),
            };
            writeln!(
                f,
                "{}{}{}{}",
                gutter,
                " ".repeat(offset),
                "^".repeat(width),
                label
            )?;
        }
    }

    if !frame.registers.is_empty() {
        let regs: Vec<String> = frame
            .registers
            .iter()
            .enumerate()
            .map(|(i, val)| format!("#{}={}", i + 1, fmt_value(val, 40)))
            .collect();
        writeln!(f, "{}registers  {}", gutter, regs.join(" "))?;
    }
    if !frame.defined_functions.is_empty() {
        writeln!(
            f,
            "{}functions  {}",
            gutter,
            frame.defined_functions.join(", ")
        )?;
    }
    Ok(())
}

impl fmt::Display for SubtextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 1. headline
        writeln!(f, "error[{}]: {}", self.kind.code(), self.kind.headline())?;

        if let Some(frame) = self.backtrace.first() {
            writeln!(f, "  --> in {}", frame.context)?;
        }

        // 2. details
        let details = self.kind.details();
        let label_width = details
            .iter()
            .map(|(l, _)| l.chars().count())
            .max()
            .unwrap_or(0);
        for (label, value) in &details {
            writeln!(
                f,
                "  {:>width$}{} {}",
                label,
                if label.is_empty() { " " } else { ":" },
                value,
                width = label_width
            )?;
        }

        // 3. notes and help (rustc-style: notes state facts, help suggests the fix)
        for note in self.kind.notes() {
            writeln!(f, "note: {}", note)?;
        }
        if let Some(help) = self.kind.help() {
            writeln!(f, "help: {}", help)?;
        }

        // 4. backtrace
        if !self.backtrace.is_empty() {
            writeln!(f, "backtrace (innermost first):")?;

            let mut collapsed: Vec<(&BacktraceFrame, usize)> = Vec::new();
            for frame in &self.backtrace {
                // frames with no information at all are skipped entirely
                if frame.state_snippet.text.is_empty()
                    && frame.registers.is_empty()
                    && frame.defined_functions.is_empty()
                {
                    continue;
                }
                match collapsed.last_mut() {
                    Some((prev, count))
                        if prev.context == frame.context
                            && prev.state_snippet.text == frame.state_snippet.text
                            && prev.registers == frame.registers =>
                    {
                        *count += 1;
                    }
                    _ => collapsed.push((frame, 1)),
                }
            }

            let caret_info = self.kind.caret_info();
            if collapsed.len() <= FRAMES_HEAD + FRAMES_TAIL + 1 {
                for (frame, repeat) in &collapsed {
                    write_frame(f, frame, *repeat, &caret_info)?;
                }
            } else {
                for (frame, repeat) in &collapsed[..FRAMES_HEAD] {
                    write_frame(f, frame, *repeat, &caret_info)?;
                }
                let omitted = collapsed.len() - FRAMES_HEAD - FRAMES_TAIL;
                writeln!(f, "    │ … {} frame(s) omitted …", omitted)?;
                for (frame, repeat) in &collapsed[collapsed.len() - FRAMES_TAIL..] {
                    write_frame(f, frame, *repeat, &caret_info)?;
                }
            }
        }

        Ok(())
    }
}

impl std::error::Error for SubtextError {}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_value_short() {
        assert_eq!(fmt_value("abc", 10), "⟨abc⟩");
    }

    #[test]
    fn test_fmt_value_sanitizes_and_truncates() {
        assert_eq!(fmt_value("a\nb", 10), "⟨a␤b⟩");
        assert_eq!(fmt_value("abcdef", 3), "⟨abc…⟩ (6 chars)");
    }

    #[test]
    fn test_edit_distance() {
        assert_eq!(edit_distance("zeroes", "zeroes"), 0);
        assert_eq!(edit_distance("zeros", "zeroes"), 1);
        assert_eq!(edit_distance("abc", ""), 3);
    }

    #[test]
    fn test_display_contains_code_and_help() {
        let err = SubtextError::new(ErrorKind::NoMatchingArm {
            input: "foo".to_string(),
            arms: vec!["bar".to_string(), "baz".to_string()],
            untrimmed_match: None,
        });
        let rendered = err.to_string();
        assert!(rendered.starts_with("error[no-matching-arm]:"));
        assert!(rendered.contains("input: ⟨foo⟩"));
        assert!(rendered.contains("arm 1: ⟨bar⟩"));
        assert!(rendered.contains("help:"));
    }

    #[test]
    fn test_explain_covers_all_codes() {
        for code in ALL_CODES {
            assert!(explain(code).is_some(), "missing explanation for {}", code);
        }
        assert!(explain("no-such-code").is_none());
    }

    #[test]
    fn test_labeled_caret_span() {
        use crate::linked_chars::LinkedChars;
        let lc = LinkedChars::from_iter("abc ^^#1 xyz".chars());
        let mut err = SubtextError::new(ErrorKind::MissingParentScope {
            requested_level: 2,
            actual_depth: 0,
            suggestion: Some("#1".to_string()),
        });
        err.backtrace.push(BacktraceFrame {
            depth: 0,
            context: "output of scope".to_string(),
            state_snippet: lc.make_snippet(Some(5), 80), // caret at the first '^'
            registers: vec![],
            defined_functions: vec![],
        });
        let rendered = err.to_string();
        assert!(rendered.contains("--> in output of scope"), "{}", rendered);
        assert!(
            rendered.contains("^^ goes 2 scope(s) up, only 0 exist"),
            "{}",
            rendered
        );
    }

    #[test]
    fn test_display_no_raw_indices() {
        let err = SubtextError::new(ErrorKind::MissingRegisterDigit { position: 42 });
        let rendered = err.to_string();
        assert!(
            !rendered.contains("42"),
            "raw positions must not be printed: {}",
            rendered
        );
    }
}
