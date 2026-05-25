#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub enum Token {
    Apply(usize), // @ or @2, @3...
    Add,       // +
    Sub,       // -
    Mul,       // *
    Div,       // /
    Move,      // >
    Borrow,    // $
    MutBorrow, // ~
    Define,    // :
    Shape,     // #
    DeBruijn(usize), // ^0, ^1
    Question,  // ?
    Bang,      // `
    New,       // N
    Get,       // G
    Set,       // S
    Export,    // X
    Let,       // L
    Import,    // I
    Eq,        // =
    Lt,        // lt
    Gt,        // gt
    Dot,       // .
    BitAnd,    // &
    BitOr,     // |
    BitXor,    // xor
    String(String),
    Len,       // sl
    Cat,       // sc
    StrSub,    // ss
    OtelEmit(usize), // oe
    Loc,       // sf
    Reg,       // sr
    Read,      // (
    Write,     // )
    Str,       // str
    Split,     // sp
    Pack(usize), // jp (Pack/Unpack)
    Map,       // map
    Filter,    // flt
    Money,     // %
    Panic,     // !
    Trap,      // ^
    TimeNow,   // tn
    TimeNano,  // tns
    TimeZone,  // tz
    TimeGet,   // tg
    TimeSet,   // ts
    Metadata,  // M
    Env,       // env
    HttpClient, // http
    HttpServer, // srv
    FileOpen,   // fo
    Router,     // rt
    HttpHeader, // hdr
    Identifier(String),
    Integer(i64),
    Float(f64),
    EOF,
}

use crate::compiler::error::CompileError;

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    pub line: usize,
    pub filename: String,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            filename: String::new(),
        }
    }

    pub fn next_token(&mut self) -> Result<Token, CompileError> {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Ok(Token::EOF);
        }

        let ch = self.input[self.pos];
        self.pos += 1;

        let tok = match ch {
            '@' => self.lex_arity_token(Token::Apply(1)),
            '+' => Token::Add,
            '-' => Token::Sub,
            '*' => Token::Mul,
            '/' => self.lex_slash()?,
            '>' => Token::Move,
            '$' => Token::Borrow,
            '~' => Token::MutBorrow,
            '=' => Token::Eq,
            '<' => Token::Lt,
            '.' => Token::Dot,
            '&' => Token::BitAnd,
            '|' => Token::BitOr,
            ':' => Token::Define,
            '#' => Token::Shape,
            '?' => Token::Question,
            '!' => Token::Panic,
            '`' => Token::Bang,
            '(' => Token::Read,
            ')' => Token::Write,
            '%' => Token::Money,
            'N' => Token::New,
            'G' => Token::Get,
            'S' => Token::Set,
            'M' => Token::Metadata,
            'X' => Token::Export,
            'L' => Token::Let,
            'I' => Token::Import,
            '^' => self.lex_trap_or_debruijn(),
            '"' => self.lex_string(),
            '0'..='9' => self.lex_number(ch),
            'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(ch),
            _ => return Err(CompileError::new(&format!("E001: Invalid char '{}' (0x{:x})", ch, ch as u32), &self.filename, self.line)),
        };
        Ok(tok)
    }

    fn lex_arity_token(&mut self, default: Token) -> Token {
        let mut num_str = String::new();
        while self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
            num_str.push(self.input[self.pos]);
            self.pos += 1;
        }
        if num_str.is_empty() {
            default
        } else {
            let arity = num_str.parse().unwrap_or(1);
            match default {
                Token::Apply(_) => Token::Apply(arity),
                Token::Pack(_) => Token::Pack(arity),
                _ => default,
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
            if self.input[self.pos] == '\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
    }

    fn lex_number(&mut self, first_ch: char) -> Token {
        let mut num_str = first_ch.to_string();
        while self.pos < self.input.len() && (self.input[self.pos].is_digit(10) || self.input[self.pos] == '.') {
            num_str.push(self.input[self.pos]);
            self.pos += 1;
        }

        if num_str.contains('.') {
            Token::Float(num_str.parse().unwrap())
        } else {
            Token::Integer(num_str.parse().unwrap())
        }
    }

    fn lex_identifier(&mut self, first_ch: char) -> Token {
        let mut id_str = first_ch.to_string();
        while self.pos < self.input.len() && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_') {
            id_str.push(self.input[self.pos]);
            self.pos += 1;
        }
        match id_str.as_str() {
            "map" => Token::Map,
            "flt" => Token::Filter,
            "http" => Token::HttpClient,
            "srv" => Token::HttpServer,
            "rt" => Token::Router,
            "hdr" => Token::HttpHeader,
            "env" => Token::Env,
            "fo" => Token::FileOpen,
            "jp" => Token::Pack(1),
            "ju" => Token::Pack(2),
            "oe" => Token::OtelEmit(4),
            "sl" => Token::Len,
            "sc" => Token::Cat,
            "ss" => Token::StrSub,
            "sf" => Token::Loc,
            "sr" => Token::Reg,
            "sp" => Token::Split,
            "tn" => Token::TimeNow,
            "tns" => Token::TimeNano,
            "tz" => Token::TimeZone,
            "tg" => Token::TimeGet,
            "ts" => Token::TimeSet,
            "str" => Token::Str,
            "gt" => Token::Gt,
            "lt" => Token::Lt,
            "xor" => Token::BitXor,
            _ => Token::Identifier(id_str),
        }
    }

    fn lex_trap_or_debruijn(&mut self) -> Token {
        if self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
            let mut num_str = String::new();
            while self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
                num_str.push(self.input[self.pos]);
                self.pos += 1;
            }
            Token::DeBruijn(num_str.parse().unwrap_or(0))
        } else {
            Token::Trap
        }
    }

    fn lex_string(&mut self) -> Token {
        let mut s = String::new();
        while self.pos < self.input.len() && self.input[self.pos] != '"' {
            if self.input[self.pos] == '\\' {
                self.pos += 1;
                if self.pos < self.input.len() {
                    match self.input[self.pos] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        _ => s.push(self.input[self.pos]),
                    }
                    self.pos += 1;
                }
            } else {
                s.push(self.input[self.pos]);
                self.pos += 1;
            }
        }
        if self.pos < self.input.len() {
            self.pos += 1; // consume "
        }
        Token::String(s)
    }

    fn lex_slash(&mut self) -> Result<Token, CompileError> {
        if self.pos < self.input.len() && self.input[self.pos] == '/' {
            while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                self.pos += 1;
            }
            if self.pos < self.input.len() && self.input[self.pos] == '\n' {
                self.pos += 1;
                self.line += 1;
            }
            self.next_token()
        } else {
            Ok(Token::Div)
        }
    }
}
