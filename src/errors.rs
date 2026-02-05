


pub enum FindingScopeError {
    FoundEndingBraceBeforeStartingBrace,
    NoEndingBrace,
    MalformedOrMissingInput(String),
    MalformedOrMissingPattern(String),
    MalformedOrMissingOutput(String),
}

pub enum LinkedTokensError {
    InsertionInvalidIndex,
    InsertionEmptyTokens,
    RemovalInvalidIndex,
    RemovalRangeTooBig,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFunctionNameCharacter(char),
    InvalidRegisterIndexCharacter(char),
    IndexMissingInRegisterCall,
    ExpectedScopeStartAfterFunctionDefinition,
    InputEndedUnexpectedly,
    ExpectedFunctionNameAfterDef,
}