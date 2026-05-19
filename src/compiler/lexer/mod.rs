#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub enum Token {
    Apply(usize), // @ or @2, @3...
    Add,       // +
    Sub,       // -
    Mul,       // *
    Div,       // /
    Move,      // ⮞
    Borrow,    // ⚓
    MutBorrow, // ~
    Define,    // :
    Shape,     // #
    DeBruijn(usize), // ^0, ^1
    Question,  // ?
    Bang,      // !
    New,       // N
    Get,       // G
    Set,       // S
    Export,    // X
    Let,       // L
    Import,    // I
    Eq,        // =
    Lt,        // <
    Gt,        // >
    Dot,       // .
    BitAnd,    // &
    BitOr,     // |
    BitXor,    // ^
    String(String),
    Len,       // ℓ
    Cat,       // ⧉
    StrSub,    // ✂
    Loc,       // 🔍
    Reg,       // ≈
    Read,      // 📥
    Write,     // 📤
    Str,       // 🧵
    Split,     // 🪓
    Pack(usize), // 📦
    Map,       // ⟴
    Filter,    // ▽
    Money,     // 💰
    Panic,     // 🚨
    Trap,      // 🛡️
    TimeNow,   // 🕒
    TimeGet,   // 📅
    TimeSet,   // 📆
    Env,       // 🌍
    Identifier(String),
    Integer(i64),
    Float(f64),
    EOF,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    pub line: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Token::EOF;
        }

        let ch = self.input[self.pos];
        self.pos += 1;

        match ch {
            '@' => self.lex_arity_token(Token::Apply(1)),
            '+' => Token::Add,
            '-' => Token::Sub,
            '*' => Token::Mul,
            '/' => self.lex_slash(),
            '⮞' => Token::Move,
            '⚓' => Token::Borrow,
            '~' => Token::MutBorrow,
            '=' => Token::Eq,
            '<' => Token::Lt,
            '>' => Token::Gt,
            '.' => Token::Dot,
            '&' => Token::BitAnd,
            '|' => Token::BitOr,
            ':' => Token::Define,
            '#' => Token::Shape,
            '?' => Token::Question,
            '!' => Token::Bang,
            'N' => Token::New,
            'G' => Token::Get,
            'S' => Token::Set,
            'X' => Token::Export,
            'L' => Token::Let,
            'I' => Token::Import,
            '^' => self.lex_xor_or_debruijn(),
            '\u{2113}' => Token::Len,
            '\u{29C9}' => Token::Cat,
            '\u{2702}' => Token::StrSub,
            '\u{1F50D}' => Token::Loc,
            '\u{2248}' => Token::Reg,
            '\u{1F4E5}' => Token::Read,
            '\u{1F4E4}' => Token::Write,
            '\u{1F9F5}' => Token::Str,
            '\u{1FA93}' => Token::Split,
            '\u{1F4E6}' => self.lex_arity_token(Token::Pack(1)),
            '\u{27F4}' => Token::Map,
            '\u{25BD}' => Token::Filter,
            '\u{1F4B0}' => Token::Money,
            '\u{1F6A8}' => Token::Panic,
            '\u{1F6E1}' => Token::Trap,
            '\u{1F552}' => Token::TimeNow,
            '\u{1F4C5}' => Token::TimeGet,
            '\u{1F4C6}' => Token::TimeSet,
            '\u{1F30D}' => Token::Env,
            '"' => self.lex_string(),
            '0'..='9' => self.lex_number(ch),
            'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(ch),
            '\u{FE0F}' => self.next_token(),
            _ => panic!("Unexpected character: {}", ch),
        }
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
        Token::Identifier(id_str)
    }

    fn lex_xor_or_debruijn(&mut self) -> Token {
        if self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
            let mut num_str = String::new();
            while self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
                num_str.push(self.input[self.pos]);
                self.pos += 1;
            }
            Token::DeBruijn(num_str.parse().unwrap_or(0))
        } else {
            Token::BitXor
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

    fn lex_slash(&mut self) -> Token {
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
            Token::Div
        }
    }
}
