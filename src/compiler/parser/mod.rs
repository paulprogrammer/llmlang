use crate::compiler::lexer::{Token, Lexer};
use crate::compiler::ast::{Expr, Param};

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    filename: String,
}

impl Parser {
    pub fn new(mut lexer: Lexer, filename: String) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token, filename }
    }

    fn report_error(&self, code: &str) -> ! {
        panic!("{} ({}:{})", code, self.filename, self.lexer.line);
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
            Token::String(s) => {
                let val = s.clone();
                self.consume();
                Expr::String(val)
            }
            Token::Len => {
                self.consume();
                Expr::Len(Box::new(self.parse_expr()))
            }
            Token::Cat => {
                self.consume();
                let left = self.parse_expr();
                let right = self.parse_expr();
                Expr::Cat(Box::new(left), Box::new(right))
            }
            Token::StrSub => {
                self.consume();
                let s = self.parse_expr();
                let start = self.parse_expr();
                let len = self.parse_expr();
                Expr::Sub(Box::new(s), Box::new(start), Box::new(len))
            }
            Token::Loc => {
                self.consume();
                let s = self.parse_expr();
                let p = self.parse_expr();
                Expr::Loc(Box::new(s), Box::new(p))
            }
            Token::Reg => {
                self.consume();
                let s = self.parse_expr();
                let r = self.parse_expr();
                Expr::Reg(Box::new(s), Box::new(r))
            }
            Token::Read => {
                self.consume();
                Expr::Read(Box::new(self.parse_expr()))
            }
            Token::Write => {
                self.consume();
                let h = self.parse_expr();
                let s = self.parse_expr();
                Expr::Write(Box::new(h), Box::new(s))
            }
            Token::Str => {
                self.consume();
                Expr::Str(Box::new(self.parse_expr()))
            }
            Token::Split => {
                self.consume();
                let s = self.parse_expr();
                let d = self.parse_expr();
                let i = self.parse_expr();
                Expr::Split(Box::new(s), Box::new(d), Box::new(i))
            }
            Token::TimeNow => {
                self.consume();
                Expr::TimeNow
            }
            Token::TimeGet => {
                self.consume();
                let t = self.parse_expr();
                let i = self.parse_expr();
                Expr::TimeGet(Box::new(t), Box::new(i))
            }
            Token::TimeSet => {
                self.consume();
                let y = self.parse_expr();
                let m = self.parse_expr();
                let d = self.parse_expr();
                let h = self.parse_expr();
                let mn = self.parse_expr();
                let s = self.parse_expr();
                Expr::TimeSet(Box::new(y), Box::new(m), Box::new(d), Box::new(h), Box::new(mn), Box::new(s))
            }
            Token::Env => {
                self.consume();
                Expr::Env(Box::new(self.parse_expr()))
            }
            Token::Dot => {
                self.consume();
                let e1 = self.parse_expr();
                let e2 = self.parse_expr();
                Expr::Seq(Box::new(e1), Box::new(e2))
            }
            Token::Add | Token::Sub | Token::Mul | Token::Div |
            Token::Eq | Token::Lt | Token::Gt | 
            Token::BitAnd | Token::BitOr | Token::BitXor => {
                let op = self.consume();
                let left = self.parse_expr();
                let right = self.parse_expr();
                Expr::BinaryOp(op, Box::new(left), Box::new(right))
            }
            Token::Apply(arity) => {
                let arity = *arity;
                self.consume(); // consume @
                let func = self.parse_expr();
                let mut args = Vec::new();
                for _ in 0..arity {
                    args.push(self.parse_expr());
                }
                Expr::Apply(Box::new(func), args)
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
                    Expr::Shape(name, fields, false)
                } else {
                    self.report_error("E002");
                }
            }
            Token::Define => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let mut args = Vec::new();
                    loop {
                        match &self.current_token {
                            Token::Identifier(arg_name) => {
                                args.push(Param { name: arg_name.clone(), expand: false });
                                self.consume();
                            }
                            Token::Bang => {
                                self.consume();
                                if let Token::Identifier(arg_name) = self.consume() {
                                    args.push(Param { name: arg_name.clone(), expand: true });
                                } else {
                                    self.report_error("E002");
                                }
                            }
                            _ => break,
                        }
                    }
                    let body = self.parse_expr();
                    Expr::Define(name, args, Box::new(body), false)
                } else {
                    self.report_error("E002");
                }
            }
            Token::Export => {
                self.consume();
                match self.parse_expr() {
                    Expr::Shape(n, f, _) => Expr::Shape(n, f, true),
                    Expr::Define(n, a, b, _) => Expr::Define(n, a, b, true),
                    _ => self.report_error("E015"),
                }
            }
            Token::New => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let count = self.parse_expr();
                    Expr::New(name, Box::new(count))
                } else {
                    self.report_error("E002");
                }
            }
            Token::Get => {
                self.consume();
                let instance = self.parse_expr();
                if let Token::Identifier(field) = self.consume() {
                    let index = self.parse_expr();
                    Expr::Get(Box::new(instance), field, Box::new(index))
                } else {
                    self.report_error("E002");
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
                    self.report_error("E002");
                }
            }
            Token::Question => {
                self.consume();
                let cond = self.parse_expr();
                let true_branch = self.parse_expr();
                let false_branch = self.parse_expr();
                Expr::If(Box::new(cond), Box::new(true_branch), Box::new(false_branch))
            }
            Token::Bang => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    Expr::Expand(name)
                } else {
                    self.report_error("E002");
                }
            }
            Token::Let => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let val = self.parse_expr();
                    let body = self.parse_expr();
                    Expr::Let(name, Box::new(val), Box::new(body))
                } else {
                    self.report_error("E002");
                }
            }
            Token::Import => {
                self.consume();
                if let Token::Identifier(module) = self.consume() {
                    if let Token::Identifier(symbol) = self.consume() {
                        Expr::Import(module, symbol)
                    } else {
                        self.report_error("E002");
                    }
                } else {
                    self.report_error("E002");
                }
            }
            Token::EOF => self.report_error("E000"),
        }
    }

    pub fn parse_module(&mut self) -> Vec<Expr> {
        let mut exprs = Vec::new();
        while self.current_token != Token::EOF {
            exprs.push(self.parse_expr());
        }
        exprs
    }
}
