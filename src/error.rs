use crate::linked_chars::LinkedChars;
use std::fmt;

/// A snapshot of the interpreter's state at a specific level in the call stack.
/// This contains as much context as possible for debugging and error reporting.
#[derive(Debug, Clone)]
pub struct BacktraceFrame {
    pub depth: usize,
    pub full_state: LinkedChars,
    pub state_snippet: String,
    pub registers: Vec<String>,
    pub defined_functions: Vec<String>,
}

/// Specific failure points that can occur during subtext execution.
#[derive(Debug, Clone)]
pub enum ErrorKind {
    // Syntax & Parsing Errors
    UnmatchedOpeningBrace {
        expected_closing: char,
        opened_at: usize,
    },
    UnmatchedClosingBrace {
        found: char,
        position: usize,
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
    MalformedScopeMissingInputSeparator {
        scope_content: String,
    },
    MalformedArmMissingArrow {
        arm_content: String,
    },

    // Runtime & Evaluation Errors
    UndefinedFunction {
        name: String,
    },
    InvalidRegex {
        pattern: String,
        reason: String,
    },
    NoMatchingArm {
        input: String,
        scope_content: String,
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

/// The main error struct holding the specific error kind and the rich backtrace.
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

    /// Adds a snapshot of the current scope to the backtrace.
    pub fn push_frame(&mut self, frame: BacktraceFrame) {
        self.backtrace.push(frame);
    }
}

impl fmt::Display for SubtextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 1. Print the primary error message
        match &self.kind {
            ErrorKind::UnmatchedOpeningBrace {
                expected_closing,
                opened_at,
            } => {
                writeln!(
                    f,
                    "Syntax Error: Reached end of file while looking for '{}' (opened at index {}).",
                    expected_closing, opened_at
                )?;
            }
            ErrorKind::MissingRegisterDigit { position } => {
                writeln!(
                    f,
                    "Syntax Error: Expected digits after '#'. Example: '#1' (at index {}).",
                    position
                )?;
            }
            ErrorKind::RegisterIndexStartsAtOne { position } => {
                writeln!(
                    f,
                    "Syntax Error: Registers are 1-indexed. Use '#1' for the first register (at index {}).",
                    position
                )?;
            }
            ErrorKind::MissingFunctionName { position } => {
                writeln!(
                    f,
                    "Syntax Error: Expected a function name after 'def' (at index {}).",
                    position
                )?;
            }
            ErrorKind::MissingFunctionBody { position } => {
                writeln!(
                    f,
                    "Syntax Error: Expected '{{' to start a function body (at index {}).",
                    position
                )?;
            }
            ErrorKind::MalformedScopeMissingInputSeparator { scope_content } => {
                writeln!(
                    f,
                    "Syntax Error: Scope is missing the input separator '::'.",
                )?;
                writeln!(f, "Scope snippet: {}", scope_content)?;
                writeln!(
                    f,
                    "Help: Scopes are written as '{{ input :: pattern => output || ... }}'."
                )?;
            }
            ErrorKind::MalformedArmMissingArrow { arm_content } => {
                writeln!(
                    f,
                    "Syntax Error: An evaluation arm is missing the output separator '=>'."
                )?;
                writeln!(f, "Arm snippet: {}", arm_content)?;
                writeln!(f, "Help: Arms are written as 'pattern => output'.")?;
            }
            ErrorKind::UnmatchedClosingBrace { found, position } => {
                writeln!(
                    f,
                    "Syntax Error: Found '{}' at index {} with no matching opener.",
                    found, position
                )?;
            }
            ErrorKind::UndefinedFunction { name } => {
                writeln!(f, "Runtime Error: Call to undefined function '{}'.", name)?;
                if let Some(frame) = self.backtrace.first()
                    && !frame.defined_functions.is_empty()
                {
                    writeln!(f, "Known functions here: {:?}", frame.defined_functions)?;
                }
            }
            ErrorKind::InvalidRegex { pattern, reason } => {
                writeln!(f, "Regex Error: The pattern '{}' is invalid.", pattern)?;
                writeln!(f, "Reason: {}", reason)?;
            }
            ErrorKind::NoMatchingArm {
                input,
                scope_content,
            } => {
                writeln!(
                    f,
                    "Runtime Error: None of the arms matched the input '{}'.",
                    input
                )?;
                writeln!(f, "Scope snippet: {}", scope_content)?;
            }
            ErrorKind::RegisterOutOfBounds {
                requested,
                available,
                suggestion,
            } => {
                if *available == 0 {
                    writeln!(
                        f,
                        "Runtime Error: Tried to access register #{}, but there are no registers available here.",
                        requested
                    )?;
                } else {
                    writeln!(
                        f,
                        "Runtime Error: Tried to access register #{}, but only {} registers are available (valid range: #1..#{}).",
                        requested, available, available
                    )?;
                }
                if let Some(hint) = suggestion {
                    writeln!(
                        f,
                        "Help: A parent scope contains this register. Did you mean to use '{}' ?",
                        hint
                    )?;
                }
            }
            ErrorKind::MissingParentScope {
                requested_level,
                actual_depth,
            } => {
                writeln!(
                    f,
                    "Runtime Error: Tried to access a parent scope {} levels deep, but the current depth is only {}.",
                    requested_level, actual_depth
                )?;
                writeln!(
                    f,
                    "Help: Reduce the number of '^' prefixes on the register call."
                )?;
            }
            ErrorKind::FileReadError { path, reason } => {
                writeln!(
                    f,
                    "I/O Error: Failed to read file '{}'.\nReason: {}",
                    path, reason
                )?;
            }
            ErrorKind::InputReadError { reason } => {
                writeln!(f, "I/O Error: Failed to read input.\nReason: {}", reason)?;
            }
            ErrorKind::OutputWriteError { reason } => {
                writeln!(f, "I/O Error: Failed to write output.\nReason: {}", reason)?;
            }
            ErrorKind::InternalInvariant { message } => {
                writeln!(
                    f,
                    "Internal Error: An interpreter invariant was violated.\nDetails: {}",
                    message
                )?;
            }
        }

        // 2. Print the rich Backtrace
        if !self.backtrace.is_empty() {
            writeln!(f, "\n--- Backtrace ---")?;
            for (i, frame) in self.backtrace.iter().enumerate() {
                writeln!(f, "{}: Depth {}", i, frame.depth)?;
                if !frame.state_snippet.is_empty() {
                    writeln!(f, "Location:\n{}", frame.state_snippet)?;
                }
                if !frame.registers.is_empty() {
                    writeln!(f, "   Registers: {:?}", frame.registers)?;
                }
                if !frame.defined_functions.is_empty() {
                    writeln!(f, "   Functions: {:?}", frame.defined_functions)?;
                }
            }
        }

        Ok(())
    }
}

impl std::error::Error for SubtextError {}
