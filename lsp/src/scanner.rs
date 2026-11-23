// src/scanner.rs
//
// Classifies every character of a Subtext program and turns the result into
// LSP semantic tokens.
//
// The scanner follows the rules of the interpreter:
//
// - `//` starts a comment everywhere, also inside patterns (comments are
//   stripped when the program is loaded, before anything else happens).
// - `::`, `||` and `=>` only separate the parts of a scope when they are not
//   nested in `( )` or `{ }` inside that scope. The first `::` ends the input,
//   `||` starts a new arm, the first `=>` of an arm ends its pattern.
// - A function body is a scope that starts with a pattern.
// - Patterns are never evaluated: calls, registers and `[ ]` inside them are
//   just regex. Their `( )` and `{ }` still count for the nesting.
// - A call name is everything in front of the `(` up to the last whitespace or
//   `~`, exactly the text the interpreter will look up.
//
// Protection `[ ]` needs some care. Protected text is evaluated later, after
// enclosing functions have returned and stripped the braces. Its structure is
// highlighted like normal code, with the `protected` modifier. Protection
// often builds a scope whose pattern is filled in by unprotected code:
//
//     [{ ]#1[ :: (?s)^]~_escape(>^#2)[$ => y || => n }]
//
// Here `(?s)^` and `$` are parts of a pattern, but `~_escape(>^#2)` sits
// outside the protection, so it runs now and is highlighted as code. In
// general: a pattern that starts at protection depth d contains code wherever
// the depth drops below d.

/// Token types, in the order of the legend sent to the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Def,
    FunctionName,
    Brace,
    InputSeparator,
    Arrow,
    ArmSeparator,
    Call,
    Register,
    Comment,
    Ghost,
    Pattern,
    Protection,
}

/// The legend: entry i is the semantic token type of `Kind` number i.
pub const TOKEN_TYPES: [&str; 12] = [
    "keyword",  // Def
    "function", // FunctionName
    "operator", // Brace
    "operator", // InputSeparator
    "operator", // Arrow
    "operator", // ArmSeparator
    "function", // Call
    "string",   // Register
    "comment",  // Comment
    "keyword",  // Ghost
    "regexp",   // Pattern
    "macro",    // Protection
];

/// Token modifiers; bit i of the modifier bitset stands for entry i.
pub const TOKEN_MODIFIERS: [&str; 1] = ["protected"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Class {
    kind: Kind,
    protected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Input,
    Pattern,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// A `{ }` scope. `depth` is the protection depth its current pattern
    /// started at (only used while `section` is `Pattern`).
    Scope { section: Section, depth: usize },
    /// A `( )` in code, usually the argument list of a call.
    Paren,
    /// A `( )` or `{ }` inside a pattern, started at protection depth `depth`.
    Group { depth: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefState {
    None,
    /// After `def`, collecting the name until the `{` of the body.
    Name,
}

struct Scanner {
    chars: Vec<char>,
    classes: Vec<Option<Class>>,
    stack: Vec<Frame>,
    /// Current protection depth (number of open `[`).
    depth: usize,
    /// Indices of the characters of the word in front of the reading head;
    /// becomes the call name when a `(` follows.
    word: Vec<usize>,
    def_state: DefState,
}

impl Scanner {
    fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let classes = vec![None; chars.len()];
        Self {
            chars,
            classes,
            stack: Vec::new(),
            depth: 0,
            word: Vec::new(),
            def_state: DefState::None,
        }
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn mark(&mut self, from: usize, to: usize, kind: Kind) {
        let protected = self.depth > 0 && kind != Kind::Protection;
        for class in &mut self.classes[from..to] {
            *class = Some(Class { kind, protected });
        }
    }

    /// Some(d) if the innermost frame is pattern text that started at protection depth d.
    fn pattern_depth(&self) -> Option<usize> {
        match self.stack.last() {
            Some(Frame::Scope {
                section: Section::Pattern,
                depth,
            })
            | Some(Frame::Group { depth }) => Some(*depth),
            _ => None,
        }
    }

    fn set_section(&mut self, section: Section) {
        let depth = self.depth;
        if let Some(Frame::Scope {
            section: s,
            depth: d,
        }) = self.stack.last_mut()
        {
            *s = section;
            *d = depth;
        }
    }

    fn top_section(&self) -> Option<Section> {
        match self.stack.last() {
            Some(Frame::Scope { section, .. }) => Some(*section),
            _ => None,
        }
    }

    fn run(&mut self) {
        let mut i = 0;
        while i < self.chars.len() {
            if self.chars[i] == '/' && self.at(i + 1) == Some('/') {
                let end = (i..self.chars.len())
                    .find(|&j| self.chars[j] == '\n')
                    .unwrap_or(self.chars.len());
                self.mark(i, end, Kind::Comment);
                i = end;
                continue;
            }
            i = match self.pattern_depth() {
                Some(d) if self.depth >= d => self.pattern_char(i, d),
                _ => self.code_char(i),
            };
        }
    }

    /// Handles the character at i inside a pattern; returns the next index.
    fn pattern_char(&mut self, i: usize, pattern_depth: usize) -> usize {
        self.word.clear();
        let c = self.chars[i];
        let next = self.at(i + 1);
        let in_scope = matches!(self.stack.last(), Some(Frame::Scope { .. }));

        match c {
            '=' if next == Some('>') && in_scope => {
                self.mark(i, i + 2, Kind::Arrow);
                self.set_section(Section::Output);
                // regex classes like [^a] at depth 0 never count as protection,
                // so a stray [ in a pattern cannot leak out of it
                self.depth = self.depth.min(pattern_depth);
                return i + 2;
            }
            '|' if next == Some('|') && in_scope => {
                self.mark(i, i + 2, Kind::ArmSeparator);
                self.set_section(Section::Pattern);
                return i + 2;
            }
            '(' | '{' => {
                self.stack.push(Frame::Group {
                    depth: pattern_depth,
                });
                self.mark(i, i + 1, Kind::Pattern);
            }
            ')' | '}' => {
                if matches!(self.stack.last(), Some(Frame::Group { .. })) {
                    self.stack.pop();
                    self.mark(i, i + 1, Kind::Pattern);
                } else if c == '}' {
                    // a scope whose last arm has no =>
                    self.stack.pop();
                    self.mark(i, i + 1, Kind::Brace);
                } else {
                    self.mark(i, i + 1, Kind::Pattern);
                }
            }
            // A [ ] in a pattern is a regex class. It only counts as protection
            // if the pattern itself is protected: then the interpreter counts
            // every bracket while skipping over the protected text.
            '[' => {
                if self.depth > 0 {
                    self.depth += 1;
                }
                self.mark(i, i + 1, Kind::Pattern);
            }
            ']' => {
                if self.depth > 0 {
                    self.depth -= 1;
                    if self.depth < pattern_depth {
                        // leaves the protection: unprotected code inside the pattern follows
                        self.mark(i, i + 1, Kind::Protection);
                        return i + 1;
                    }
                }
                self.mark(i, i + 1, Kind::Pattern);
            }
            c if c.is_whitespace() => {}
            _ => self.mark(i, i + 1, Kind::Pattern),
        }
        i + 1
    }

    /// Handles the character at i outside of patterns; returns the next index.
    fn code_char(&mut self, i: usize) -> usize {
        let c = self.chars[i];
        let next = self.at(i + 1);

        let is_bracket = c == '[' || (c == ']' && self.depth > 0);
        if self.def_state == DefState::Name && c != '{' && !is_bracket {
            // the interpreter takes every non-whitespace character up to the { as the name
            if !c.is_whitespace() {
                self.mark(i, i + 1, Kind::FunctionName);
            }
            return i + 1;
        }

        match c {
            c if c.is_whitespace() => {
                let is_def = self.word.len() == 3
                    && self.word.iter().map(|&j| self.chars[j]).eq("def".chars());
                if is_def {
                    let (from, to) = (self.word[0], self.word[2] + 1);
                    self.mark(from, to, Kind::Def);
                    self.def_state = DefState::Name;
                }
                self.word.clear();
            }
            '~' => {
                self.mark(i, i + 1, Kind::Ghost);
                self.word.clear();
            }
            '^' | '#' => {
                let mut j = i;
                while self.at(j) == Some('^') {
                    j += 1;
                }
                if self.at(j) != Some('#') {
                    // a ^ without # is just text
                    self.word.push(i);
                    return i + 1;
                }
                j += 1;
                while self.at(j).is_some_and(|c| c.is_ascii_digit()) {
                    j += 1;
                }
                self.mark(i, j, Kind::Register);
                // a register call swallows one following ~
                if self.at(j) == Some('~') {
                    self.mark(j, j + 1, Kind::Ghost);
                    j += 1;
                }
                self.word.clear();
                return j;
            }
            // protection braces do not end a word: the interpreter keeps
            // reading the name behind them
            '[' => {
                self.depth += 1;
                self.mark(i, i + 1, Kind::Protection);
            }
            ']' if self.depth > 0 => {
                self.depth -= 1;
                self.mark(i, i + 1, Kind::Protection);
            }
            '{' => {
                let section = if self.def_state == DefState::Name {
                    Section::Pattern
                } else {
                    Section::Input
                };
                self.def_state = DefState::None;
                self.stack.push(Frame::Scope {
                    section,
                    depth: self.depth,
                });
                self.mark(i, i + 1, Kind::Brace);
                self.word.clear();
            }
            '}' => {
                // close the innermost scope, together with anything left open in it
                if let Some(pos) = self
                    .stack
                    .iter()
                    .rposition(|f| matches!(f, Frame::Scope { .. }))
                {
                    self.stack.truncate(pos);
                }
                self.mark(i, i + 1, Kind::Brace);
                self.word.clear();
            }
            '(' => {
                let word = std::mem::take(&mut self.word);
                for j in word {
                    self.mark(j, j + 1, Kind::Call);
                }
                self.stack.push(Frame::Paren);
                self.mark(i, i + 1, Kind::Brace);
            }
            ')' => {
                if self.stack.last() == Some(&Frame::Paren) {
                    self.stack.pop();
                }
                self.mark(i, i + 1, Kind::Brace);
                self.word.clear();
            }
            ':' if next == Some(':') && self.top_section() == Some(Section::Input) => {
                self.mark(i, i + 2, Kind::InputSeparator);
                self.set_section(Section::Pattern);
                self.word.clear();
                return i + 2;
            }
            '|' if next == Some('|')
                && matches!(
                    self.top_section(),
                    Some(Section::Output) | Some(Section::Pattern)
                ) =>
            {
                self.mark(i, i + 2, Kind::ArmSeparator);
                self.set_section(Section::Pattern);
                self.word.clear();
                return i + 2;
            }
            // only reachable for unprotected code inside a protected pattern;
            // its text ends up in the built scope and ends the pattern there
            '=' if next == Some('>') && self.top_section() == Some(Section::Pattern) => {
                self.mark(i, i + 2, Kind::Arrow);
                self.set_section(Section::Output);
                self.word.clear();
                return i + 2;
            }
            _ => self.word.push(i),
        }
        i + 1
    }
}

fn classify(text: &str) -> Vec<Option<Class>> {
    let mut scanner = Scanner::new(text);
    scanner.run();
    scanner.classes
}

/// Returns the semantic tokens of `text` in the LSP format: a flat array in
/// which every 5 integers describe one token:
/// [deltaLine, deltaStart, length, tokenType, tokenModifiers, ..next token..]
/// Positions and lengths are counted in UTF-16 code units, and no token spans
/// more than one line.
pub fn scan(text: &str) -> Vec<u32> {
    let classes = classify(text);

    // (line, column, length, class) of every token, built from runs of equally classified characters
    let mut tokens: Vec<(u32, u32, u32, Class)> = Vec::new();
    let (mut line, mut column) = (0u32, 0u32);
    let mut previous: Option<Class> = None;
    for (c, class) in text.chars().zip(classes) {
        let width = c.len_utf16() as u32;
        match (class, tokens.last_mut()) {
            (Some(class), Some(last)) if previous == Some(class) => last.2 += width,
            (Some(class), _) => tokens.push((line, column, width, class)),
            (None, _) => {}
        }
        previous = class;
        if c == '\n' {
            line += 1;
            column = 0;
            previous = None;
        } else {
            column += width;
        }
    }

    let mut data = Vec::with_capacity(tokens.len() * 5);
    let (mut last_line, mut last_column) = (0, 0);
    for (line, column, length, class) in tokens {
        let delta_start = if line == last_line {
            column - last_column
        } else {
            column
        };
        data.extend([
            line - last_line,
            delta_start,
            length,
            class.kind as u32,
            class.protected as u32,
        ]);
        (last_line, last_column) = (line, column);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One character per input character: the kind, `.` for none.
    fn kinds(text: &str) -> String {
        classify(text)
            .iter()
            .map(|class| match class.map(|c| c.kind) {
                None => '.',
                Some(Kind::Def) => 'k',
                Some(Kind::FunctionName) => 'F',
                Some(Kind::Brace) => 'B',
                Some(Kind::InputSeparator) => ':',
                Some(Kind::Arrow) => '>',
                Some(Kind::ArmSeparator) => '|',
                Some(Kind::Call) => 'c',
                Some(Kind::Register) => 'r',
                Some(Kind::Comment) => '/',
                Some(Kind::Ghost) => '~',
                Some(Kind::Pattern) => 'p',
                Some(Kind::Protection) => '[',
            })
            .collect()
    }

    /// One character per input character: `*` if protected.
    fn protection(text: &str) -> String {
        classify(text)
            .iter()
            .map(|class| match class {
                Some(c) if c.protected => '*',
                _ => '.',
            })
            .collect()
    }

    #[test]
    fn function_definition() {
        let text = "def f { a(.) => g(#1) || => x }";
        assert_eq!(kinds(text), "kkk.F.B.pppp.>>.cBrrB.||.>>...B");
    }

    #[test]
    fn scope_and_registers() {
        let text = "{ x :: (.)(.) => ^#2~~f(#1) }";
        assert_eq!(kinds(text), "B...::.pppppp.>>.rrr~~cBrrB.B");
    }

    #[test]
    fn regex_classes_and_braces_stay_in_the_pattern() {
        let text = "def f { [^#]{2}(x)=>y || [a] => z }";
        assert_eq!(kinds(text), "kkk.F.B.pppppppppp>>..||.ppp.>>...B");
    }

    #[test]
    fn comments_inside_patterns_and_bodies() {
        let text = "def f {\n  // c\n  a => b // d\n}";
        assert_eq!(kinds(text), "kkk.F.B...////...p.>>...////.B");
        assert_eq!(kinds("def f { a// x\n => b }"), "kkk.F.B.p////..>>...B");
    }

    #[test]
    fn separators_outside_of_scopes_are_text() {
        assert_eq!(kinds("print(a || b :: c => d)"), "cccccB................B");
    }

    #[test]
    fn call_names_reach_back_to_whitespace_or_ghost_char() {
        assert_eq!(kinds("a,b(x) y~z(x)"), "cccB.B..~cB.B");
        // the interpreter would call a function named "x>f"
        assert_eq!(kinds("x>f()"), "cccBB");
        // brackets do not end the name either: once they are stripped, this calls abc
        assert_eq!(kinds("a[b]c(x)"), "c[c[cB.B");
    }

    #[test]
    fn caret_without_register_is_text() {
        assert_eq!(kinds("a^b ^^#12~c"), "....rrrrr~.");
    }

    #[test]
    fn trampoline() {
        let text = "#1[~_len(]~inc(^#1), #2[)]";
        assert_eq!(kinds(text), "rr[~ccccB[~cccBrrrB..rr[B[");
        assert_eq!(protection(text), "...******...............*.");
    }

    #[test]
    fn protected_scope_with_live_code_in_its_pattern() {
        let text = "[{ ]#1[ :: (?s)^]~_escape(>^#2)[$ => y || => n }]";
        assert_eq!(
            kinds(text),
            "[B.[rr[.::.ppppp[~cccccccB.rrrB[p.>>...||.>>...B["
        );
        assert_eq!(
            protection(text),
            ".*......**.*****................*.**...**.**...*."
        );
    }

    #[test]
    fn protected_pattern_with_regex_classes() {
        // from nth in std/list.stx
        let text = "[{ ]#1[ :: (?:\\|[^|]*){]#2[}\\|([^|]*) => #1 || => ~error(i ]#2[) }]";
        assert_eq!(
            kinds(text),
            "[B.[rr[.::.pppppppppppp[rr[pppppppppp.>>.rr.||.>>.~cccccB..[rr[B.B["
        );
    }

    #[test]
    fn live_calls_inside_a_protected_quantifier() {
        // from slice in std/string.stx
        let text = "[{ ]#1[ :: ^.{]#2[}(.{0,]~max(0, ^#3)[}) => #1 || => }]";
        assert_eq!(
            kinds(text),
            "[B.[rr[.::.ppp[rr[pppppp[~cccB...rrrB[pp.>>.rr.||.>>.B["
        );
    }

    #[test]
    fn delayed_definition() {
        // from examples/memory_example.stx: the def only happens once the braces are stripped
        assert_eq!(kinds("[d]ef x [{a=>b}]"), "[kkkk.F.[Bp>>.B[");
    }

    #[test]
    fn def_name_may_follow_on_the_next_line() {
        assert_eq!(kinds("def\nadd {a=>b}"), "kkk.FFF.Bp>>.B");
    }

    #[test]
    fn tokens_are_split_at_line_ends_and_counted_in_utf16() {
        // pattern "a\nb" is two tokens; "é" and "𝄞" take 1 and 2 UTF-16 units
        let data = scan("{x::a\nb=>é𝄞f()}");
        let tokens: Vec<&[u32]> = data.chunks(5).collect();
        let pattern = Kind::Pattern as u32;
        assert_eq!(tokens[2], &[0, 2, 1, pattern, 0]);
        assert_eq!(tokens[3], &[1, 0, 1, pattern, 0]);
        // the call "é𝄞f" starts at column 3 and is 4 units long
        assert_eq!(tokens[5], &[0, 2, 4, Kind::Call as u32, 0]);
    }
}
