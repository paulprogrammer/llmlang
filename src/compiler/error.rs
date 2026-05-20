use std::fmt;
use std::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub code: String,
    pub filename: String,
    pub line: usize,
}

impl CompileError {
    pub fn new(code: &str, filename: &str, line: usize) -> Self {
        Self {
            code: code.to_string(),
            filename: filename.to_string(),
            line,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}:{})", self.code, self.filename, self.line)
    }
}

impl Error for CompileError {}
