

pub enum FindingScopeError {
    FoundEndingBraceBeforeStartingBrace,
    NoEndingBrace,
    MalformedOrMissingInput(String),
    MalformedOrMissingPattern(String),
    MalformedOrMissingOutput(String),
    ArmsNotSeparatedBySemicolon(String),
}