use crate::lexer::{Token, Lexer};

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct Param {
    pub name: String,
    pub expand: bool, // true if marked with !
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
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
    Define(String, Vec<Param>, Box<Expr>, bool), // : name (args) body, exported?
    Shape(String, Vec<String>, bool),             // # name field1 field2 ..., exported?
    New(String, Box<Expr>),                 // N shape_name count
    Get(Box<Expr>, String, Box<Expr>),      // G instance field index
    Set(Box<Expr>, String, Box<Expr>, Box<Expr>), // S instance field index value
    If(Box<Expr>, Box<Expr>, Box<Expr>),    // ? cond true_branch false_branch
    Expand(String),                         // ! name (reference to expand param)
    Let(String, Box<Expr>, Box<Expr>),      // L name val body
    Import(String, String),                 // I module_alias symbol_name
}

impl Expr {
    pub fn get_calls(&self) -> Vec<String> {
        let mut calls = Vec::new();
        self.collect_calls(&mut calls);
        calls
    }

    fn collect_calls(&self, calls: &mut Vec<String>) {
        match self {
            Expr::BinaryOp(_, left, right) => {
                left.collect_calls(calls);
                right.collect_calls(calls);
            }
            Expr::Apply(func_expr, args) => {
                if let Expr::Identifier(name) = &**func_expr {
                    calls.push(name.clone());
                }
                func_expr.collect_calls(calls);
                for arg in args {
                    arg.collect_calls(calls);
                }
            }
            Expr::Move(expr) | Expr::Borrow(expr) | Expr::MutBorrow(expr) => {
                expr.collect_calls(calls);
            }
            Expr::Define(_, _, body, _) => {
                body.collect_calls(calls);
            }
            Expr::If(cond, t, f) => {
                cond.collect_calls(calls);
                t.collect_calls(calls);
                f.collect_calls(calls);
            }
            Expr::Let(_, val, body) => {
                val.collect_calls(calls);
                body.collect_calls(calls);
            }
            Expr::New(_, count) => {
                count.collect_calls(calls);
            }
            Expr::Get(inst, _, idx) => {
                inst.collect_calls(calls);
                idx.collect_calls(calls);
            }
            Expr::Set(inst, _, idx, val) => {
                inst.collect_calls(calls);
                idx.collect_calls(calls);
                val.collect_calls(calls);
            }
            _ => {}
        }
    }

    pub fn structural_fingerprint(&self) -> String {
        let mut s = String::new();
        self.collect_fingerprint(&mut s);
        s
    }

    fn collect_fingerprint(&self, s: &mut String) {
        match self {
            Expr::Integer(_) => s.push_str("i"),
            Expr::Float(_) => s.push_str("f"),
            Expr::DeBruijn(n) => s.push_str(&format!("^{}", n)),
            Expr::Identifier(_) => s.push_str("n"),
            Expr::BinaryOp(tok, left, right) => {
                s.push_str(&format!("{:?}", tok));
                left.collect_fingerprint(s);
                right.collect_fingerprint(s);
            }
            Expr::Apply(_, args) => {
                s.push_str("@");
                for arg in args {
                    arg.collect_fingerprint(s);
                }
            }
            Expr::Move(e) => { s.push_str(">"); e.collect_fingerprint(s); }
            Expr::Borrow(e) => { s.push_str("&"); e.collect_fingerprint(s); }
            Expr::MutBorrow(e) => { s.push_str("~"); e.collect_fingerprint(s); }
            Expr::Define(_, params, body, _) => {
                s.push_str(":");
                for _ in params { s.push_str("p"); }
                body.collect_fingerprint(s);
            }
            Expr::Shape(_, fields, _) => {
                s.push_str("#");
                for _ in fields { s.push_str("t"); }
            }
            Expr::New(_, count) => {
                s.push_str("N");
                count.collect_fingerprint(s);
            }
            Expr::Get(inst, _, idx) => {
                s.push_str("G");
                inst.collect_fingerprint(s);
                idx.collect_fingerprint(s);
            }
            Expr::Set(inst, _, idx, val) => {
                s.push_str("S");
                inst.collect_fingerprint(s);
                idx.collect_fingerprint(s);
                val.collect_fingerprint(s);
            }
            Expr::If(c, t, f) => {
                s.push_str("?");
                c.collect_fingerprint(s);
                t.collect_fingerprint(s);
                f.collect_fingerprint(s);
            }
            Expr::Expand(_) => s.push_str("!"),
            Expr::Let(_, val, body) => {
                s.push_str("L");
                val.collect_fingerprint(s);
                body.collect_fingerprint(s);
            }
            Expr::Import(_, _) => s.push_str("I"),
        }
    }
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
                let mut args = Vec::new();
                while self.current_token != Token::EOF && self.current_token != Token::Define && self.current_token != Token::Shape && self.current_token != Token::Let && self.current_token != Token::Import {
                     args.push(self.parse_expr());
                     if args.len() == 1 { break; } 
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
                    panic!("E002");
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
                                    panic!("E002");
                                }
                            }
                            _ => break,
                        }
                    }
                    let body = self.parse_expr();
                    Expr::Define(name, args, Box::new(body), false)
                } else {
                    panic!("E002");
                }
            }
            Token::Export => {
                self.consume();
                match self.parse_expr() {
                    Expr::Shape(n, f, _) => Expr::Shape(n, f, true),
                    Expr::Define(n, a, b, _) => Expr::Define(n, a, b, true),
                    _ => panic!("E015"),
                }
            }
            Token::New => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let count = self.parse_expr();
                    Expr::New(name, Box::new(count))
                } else {
                    panic!("E002");
                }
            }
            Token::Get => {
                self.consume();
                let instance = self.parse_expr();
                if let Token::Identifier(field) = self.consume() {
                    let index = self.parse_expr();
                    Expr::Get(Box::new(instance), field, Box::new(index))
                } else {
                    panic!("E002");
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
                    panic!("E002");
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
                    panic!("E002");
                }
            }
            Token::Let => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let val = self.parse_expr();
                    let body = self.parse_expr();
                    Expr::Let(name, Box::new(val), Box::new(body))
                } else {
                    panic!("E002");
                }
            }
            Token::Import => {
                self.consume();
                if let Token::Identifier(module) = self.consume() {
                    if let Token::Identifier(symbol) = self.consume() {
                        Expr::Import(module, symbol)
                    } else {
                        panic!("E002");
                    }
                } else {
                    panic!("E002");
                }
            }
            Token::EOF => panic!("E000"),
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
