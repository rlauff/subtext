use thiserror::Error;


#[derive(Debug, Error)]
pub enum FindingScopeError {
    #[error("Found ending brace before starting brace")]
    FoundEndingBraceBeforeStartingBrace,
    #[error("No matching ending brace found")]
    NoEndingBrace,
    #[error("Malformed or missing input: {0}")]
    MalformedOrMissingInput(String),
    #[error("Malformed or missing pattern: {0}")]
    MalformedOrMissingPattern(String),
    #[error("Malformed or missing output: {0}")]
    MalformedOrMissingOutput(String),
    #[error("Arms not separated by semicolon: {0}")]
    ArmsNotSeparatedBySemicolon(String),
}

enum LinkedTokensError {
    InsertionInvalidINdex,
    InsertionEmptyTokens,
    RemovalINvalidINdex,
    RemovalRangeTooBig,
}

enum ParserError {
    InvalidFunctionNameCharacter(char),
    InvalidRegisterIndexCharacter(char),
    IndexMissingInRegisterCall,
    ExpectedScopeStartAfterFunctionDefinition,
    InputEndedUnexpectedly,
}