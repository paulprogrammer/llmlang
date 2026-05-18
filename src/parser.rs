use crate::lexer::{Token, Lexer};

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    DeBruijn(usize),
    Identifier(String),
    BinaryOp(Token, Box<Expr>, Box<Expr>), // +, -, *, /
    Apply(Box<Expr>, Vec<Expr>),           // @ func arg1 arg2 ...
    Move(Box<Expr>),                       // > expr
    Borrow(Box<Expr>),                     // & expr
    MutBorrow(Box<Expr>),                  // ~ expr
    Define(String, Vec<String>, Box<Expr>), // : name (args) body
    Shape(String, Vec<String>),             // # name field1 field2 ...
    New(String, Box<Expr>),                 // new shape_name count
    Get(Box<Expr>, String, Box<Expr>),      // get instance field index
    Set(Box<Expr>, String, Box<Expr>, Box<Expr>), // set instance field index value
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn consume(&mut self) -> Token {
        let tok = self.current_token.clone();
        self.current_token = self.lexer.next_token();
        tok
    }

    pub fn parse_expr(&mut self) -> Expr {
        match &self.current_token {
            Token::Integer(i) => {
                let val = *i;
                self.consume();
                Expr::Integer(val)
            }
            Token::Float(f) => {
                let val = *f;
                self.consume();
                Expr::Float(val)
            }
            Token::DeBruijn(i) => {
                let val = *i;
                self.consume();
                Expr::DeBruijn(val)
            }
            Token::Identifier(s) => {
                let val = s.clone();
                self.consume();
                Expr::Identifier(val)
            }
            Token::Add | Token::Sub | Token::Mul | Token::Div => {
                let op = self.consume();
                let left = self.parse_expr();
                let right = self.parse_expr();
                Expr::BinaryOp(op, Box::new(left), Box::new(right))
            }
            Token::Apply => {
                self.consume(); // consume @
                let func = self.parse_expr();
                // For now, let's assume @ takes a fixed number of args or we need a way to end the list.
                // In a pure prefix-arity language, every function has a known arity.
                // Let's assume for now @ takes 2 arguments as a placeholder.
                let arg1 = self.parse_expr();
                let arg2 = self.parse_expr();
                Expr::Apply(Box::new(func), vec![arg1, arg2])
            }
            Token::Move => {
                self.consume();
                Expr::Move(Box::new(self.parse_expr()))
            }
            Token::Borrow => {
                self.consume();
                Expr::Borrow(Box::new(self.parse_expr()))
            }
            Token::MutBorrow => {
                self.consume();
                Expr::MutBorrow(Box::new(self.parse_expr()))
            }
            Token::Shape => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let mut fields = Vec::new();
                    while let Token::Identifier(field_type) = &self.current_token {
                        fields.push(field_type.clone());
                        self.consume();
                    }
                    Expr::Shape(name, fields)
                } else {
                    panic!("Expected identifier after #");
                }
            }
            Token::Define => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let mut args = Vec::new();
                    while let Token::Identifier(arg) = &self.current_token {
                        args.push(arg.clone());
                        self.consume();
                    }
                    let body = self.parse_expr();
                    Expr::Define(name, args, Box::new(body))
                } else {
                    panic!("Expected identifier after :");
                }
            }
            Token::New => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let count = self.parse_expr();
                    Expr::New(name, Box::new(count))
                } else {
                    panic!("Expected identifier after new");
                }
            }
            Token::Get => {
                self.consume();
                let instance = self.parse_expr();
                if let Token::Identifier(field) = self.consume() {
                    let index = self.parse_expr();
                    Expr::Get(Box::new(instance), field, Box::new(index))
                } else {
                    panic!("Expected identifier for field in get");
                }
            }
            Token::Set => {
                self.consume();
                let instance = self.parse_expr();
                if let Token::Identifier(field) = self.consume() {
                    let index = self.parse_expr();
                    let value = self.parse_expr();
                    Expr::Set(Box::new(instance), field, Box::new(index), Box::new(value))
                } else {
                    panic!("Expected identifier for field in set");
                }
            }
            Token::EOF => panic!("Unexpected EOF"),
            _ => panic!("Unexpected token: {:?}", self.current_token),
        }
    }
}
