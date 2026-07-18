use std::env;
use std::fs;
use std::io::IsTerminal;

use subtext::{
    error::{self, ErrorKind, SubtextError},
    run_code_logic,
};

fn use_color() -> bool {
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0") {
        return true;
    }
    std::io::stderr().is_terminal()
}

const RED: &str = "\x1b[31;1m";
const YELLOW: &str = "\x1b[33;1m";
const CYAN: &str = "\x1b[36;1m";
const BLUE: &str = "\x1b[34;1m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Colorizes one rendered error line based on its stable prefix — the exact same
/// classification the web terminal uses.
fn colorize_line(line: &str) -> String {
    // error[code]: headline  ->  red label, bold message
    if let Some(rest) = line.strip_prefix("error[")
        && let Some((code, msg)) = rest.split_once("]:")
    {
        return format!("{}error[{}]:{}{}{}{}", RED, code, RESET, BOLD, msg, RESET);
    }
    if let Some(rest) = line.strip_prefix("note:") {
        return format!("{}note:{}{}", CYAN, RESET, rest);
    }
    if let Some(rest) = line.strip_prefix("help:") {
        return format!("{}help:{}{}", YELLOW, RESET, rest);
    }
    if line.trim_start().starts_with("-->") {
        return format!("{}{}{}", BLUE, line, RESET);
    }
    if line.starts_with("backtrace") || line.starts_with("for a detailed explanation") {
        return format!("{}{}{}", DIM, line, RESET);
    }
    // backtrace gutter lines; caret lines get their span painted red
    if let Some(bar) = line.find('\u{2502}') {
        let (gutter, content) = line.split_at(bar + '\u{2502}'.len_utf8());
        if content.trim_start().starts_with('^') {
            return format!("{}{}{}{}{}{}", DIM, gutter, RESET, RED, content, RESET);
        }
        return format!("{}{}{}{}", DIM, gutter, RESET, content);
    }
    line.to_string()
}

fn print_error(rendered: &str) {
    if use_color() {
        for line in rendered.lines() {
            eprintln!("{}", colorize_line(line));
        }
    } else {
        eprintln!("{}", rendered.trim_end_matches('\n'));
    }
}

fn main() {
    let arg = match env::args().nth(1) {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: subtext <file>");
            eprintln!("       subtext --explain <error-code>");
            return;
        }
    };

    // `--explain`: prints the long-form explanation of an
    // error code (the codes appear as `error[<code>]` in error messages).
    if arg == "--explain" {
        let code = match env::args().nth(2) {
            Some(code) => code,
            None => {
                eprintln!("Usage: subtext --explain <error-code>");
                return;
            }
        };
        match error::explain(&code) {
            Some(text) => println!("error[{}]\n\n{}", code, text),
            None => {
                eprintln!("no explanation for '{}'. Known codes:", code);
                for known in error::ALL_CODES {
                    eprintln!("  {}", known);
                }
            }
        }
        return;
    }

    let input_string = match fs::read_to_string(&arg) {
        Ok(content) => content,
        Err(err) => {
            let io_error = SubtextError::new(ErrorKind::FileReadError {
                path: arg,
                reason: err.to_string(),
            });
            print_error(&io_error.to_string());
            return;
        }
    };

    if let Err(err) = run_code_logic(input_string) {
        let trailer = format!(
            "for a detailed explanation of this error, run `subtext --explain {}`",
            err.kind.code()
        );
        print_error(&format!("{}{}", err, trailer));
    }
}
