pub mod compiler {
    pub mod error;
    pub mod lexer;
    pub mod ast;
    pub mod parser;
    pub mod codegen;
    pub mod analysis;
}

pub mod testing;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub parallel_threshold: usize,
    pub max_threads: usize,
    pub queue_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            parallel_threshold: 50,
            max_threads: 8,
            queue_size: 64,
        }
    }
}
