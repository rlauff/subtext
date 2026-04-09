// src/scanner.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenType {
    Def,
    NameAfterDef,
    Brace(BraceType),
    InputPatternSeparator,
    PatternOutputSeparator,
    NewArmSeparator,
    FunctionCall,
    RegisterCall,
    Comment,
    GhostChar,
    //   Input,
    Pattern,
    //   Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraceType {
    RoundOpening,
    RoundClosing,
    CurlyOpening,
    CurlyClosing,
}

impl TokenType {
    fn as_lsp_index(&self) -> usize {
        match self {
            TokenType::Def => 0,
            TokenType::NameAfterDef => 1,
            TokenType::Brace(..) => 2,
            TokenType::InputPatternSeparator => 3,
            TokenType::PatternOutputSeparator => 4,
            TokenType::NewArmSeparator => 5,
            TokenType::FunctionCall => 6,
            TokenType::RegisterCall => 7,
            TokenType::Comment => 8,
            TokenType::GhostChar => 9,
            //            TokenType::Input => 10,
            TokenType::Pattern => 11,
            //           TokenType::Output => 12,
        }
    }
}

fn brace_to_brace_type(c: char) -> BraceType {
    match c {
        '(' => BraceType::RoundOpening,
        ')' => BraceType::RoundClosing,
        '{' => BraceType::CurlyOpening,
        '}' => BraceType::CurlyClosing,
        _ => unreachable!(),
    }
}

struct Token {
    pub token_type: TokenType,
    pub length: usize,            // How many characters the editor should colorize
    pub position: (usize, usize), // x and y position of the token in the text
}

struct Scanner {
    last_char: char,
    in_pattern: bool,
    pattern_started: bool,
    in_comment: bool,
    in_function_name_definition: bool,
    in_register_call: bool,
    next_curly_starts_function_body: bool,
    word_buffer: String,
    word_start_position: (usize, usize),
    potential_token_position: (usize, usize),
    reading_head: (usize, usize),
    char_buffer: Vec<char>,
    tokens: Vec<Token>,
}

impl Scanner {
    fn new() -> Self {
        Self {
            last_char: ' ',
            in_pattern: false,
            pattern_started: false,
            in_comment: false,
            in_function_name_definition: false,
            in_register_call: false,
            next_curly_starts_function_body: false,
            word_buffer: String::new(),
            word_start_position: (0, 0),
            potential_token_position: (0, 0),
            reading_head: (0, 0),
            char_buffer: Vec::new(),
            tokens: Vec::new(),
        }
    }

    fn update_with_new_char(&mut self, c: char) {
        let current_position = self.reading_head;
        if self.in_pattern {
            if !self.pattern_started && !c.is_whitespace() {
                self.potential_token_position = current_position;
                self.pattern_started = true;
            }
        } else if !self.in_comment
            && !self.in_register_call
            && (self.last_char.is_whitespace()
                || matches!(self.last_char, '{' | '}' | '(' | ')' | ':' | '|' | '>'))
            && !c.is_whitespace()
            && !((self.last_char == ':' && c == ':')
                || (self.last_char == '|' && c == '|')
                || (self.last_char == '=' && c == '>')
                || (self.last_char == '/' && c == '/'))
        {
            self.potential_token_position = current_position;
        }

        self.char_buffer.push(c);
        if c == '\n' {
            self.reading_head.0 = 0;
            self.reading_head.1 += 1;
        } else {
            self.reading_head.0 += 1;
        };
    }

    fn add_token(&mut self, token_type: TokenType) {
        let was_in_pattern = self.in_pattern;
        self.char_buffer.clear();
        self.in_pattern = false;
        self.pattern_started = false;
        self.in_comment = false;
        self.in_register_call = false;

        match token_type {
            TokenType::PatternOutputSeparator => {
                let len_terminator = match token_type {
                    TokenType::Brace(BraceType::CurlyClosing) => 1,
                    _ => 2,
                };
                let consumed = self
                    .reading_head
                    .0
                    .saturating_sub(self.potential_token_position.0);
                let predecessor_length = consumed.saturating_sub(len_terminator);
                if was_in_pattern && predecessor_length > 0 {
                    self.tokens.push(Token {
                        token_type: TokenType::Pattern,
                        length: predecessor_length,
                        position: self.potential_token_position,
                    });
                }
                self.tokens.push(Token {
                    token_type,
                    length: len_terminator,
                    position: (self.reading_head.0 - len_terminator, self.reading_head.1),
                });
            }

            TokenType::InputPatternSeparator
            | TokenType::NewArmSeparator
            | TokenType::Brace(BraceType::CurlyClosing) => {
                let len_terminator = match token_type {
                    TokenType::Brace(BraceType::CurlyClosing) => 1,
                    _ => 2,
                };
                self.tokens.push(Token {
                    token_type,
                    length: len_terminator,
                    position: (self.reading_head.0 - len_terminator, self.reading_head.1),
                });

                if token_type == TokenType::InputPatternSeparator
                    || token_type == TokenType::NewArmSeparator
                {
                    self.in_pattern = true;
                    self.pattern_started = false;
                }
            }

            TokenType::Brace(BraceType::RoundOpening) => {
                // a round opening brace indicates a function call, we need to add the two
                // correspoinding tokens
                let function_name_length = self
                    .reading_head
                    .0
                    .saturating_sub(self.potential_token_position.0)
                    - 1;
                if function_name_length > 0 {
                    self.tokens.push(Token {
                        token_type: TokenType::FunctionCall,
                        length: function_name_length,
                        position: self.potential_token_position,
                    });
                }
                self.tokens.push(Token {
                    token_type: TokenType::Brace(BraceType::RoundOpening),
                    length: 1,
                    position: (self.reading_head.0 - 1, self.reading_head.1),
                });
            }

            TokenType::Brace(BraceType::RoundClosing)
            | TokenType::Brace(BraceType::CurlyOpening) => {
                self.tokens.push(Token {
                    token_type,
                    length: 1,
                    position: (self.reading_head.0 - 1, self.reading_head.1),
                });
            }

            _ => {
                let length = self
                    .reading_head
                    .0
                    .saturating_sub(self.potential_token_position.0);
                if length > 0 {
                    self.tokens.push(Token {
                        token_type,
                        length,
                        position: self.potential_token_position,
                    });
                }
            }
        }
        self.potential_token_position = self.reading_head;

        if token_type == TokenType::Brace(BraceType::CurlyOpening)
            && self.next_curly_starts_function_body
        {
            self.in_pattern = true;
            self.pattern_started = false;
            self.next_curly_starts_function_body = false;
        }
    }
}

pub fn scan(text: &str) -> Vec<u32> {
    let mut scanner = Scanner::new();
    for c in text.chars() {
        let is_identifier_char = c.is_alphanumeric() || c == '_';
        if !scanner.in_comment && !scanner.in_pattern && !scanner.in_register_call {
            if is_identifier_char {
                if scanner.word_buffer.is_empty() {
                    scanner.word_start_position = scanner.reading_head;
                }
                scanner.word_buffer.push(c);
            } else if !scanner.word_buffer.is_empty() {
                if scanner.word_buffer == "def" {
                    scanner.tokens.push(Token {
                        token_type: TokenType::Def,
                        length: scanner.word_buffer.len(),
                        position: scanner.word_start_position,
                    });
                    scanner.in_function_name_definition = true;
                } else if scanner.in_function_name_definition {
                    scanner.tokens.push(Token {
                        token_type: TokenType::NameAfterDef,
                        length: scanner.word_buffer.len(),
                        position: scanner.word_start_position,
                    });
                    scanner.in_function_name_definition = false;
                    scanner.next_curly_starts_function_body = true;
                }
                scanner.word_buffer.clear();
            }
        }

        // check if the current state should be ended
        if scanner.in_register_call && !(c.is_digit(10) || c == '^' || c == '#') {
            scanner.add_token(TokenType::RegisterCall);
        }

        if scanner.in_comment && c == '\n' {
            scanner.add_token(TokenType::Comment);
        }

        // update the scanner state with the new character
        scanner.update_with_new_char(c);

        // running states coninued
        if scanner.in_comment || scanner.in_register_call {
            scanner.last_char = c;
            continue;
        }

        if scanner.in_pattern {
            if c == '>' && scanner.last_char == '=' {
                scanner.add_token(TokenType::PatternOutputSeparator);
            }
            scanner.last_char = c;
            continue;
        }

        // potentially get into new states or add tokens based on the new character
        match c {
            '~' => scanner.add_token(TokenType::GhostChar),

            ':' if scanner.last_char == ':' => {
                scanner.add_token(TokenType::InputPatternSeparator);
            }

            '|' if scanner.last_char == '|' => {
                scanner.add_token(TokenType::NewArmSeparator);
            }

            '>' if scanner.last_char == '=' => {
                scanner.add_token(TokenType::PatternOutputSeparator);
            }

            '(' | ')' | '{' | '}' => {
                let brace_type = brace_to_brace_type(c);
                scanner.add_token(TokenType::Brace(brace_type));
            }

            '/' if scanner.last_char == '/' => {
                scanner.in_comment = true;
                if scanner.reading_head.0 >= 2 {
                    scanner.potential_token_position =
                        (scanner.reading_head.0 - 2, scanner.reading_head.1);
                } else {
                    scanner.potential_token_position = (0, scanner.reading_head.1);
                }
            }

            '^' | '#' => {
                if !scanner.in_register_call {
                    scanner.in_register_call = true;
                    scanner.potential_token_position = (
                        scanner.reading_head.0.saturating_sub(1),
                        scanner.reading_head.1,
                    );
                }
            }

            _ => {}
        }

        scanner.last_char = c;
    }

    if scanner.in_register_call {
        scanner.add_token(TokenType::RegisterCall);
    }

    if !scanner.word_buffer.is_empty() {
        if scanner.word_buffer == "def" {
            scanner.tokens.push(Token {
                token_type: TokenType::Def,
                length: scanner.word_buffer.len(),
                position: scanner.word_start_position,
            });
            scanner.in_function_name_definition = true;
        } else if scanner.in_function_name_definition {
            scanner.tokens.push(Token {
                token_type: TokenType::NameAfterDef,
                length: scanner.word_buffer.len(),
                position: scanner.word_start_position,
            });
            scanner.in_function_name_definition = false;
            scanner.next_curly_starts_function_body = true;
        }
        scanner.word_buffer.clear();
    }

    if scanner.in_comment {
        scanner.add_token(TokenType::Comment);
    }

    // the scanner is done, now we calculate the deltas and return the results in the lsp format
    // which is a flat array of integers where every 5 integers represent a token with the following format:
    // [deltaLine, deltaStart, length, tokenTypeIndex, tokenModifierBitset, ..next token..]
    let mut lsp_tokens = Vec::new();
    let mut last_position = (0, 0);
    for token in scanner.tokens {
        let delta_start = if last_position.1 < token.position.1 {
            // if the new token is on a new line, its delta start is counted from the beginning of
            // the line:
            token.position.0
        } else {
            token.position.0 - last_position.0
        };
        let delta_line = token.position.1 - last_position.1;
        last_position = token.position;
        lsp_tokens.push(delta_line as u32);
        lsp_tokens.push(delta_start as u32);
        lsp_tokens.push(token.length as u32);
        lsp_tokens.push(token.token_type.as_lsp_index() as u32);
        lsp_tokens.push(0); // token modifiers bitset, we don't use any modifiers
    }
    lsp_tokens
}
