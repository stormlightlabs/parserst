use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected end of input")]
    Eof,
    #[error("invalid syntax at line {line}: {msg}")]
    Invalid { line: usize, msg: String },
}
