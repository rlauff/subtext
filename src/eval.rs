use crate::linked_tokens::*;
use crate::errors::EvalError;

// evaluates a simple scope (i.e. a scope without nested scopes, function or register calls)
// modifies the linked tokens in place
pub fn eval_simple_scope(linked_tokens: &mut LinkedTokens, start_index: usize) -> Result<(), EvalError> {
    let len_arena = linked_tokens.arena.len();
    if start_index >= len_arena {
        return Err(EvalError::StartIndexOutOfBounds);
    }
    let mut current_index = linked_tokens.arena[start_index]
        .next
        .ok_or(EvalError::MissingInput)?;


    // get the input string. It is located beween start_index and the first colon
    let mut input_string = String::new();
    loop {
        match &linked_tokens.arena[current_index].token {
            Token::Char(c) => input_string.push(*c),
            Token::Colon => break,
            Token::ScopeEnd => return Err(EvalError::ScopeEndedWhileParsingInput),
            _ => return Err(EvalError::ScopeNotSimple),
        }
        current_index = linked_tokens.arena[current_index]
            .next
            .ok_or(EvalError::ScopeEndedWhileParsingInput)?;
    }
    // now current_index is at the colon, advance to the next token
    current_index = linked_tokens.arena[current_index]
        .next
        .ok_or(EvalError::MissingPattern)?;


    unimplemented!()
}