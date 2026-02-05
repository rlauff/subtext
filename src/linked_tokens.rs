

use crate::errors::LinkedTokensError;
use crate::errors::ParseError;
use crate::parser::*;

#[derive(Clone, Debug)]
enum Token {
    Char(char),
    ScopeStart,
    ScopeEnd,
    Colon,                      // separates input, pattern and output
    NewArm,                     // separates arms in a function definition
    FunctionCall(String),       // function name, ended by a ScopeEnd
    DefStart(String),           // name of the defined function
    DefGlobalStart(String),     // name of the defined function, but marked as global
    RegisterCall(usize, usize), // depth, index
    RootNodeToken,              // a placeholder token for the root node
    GetInput(String),           // get input from the user, with a prompt
    Error(String),              // error token with message
    GenericWhitespace           // A generic Whitespace token to terminate register calls etc
}

#[derive(Clone, Debug)]
struct TokenNode {
    token: Token,
    next: Option<usize>,    // index of the next token in the arena
}
// the linked tokes struct
// the first token node is always the one at index 0 in the arena
// it does not correspond to any actual token, but is meant as a root for the linked list
// hence, if we want to insert at the very start, we insert after index 0
#[derive(Clone)]
pub struct LinkedTokens {
    arena: Vec<TokenNode>,
}

impl LinkedTokens {
    // parses a string into linked tokens
    pub fn from_string(s: &str) -> Result<Self, ParseError> {
        // initialize the arena with the root node
        let mut arena = vec![
            TokenNode {
                token: Token::RootNodeToken,
                next: None,
            }
        ];
        let mut parser = Parser::new();
        let mut chars = s.chars();
        let mut c = ' ';                // the current charcter being parsed
        let mut get_new_char = true;    // whether we need to get a new char from the iterator

        // iter over the chars in the string and create the token nodes
        loop {
            if get_new_char {
                if let Some(next_char) = chars.next() { // find the next char to parse and update c variable
                    c = next_char;
                } else {
                    break // no more chars to parse
                }
            }
            get_new_char = true; // by default we get a new char in the next iteration
            match parser.state {
                ParseState::Normal => {
                    match c {
                        '{' => {
                            if parser.buffer.len() > 0 { // this scope is a function call
                                // we have a function name before the scope start
                                let func_name: String = parser.buffer.iter().collect();
                                push_token_at_end(&mut arena, Token::FunctionCall(func_name));
                            } else {    // this scope is stand alone
                                push_token_at_end(&mut arena, Token::ScopeStart);
                            }
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                            
                        },
                        '}' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_token_at_end(&mut arena, Token::ScopeEnd);
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        ':' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_token_at_end(&mut arena, Token::Colon);
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        ';' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_token_at_end(&mut arena, Token::NewArm);
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        '^' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            // found a potential register call
                            parser.state = ParseState::PotRegisterCall;
                            parser.depth = 1;   // we found one '^' so init depth as 1
                            parser.buffer.push('^'); // push into the buffer, in case its not a Register call, then we need to add the Char Tokens
                        },
                        '$' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            // found a register call start
                            parser.state = ParseState::InRegisterCallParseIndex;
                            parser.buffer.push('$');
                        },
                        ' ' | '\n' | '\t' => {
                            // if the buffer is exactly def, we found a function definition start
                            if parser.buffer == vec!['d', 'e', 'f'] {
                                parser.state = ParseState::ParsingDefFunctionName;
                            }  else if parser.buffer == vec!['*', 'd', 'e', 'f'] {
                                parser.state = ParseState::ParsingDefFunctionName;
                                parser.global = true;
                            } else {
                                empty_buffer_at_end(&mut arena, &parser.buffer);
                                push_token_at_end(&mut arena, Token::GenericWhitespace);
                            }
                            parser.reset_buffers();
                        },
                        '/' => {
                            // found a potential comment start
                            parser.state = ParseState::PotComment;
                        },
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
                            // valid function name char, add to buffer
                            parser.buffer.push(c);
                        },
                        '\\' => {
                            // escape character found
                            parser.state = ParseState::Escape;
                        },
                        '(' => {
                            if parser.buffer == vec!['g', 'e', 't', '_', 'i', 'n', 'p', 'u', 't'] {
                                // we found a get_input function call start
                                parser.reset_buffers();
                                parser.state = ParseState::Normal;
                                // now parse the prompt string until the closing ')'
                                let mut prompt_buffer: Vec<char> = Vec::new();
                                loop {
                                    if let Some(next_char) = chars.next() {
                                        if next_char == ')' {
                                            break; // end of prompt string
                                        } else {
                                            prompt_buffer.push(next_char);
                                        }
                                    } else {
                                        return Err(ParseError::InputEndedUnexpectedly);
                                    }
                                }
                                let prompt: String = prompt_buffer.iter().collect();
                                push_token_at_end(&mut arena, Token::GetInput(prompt));
                            } else if parser.buffer == vec!['e', 'r', 'r', 'o', 'r'] {
                                // we found an error function call start
                                parser.reset_buffers();
                                parser.state = ParseState::Normal;
                                // now parse the error message string until the closing ')'
                                let mut message_buffer: Vec<char> = Vec::new();
                                loop {
                                    if let Some(next_char) = chars.next() {
                                        if next_char == ')' {
                                            break; // end of message string
                                        } else {
                                            message_buffer.push(next_char);
                                        }
                                    } else {
                                        return Err(ParseError::InputEndedUnexpectedly);
                                    }
                                }
                                let message: String = message_buffer.iter().collect();
                                push_token_at_end(&mut arena, Token::Error(message));
                            } else {
                                // just a normal char
                                parser.buffer.push(c);
                            }
                        }
                        _ => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_token_at_end(&mut arena, Token::Char(c));
                            // update the parser state and buffers
                            parser.state = ParseState::Normal;
                            parser.reset_buffers(); // reset buffer, because these symbols cannot be part of a function name etc
                        }
                    }
                },
                ParseState::PotRegisterCall => {
                    match c {
                        '^' => {
                            parser.depth += 1;   // increase the depth of the register call
                        },
                        '$' => {
                            // found a register call start
                            parser.state = ParseState::InRegisterCallParseIndex;
                        },
                        _ => {
                            // it was not a register call, go back to normal state and reparse this char
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            parser.state = ParseState::Normal;
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                            get_new_char = false; // do not get a new char in the next iteration
                        }
                    }
                },
                ParseState::InRegisterCallParseIndex => {
                    match c {
                        '0'..='9' => {
                            parser.index.push(c);
                        },
                        ' ' | '\n' | '\t' => {
                            // end of index parsing
                            if parser.index.is_empty() {
                                return Err(ParseError::IndexMissingInRegisterCall);
                            }
                            let index_str: String = parser.index.iter().collect();
                            let index: usize = index_str.parse().unwrap(); // safe to unwrap, we only have digits in the string
                            push_token_at_end(&mut arena, Token::RegisterCall(parser.depth, index));
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        _ => {
                            return Err(ParseError::InvalidRegisterIndexCharacter(c));
                        }
                    }
                },
                ParseState::ParsingDefFunctionName => {
                    match c {
                        ' ' => {
                            // end of function name
                            let func_name: String = parser.buffer.iter().collect();
                            if !parser.global {
                                push_token_at_end(&mut arena, Token::DefStart(func_name));
                            } else {
                                push_token_at_end(&mut arena, Token::DefGlobalStart(func_name));
                            }
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                            // expect a scope start next, which should not be added as a token
                            let pot_next_char = chars.next();
                            if pot_next_char != Some('{') {
                                return Err(ParseError::ExpectedScopeStartAfterFunctionDefinition);
                            }
                        },
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
                            // valid function name char, add to buffer
                            parser.buffer.push(c);
                        },
                        _ => {
                            return Err(ParseError::InvalidFunctionNameCharacter(c));
                        },
                    }
                },
                ParseState::PotComment => {
                    match c {
                        '/' => {
                            // confirmed comment start
                            parser.state = ParseState::InComment;

                        }
                        ch => {
                            // not a comment, but / was just a normal char
                            push_token_at_end(&mut arena, Token::Char('/'));
                            push_token_at_end(&mut arena, Token::Char(ch));
                            parser.state = ParseState::Normal;
                        },
                    }
                    // update the parser state and buffers
                    parser.reset_buffers();
                },
                ParseState::InComment => {
                    match c {
                        '\n' => {
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        _ => (), // stay in comment
                    }
                },
                ParseState::Escape => {
                    // push the next char as a Char token, regardless of what it is
                    push_token_at_end(&mut arena, Token::Char(c));
                    parser.state = ParseState::Normal;
                }
            }
        }
        match parser.state {
            ParseState::Normal | ParseState::PotComment | ParseState::PotRegisterCall => 
                            empty_buffer_at_end(&mut arena, &parser.buffer), // all good
            _ => {
                return Err(ParseError::InputEndedUnexpectedly);
            }
        };

        let mut lt = LinkedTokens { arena: arena.clone() }; // Todo remove the clone somehow, it should really not be needed
        // we now remove the whitespace after scope starts (including function calls etc), after scope ends and before and after Colons
        let mut start: usize = 0;
        let mut n: usize = 0;
        let mut after_scope_start = false;
        for (i, token_node) in arena.iter().enumerate() {
            match token_node.token {
                Token::GenericWhitespace => {
                    n += 1;
                },
                Token::Colon | Token::NewArm => {
                    lt.remove_range(start, n); // remove all preceeding whitespace
                    after_scope_start = true;   // set so that we remove all comming whitespace once finding a non-whitespace token
                    start = i;
                    n = 0;
                },
                Token::ScopeEnd => {
                    if after_scope_start {
                        lt.remove_range(start, n); // remove all preceeding whitespace
                        after_scope_start = false;
                    }
                    lt.remove_range(start, n); // remove all preceeding whitespace
                    after_scope_start = false;
                    start = i;
                    n = 0;
                },
                Token::ScopeStart | Token::DefGlobalStart(_) | Token::DefStart(_) | Token::FunctionCall(_) => {
                    if after_scope_start {
                        lt.remove_range(start, n); // remove all preceeding whitespace
                        after_scope_start = false;
                    }
                    after_scope_start = true;   // set so that we remove all comming whitespace once finding a non-whitespace token
                    start = i;
                    n = 0;
                },
                _ => {
                    if after_scope_start {
                        lt.remove_range(start, n); // remove all preceeding whitespace
                        after_scope_start = false;
                    }
                    start = i;
                    n = 0;
                }
            }
        }
        if after_scope_start {
            lt.remove_range(start, n); // remove all preceeding whitespace
            after_scope_start = false;
        }
        println!("TODO Fix whitespace removal");

        Ok(lt)
    }

    // inserts tokens after the token at the given index in the arena of self
    // index = 0 inserts after the root token node, i.e before the first actual token
    fn _insert_after(&mut self, index: usize, tokens: Vec<Token>) -> Result<(), LinkedTokensError> {
        // check boundaries
        if index >= self.arena.len() {
            return Err(LinkedTokensError::InsertionInvalidIndex);
        }
        if tokens.is_empty() {
            return Err(LinkedTokensError::InsertionEmptyTokens);
        }

        let len_arena = self.arena.len();
        let len_tokens = tokens.len();

        // retrieve the index of the token that comes after the insertion point currently
        // we must save this as a usize value before mutating the arena to avoid borrowing conflicts
        let old_next_index = self.arena[index].next;

        // iterate over the new tokens and add them to the arena
        for (i, t) in tokens.into_iter().enumerate() {
            // calculate the next link:
            // if it is the last new token, it points to the old_next_index
            // otherwise it points to the next new token (which will be at len_arena + i + 1)
            let next_link = if i == len_tokens - 1 {
                old_next_index
            } else {
                Some(len_arena + i + 1)
            };

            // push the constructed node to the arena
            self.arena.push(TokenNode {
                token: t,
                next: next_link,
            });
        }

        // update the next link of the token after which we inserted to point to the first new token
        self.arena[index].next = Some(len_arena);

        Ok(())
    }

    // removes n tokens after the token at index in the arena of self
    // does not remove the token at index itself
    fn remove_range(&mut self, index: usize, n: usize) -> Result<(), LinkedTokensError> {
        let len_arena = self.arena.len();
        if index >= len_arena {
            return Err(LinkedTokensError::RemovalInvalidIndex);
        }
        if n == 0 { return Ok(()); };

        // We use an index cursor instead of a reference to avoid borrowing issues
        let mut current_index = index;

        // go forward n steps from token to token. If there is no next token then the range is too big
        for _ in 0..n {
            // We look up the index, copy the next index, and move on.
            // We do not hold a reference to the token here.
            if let Some(next_token_index) = self.arena[current_index].next {
                current_index = next_token_index;
            } else {
                return Err(LinkedTokensError::RemovalRangeTooBig);
            }
        }

        // current_index is now the index of the last token to be removed.
        // We need the index of the token coming AFTER the removed range.
        let new_next_index = self.arena[current_index].next;

        // adapt the next link of the token before the removed range
        self.arena[index].next = new_next_index;

        Ok(())
    }

    // removes all tokens between start_index and end_index (exclusive)
    // effectively just links the token at start_index to the token at end_index
    fn _remove_between_indices(&mut self, start_index: usize, end_index: usize) -> Result<(), LinkedTokensError> {
        let len_arena = self.arena.len();
        if start_index >= len_arena || end_index >= len_arena {
            return Err(LinkedTokensError::RemovalInvalidIndex);
        }
        self.arena[start_index].next = Some(end_index);
        Ok(())
    }

    // rebuilds the arena to remove unused token nodes
    fn _collect_garbage(&mut self) {
        // initialize a new arena with a new root node
        let mut new_arena = vec![
            TokenNode {
                token: Token::RootNodeToken,
                next: Some(1),
            }
        ];
        let mut current_index = 0;
        let current_token_node = self.arena[0].clone(); // start at the root token
        // iter over the linked tokens and push them to the new arena
        // if we find a token which does not have a next, then we stop
        while let Some(next_token_node) = current_token_node.next {
            current_index += 1;
            new_arena.push(
                TokenNode {
                    token: self.arena[next_token_node].token.clone(),
                    next: Some(current_index + 1),
                }
            )
        }
        // the last token we added does not have a next token, remove it again
        new_arena.last_mut().unwrap().next = None;
        // replace the old arena with the new one
        self.arena = new_arena;
    }

    pub fn to_raw_string(&self) -> String {
        let mut result = String::new();
        let mut current_token_node = &self.arena[0]; // start at the root token
        // iter over the linked tokens and build the string representation
        while let Some(next_token_index) = current_token_node.next {
            let next_token_node = &self.arena[next_token_index];
            match &next_token_node.token {
                Token::Char(c) => result.push(*c),
                Token::ScopeStart => result.push('{'),
                Token::ScopeEnd => result.push('}'),
                Token::Colon => result.push(':'),
                Token::NewArm => result.push(';'),
                Token::FunctionCall(name) => {
                    result.push_str(name);
                    result.push('{');
                },
                Token::DefStart(name) => {
                    result.push_str("def ");
                    result.push_str(name);
                    result.push(' ');
                    result.push('{');
                },
                Token::DefGlobalStart(name) => {
                    result.push_str("*def ");
                    result.push_str(name);
                    result.push(' ');
                    result.push('{');
                }
                Token::RegisterCall(depth, index) => {
                    for _ in 0..*depth {
                        result.push('^');
                    }
                    result.push('$');
                    result.push_str(&index.to_string());
                },
                Token::GetInput(prompt) => {
                    result.push_str("get_input(");
                    result.push_str(prompt);
                    result.push(')');
                },
                Token::Error(message) => {
                    result.push_str("error(");
                    result.push_str(message);
                    result.push(')');
                },
                Token::GenericWhitespace => result.push(' '),
                Token::RootNodeToken => (), // do nothing for root node
            }
            current_token_node = next_token_node;
        }
        result

    }
}

// push a single new token to the arena, linking it to the current last token
// WARNING: this only wokrs correctly if the last token in the arena is actually the last token in the linked list
// otherwise behavior is undefined
fn push_token_at_end(arena: &mut Vec<TokenNode>, token: Token) {
    let new_index = arena.len();
    // link the current last token to the new token
    arena.last_mut().unwrap().next = Some(new_index);
    // push the new token to the arena, linking it back to the current last token
    arena.push(TokenNode {
        token,
        next: None,
    });
}

// push the contents of the buffer as Char tokens to the arena, linking them to the current last token
// WARNING: this only wokrs correctly if the last token in the arena is actually the last token in the linked list
// otherwise behavior is undefined
fn empty_buffer_at_end(arena: &mut Vec<TokenNode>, buffer: &Vec<char>) {
    for c in buffer {
        push_token_at_end(arena, Token::Char(*c));
    }
}

use std::fmt;

// implementation of the Display trait for Token
// handles the conversion of a single token to its string representation
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Char(c) => write!(f, "{}", c),
            Token::ScopeStart => write!(f, "{{"),
            Token::ScopeEnd => write!(f, "}}"),
            Token::Colon => write!(f, ":"),
            Token::NewArm => write!(f, ";"),
            Token::FunctionCall(name) => {
                write!(f, "{}{{", name)
            },
            Token::DefStart(name) => {
                write!(f, "def {} {{", name)
            },
            Token::DefGlobalStart(name) => {
                write!(f, "*def {} {{", name)
            }
            Token::RegisterCall(depth, index) => {
                // write the depth markers
                for _ in 0..*depth {
                    write!(f, "^")?;
                }
                // write the index marker and value
                write!(f, "${}", index)
            },
            Token::GetInput(prompt) => {
                write!(f, "get_input({})", prompt)
            },
            Token::Error(message) => {
                write!(f, "error({})", message)
            },
            Token::GenericWhitespace => write!(f, " "),
            Token::RootNodeToken => Ok(()), // the root token has no string representation
        }
    }
}

// implementation of the Display trait for TokenNode
// simply delegates the printing to the contained token, ignoring prev/next links
impl fmt::Display for TokenNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.token)
    }
}

// implementation of the Display trait for LinkedTokens
// allows printing the struct using the {} formatter
// traverses the list logically via indices to ensure correct order
impl fmt::Display for LinkedTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // start traversal at the root node (always index 0)
        let mut current_index = 0;
        
        // loop through the list following the 'next' pointers
        // we access the arena directly but follow the logical linked list order
        while let Some(next_index) = self.arena[current_index].next {
            // retrieve the next node from the arena
            // we use get() to be safe, though a valid list should always allow indexing
            if let Some(node) = self.arena.get(next_index) {
                write!(f, "{}\n", node)?;
                current_index = next_index;
            } else {
                // should not happen in a valid list, stop printing to avoid panic
                break;
            }
        }
        Ok(())
    }
}