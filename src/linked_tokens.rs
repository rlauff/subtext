
use std::f32::consts::E;
use thiserror::Error;

use crate::errors::LinkedTokensError;
use crate::errors::ParserError;
use crate::parser::*;


enum Token {
    Char(char),
    ScopeStart,
    ScopeEnd,
    Colon,                      // separates input, pattern and output
    ArmEnd,
    FunctionStart(String),      // function name, ended by a ScopeEnd
    DefStart(String),           // name of the defined function
    DefEnd,
    RegisterCall(usize, usize), // depth, index
    RootNodeToken,              // a placeholder token for the root node
}

struct TokenNode {
    token: Token,
    next: Option<usize>,    // index of the next token in the arena
    prev: Option<usize>,    // index of the previous token in the arena
}
// the linked tokes struct
// the first token node is always the one at index 0 in the arena
// it does not correspond to any actual token, but is meant as a root for the linked list
// hence, if we want to insert at the very start, we insert after index 0
struct LinkedTokens {
    arena: Vec<TokenNode>,
}

macro_rules! push_token {
    ($t:expr) => {
        {
            i += 1;
            // linking the previous token to the new one
            arena[i - 1].next = Some(i);
            
            arena.push(TokenNode {
                token: $t,
                next: None,
                prev: Some(i - 1),
            });
            parser.reset_buffers();
            parser.state = ParseState::Normal;
        }
    };
}

impl LinkedTokens {
    // parses a string into linked tokens
    fn from_string(s: &str) -> Result<Self, ParseError> {
        // initialize the arena with the root node
        let mut arena = vec![
            TokenNode {
                token: Token::RootNodeToken,
                next: None,
                prev: None,
            }
        ];
        let mut parser = Parser::new();
        let chars = s.chars();
        let mut c = ' ';                // the current charcter being parsed
        let mut i = 0;                 // the current index in the arena
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
                                push_token!(Token::FunctionStart(func_name));
                            } else {    // this scope is stand alone
                                push_token!(Token::ScopeStart);
                            }
                        },
                        '}' => {
                            push_token!(Token::ScopeEnd);
                        },
                        ':' => {
                            push_token!(Token::Colon);
                        },
                        ';' => {
                            push_token!(Token::ArmEnd);
                        },
                        '^' => {
                            // found a potential register call
                            parser.state = ParseState::PotRegisterCall;
                            parser.depth = 1;   // we found one '^' so init depth as 1
                        },
                        '$' => {
                            // found a register call start
                            parser.state = ParseState::InRegisterCallParseIndex;
                        },
                        ' ' => {
                            // if the buffer is exactly def, we found a function definition start
                            if parser.buffer == vec!['d', 'e', 'f'] {
                                parser.state = ParseState::ParsingDefFunctionName;
                            } else {
                                // reset the buffer
                                parser.reset_buffers();
                            }
                        },
                        '/' => {
                            // found a potential comment start
                            parser.state = ParseState::PotCommentStart;
                        },
                        'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            // valid function name char, add to buffer
                            parser.buffer.push(c);
                        },
                        '\n' | '\t' | '\r' => {
                            // ignore whitespace characters
                        },
                        _ => {
                            push_token!(Token::Char(c));
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
                        ' ' => {
                            // end of index parsing
                            if parser.index.is_empty() {
                                return Err(ParseError::IndexMissingInRegisterCall);
                            }
                            let index_str: String = parser.index.iter().collect();
                            let index: usize = index_str.parse().unwrap(); // safe to unwrap, we only have digits in the string
                            push_token!(Token::RegisterCall(parser.depth, index));
                            parser.state = ParseState::JustFoundToken;
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
                            push_token!(Token::DefStart(func_name));
                            parser.state = ParseState::JustFoundToken;
                            // expect a scope start next, which should not be added as a token
                            let pot_next_char = chars.next();
                            if pot_next_char != Some('{') {
                                return Err(ParseError::ExpectedScopeStartAfterFunctionDefinition);
                            }
                        },
                        'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            // valid function name char, add to buffer
                            parser.buffer.push(c);
                        },
                        _ => {
                            return Err(ParseError::InvalidFunctionNameCharacter(c));
                        },
                    }
                },
                ParseState::PotCommentStart => {
                    match c {
                        '/' => {
                            // confirmed comment start
                            parser.state = ParseState::InComment;
                            parser.reset_buffers(); // dont know if this is needed, just to be safe
                        },
                        _ => {
                            // false alarm, go back to normal state and reparse this char
                            parser.state = ParseState::Normal;
                            get_new_char = false; // do not get a new char in the next iteration
                        },
                    }
                },
                ParseState::InComment => {
                    match c {
                        '\n' => {
                            parser.state = ParseState::Normal;
                        },
                        _ => (), // stay in comment
                    }
                },
            }
        }
        match parser.state {
            ParseState::Normal | ParseState::PotCommentStart | ParseState::PotRegisterCall => (), // all good
            _ => {
                return Err(ParseError::InputEndedUnexpectedly);
            }
        };

        Ok(LinkedTokens { arena })
    }

    // inserts tokens after the token at the given index in the arena of self
    // index = 0 inserts after the root token node, i.e before the first actual token
    fn insert_after(&mut self, index: usize, tokens: Vec<Token>) -> Result<(), LinkedTokensError> {
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

            // calculate the prev link:
            // if it is the first new token, it points back to the insertion index
            // otherwise it points to the previous new token (which is at len_arena + i - 1)
            let prev_link = if i == 0 {
                Some(index)
            } else {
                Some(len_arena + i - 1)
            };

            // push the constructed node to the arena
            self.arena.push(TokenNode {
                token: t,
                next: next_link,
                prev: prev_link,
            });
        }

        // update the next link of the token after which we inserted to point to the first new token
        self.arena[index].next = Some(len_arena);

        // update the prev link of the token that used to follow the insertion point
        // it must now point to the last of the newly inserted tokens
        if let Some(next_index) = old_next_index {
            // we access the arena directly by index here
            self.arena[next_index].prev = Some(len_arena + len_tokens - 1);
        }

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
        let mut token = &self.arena[index];
        let mut current_token = token;
        // go forward n steps from token to token. If there is no next token then the range is too big
        for _ in 0..n {
            if let Some(next_token_index) = current_token.next {
                current_token = &self.arena[next_token_index];
            } else {
                return Err(LinkedTokensError::RemovalRangeTooBig);
            }
        }
        let new_next_index = current_token.next;
        // adapt the next link of the token before the removed range
        token.next = new_next_index;
        // adapt the prev link of the token after the removed range, if it exists
        if let Some(next_index) = new_next_index {
            let mut next_token = &self.arena[next_index];
            next_token.prev = Some(index);
        };
        Ok(())
    }

    // removes all tokens between start_index and end_index (exclusive)
    // effectively just links the token at start_index to the token at end_index
    fn remove_between_indices(&mut self, start_index: usize, end_index: usize) -> Result<(), LinkedTokensError> {
        let len_arena = self.arena.len();
        if start_index >= len_arena || end_index >= len_arena {
            return Err(LinkedTokensError::RemovalInvalidIndex);
        }
        self.arena[start_index].next = Some(end_index);
        self.arena[end_index].prev = Some(start_index);
        Ok(())
    }

    // rebuilds the arena to remove unised token nodes
    fn collect_garbage(&mut self) {
        // initialize a new arena with a new root node
        let mut new_arena = vec![
            TokenNode {
                token: Token::RootNodeToken,
                next: Some(1),
                prev: None,
            }
        ];
        let mut current_index = 0;
        let mut current_token_node = self.arena[0].clone(); // start at the root token
        // iter over the linked tokens and push them to the new arena
        // if we find a token which does not have a next, then we stop
        while let Some(token) = current_token_node.next.token {
            current_index += 1;
            new_arena.push(
                TokenNode {
                    token: token,
                    next: Some(current_index + 1),
                    prev: Some(current_index - 1),
                }
            )
        }
        // the last token we added does not have a next token, remove it again
        new_arena.last_mut().unwrap().next = None;
        // replace the old arena with the new one
        self.arena = new_arena;
    }
}