#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Apply,     // @
    Add,       // +
    Sub,       // -
    Mul,       // *
    Div,       // /
    Move,      // >
    Borrow,    // &
    MutBorrow, // ~
    Define,    // :
    Shape,     // #
    DeBruijn(usize), // ^0, ^1
    New,       // new
    Get,       // get
    Set,       // set
    Identifier(String),
    Integer(i64),
    Float(f64),
    EOF,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
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
            '@' => Token::Apply,
            '+' => Token::Add,
            '-' => Token::Sub,
            '*' => Token::Mul,
            '/' => Token::Div,
            '>' => Token::Move,
            '&' => Token::Borrow,
            '~' => Token::MutBorrow,
            ':' => Token::Define,
            '#' => Token::Shape,
            '^' => self.lex_debruijn(),
            '0'..='9' => self.lex_number(ch),
            'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(ch),
            _ => panic!("Unexpected character: {}", ch),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
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
            "new" => Token::New,
            "get" => Token::Get,
            "set" => Token::Set,
            _ => Token::Identifier(id_str),
        }
    }

    fn lex_debruijn(&mut self) -> Token {
        let mut num_str = String::new();
        while self.pos < self.input.len() && self.input[self.pos].is_digit(10) {
            num_str.push(self.input[self.pos]);
            self.pos += 1;
        }
        Token::DeBruijn(num_str.parse().unwrap_or(0))
    }
}
