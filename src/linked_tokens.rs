

use crate::errors::LinkedTokensError;
use crate::errors::ParseError;
use crate::parser::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
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
    Print(String),              // print token with message
    GenericWhitespace           // A generic Whitespace token to terminate register calls etc
}

#[derive(Clone, Debug)]
pub struct TokenNode {
    pub token: Token,
    pub next: Option<usize>,    // index of the next token in the arena
}
// the linked tokes struct
// the first token node is always the one at index 0 in the arena
// it does not correspond to any actual token, but is meant as a root for the linked list
// hence, if we want to insert at the very start, we insert after index 0
#[derive(Clone)]
pub struct LinkedTokens {
    pub arena: Vec<TokenNode>,
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
                                push_while_parsing(&mut arena, Token::FunctionCall(func_name));
                            } else {    // this scope is stand alone
                                push_while_parsing(&mut arena, Token::ScopeStart);
                            }
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                            
                        },
                        '}' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_while_parsing(&mut arena, Token::ScopeEnd);
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        ':' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_while_parsing(&mut arena, Token::Colon);
                            // update the parser state and buffers
                            parser.reset_buffers();
                            parser.state = ParseState::Normal;
                        },
                        ';' => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_while_parsing(&mut arena, Token::NewArm);
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
                                push_while_parsing(&mut arena, Token::GenericWhitespace);
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
                                push_while_parsing(&mut arena, Token::GetInput(prompt));
                            } else if parser.buffer == vec!['p', 'r', 'i', 'n', 't'] {
                                // we found a print function call start
                                parser.reset_buffers();
                                parser.state = ParseState::Normal;
                                // now parse the message string until the closing ')'
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
                                push_while_parsing(&mut arena, Token::Print(message));
                            } else if parser.buffer == vec!['e', 'r', 'r', 'o', 'r'] {
                                // we found a error function call start
                                parser.reset_buffers();
                                parser.state = ParseState::Normal;
                                // now parse the message string until the closing ')'
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
                                push_while_parsing(&mut arena, Token::Error(message));
                            } else {
                                // just a normal char
                                parser.buffer.push(c);
                            }
                        }
                        _ => {
                            empty_buffer_at_end(&mut arena, &parser.buffer);
                            push_while_parsing(&mut arena, Token::Char(c));
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
                            push_while_parsing(&mut arena, Token::RegisterCall(parser.depth, index));
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
                                push_while_parsing(&mut arena, Token::DefStart(func_name));
                            } else {
                                push_while_parsing(&mut arena, Token::DefGlobalStart(func_name));
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
                            push_while_parsing(&mut arena, Token::Char('/'));
                            push_while_parsing(&mut arena, Token::Char(ch));
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
                    push_while_parsing(&mut arena, Token::Char(c));
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

        

        Ok(LinkedTokens { arena })
    }

    // after replacing a part, we might have created a new register or function call
    // this function reparses the linked tokens to update the token types accordingly
    fn reparse(&mut self) -> Result<(), ParseError> {
        let mut parser = Parser::new();
        let mut current_node_index = 0; // start at the root node
        current_node_index = self.arena[current_node_index].next.unwrap(); // move to the first actual token
        loop {
            if let Some(next_index) = self.arena[current_node_index].next {
                current_node_index = next_index;
            } else {
                break; // end of the list
            }
            match self.arena[current_node_index].token {
                Token::Char(c) => {
                    match c {
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
                            // valid function name char, add to buffer
                            parser.buffer.push(c);
                        },
                        '(' => {
                            // the only thing that can happen is that we build up a function call from multiple parts
                            // we want to allow this to make some quirky things possible, like replacing a part of a function name and still have it work as a function call, as long as the final result is a valid function call
                            // however, this could be removed as a functionality without affecting the capabilities of the language in a meaningful way.
                            // TODO: actually implement this or decide to leave it out
                        }
                        _ => {
                            // any other character would end a potential function name etc.
                            // note that whitespace characters are tokenized and stay that way
                            // same with register calls, they are tokenized as separate tokens and cannot be "completed" by adding more chars, so we do not need to worry about them here
                            // therefore, this cannot be a function call or register call
                        }
                    }
                },
                _ => {}
            }
        }
        Ok(())
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
                Token::Print(message) => {
                    result.push_str("print(");
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
// skip if the token is a GenericWhitespace and the last pushed token was also a whitespace, to avoid multiple consecutive whitespace tokens
fn push_while_parsing(arena: &mut Vec<TokenNode>, token: Token) {
    if token == Token::GenericWhitespace {
        if let Some(last_token_node) = arena.last() {
            if last_token_node.token == Token::GenericWhitespace {
                return; // skip pushing another whitespace token
            }
        }
    }
    let new_index = arena.len();
    // link the current last token to the new token
    arena.last_mut().unwrap().next = Some(new_index);
    // push the new token to the arena,
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
        push_while_parsing(arena, Token::Char(*c));
    }
}


// Display implementations by Gemini 3. Non-criticals
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
            Token::FunctionCall(name) => write!(f, "{}{{", name),
            Token::DefStart(name) => write!(f, "def {} {{", name),
            Token::DefGlobalStart(name) => write!(f, "*def {} {{", name),
            Token::RegisterCall(depth, index) => {
                // write the depth markers
                for _ in 0..*depth {
                    write!(f, "^")?;
                }
                // write the index marker and value
                write!(f, "${}", index)
            },
            Token::GetInput(prompt) => write!(f, "get_input({})", prompt),
            Token::Error(message) => write!(f, "error({})", message),
            Token::Print(message) => write!(f, "print({})", message),
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