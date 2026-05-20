use crate::compiler::lexer::{Token, Lexer};
use crate::compiler::ast::{Expr, Param};

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    filename: String,
    scopes: Vec<Vec<String>>,
}

impl Parser {
    pub fn new(mut lexer: Lexer, filename: String) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token, filename, scopes: Vec::new() }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn add_variable(&mut self, name: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name);
        }
    }

    fn resolve_variable(&self, name: &str) -> Option<usize> {
        let mut index = 0;
        for scope in self.scopes.iter().rev() {
            for (i, var) in scope.iter().rev().enumerate() {
                if var == name {
                    return Some(index + i);
                }
            }
            index += scope.len();
        }
        None
    }

    fn load_signature(&self, module: &str) -> (Vec<(String, usize)>, Vec<(String, Vec<String>)>) {
        use std::path::Path;
        let mut sig_path = format!("{}.llmi", module);
        
        // Try relative to current file first
        if let Some(parent) = Path::new(&self.filename).parent() {
            let rel_path = parent.join(format!("{}.llmi", module));
            if rel_path.exists() {
                sig_path = rel_path.to_str().unwrap().to_string();
            }
        }

        let content = std::fs::read_to_string(&sig_path).map_err(|e| format!("Could not read {}: {}", sig_path, e)).unwrap();
        let mut funcs = Vec::new();
        let mut shapes = Vec::new();
        for line in content.lines() {
            if line.starts_with(':') {
                let parts: Vec<&str> = line[1..].trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(arity) = parts[1].parse::<usize>() {
                        funcs.push((parts[0].to_string(), arity));
                    }
                }
            } else if line.starts_with('#') {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[1].to_string();
                    let fields = parts[2..].iter().map(|s| s.to_string()).collect();
                    shapes.push((name, fields));
                }
            }
        }
        (funcs, shapes)
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
        let res = match &self.current_token {
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
            Token::Identifier(s) => {
                let val = s.clone();
                if let Some(idx) = self.resolve_variable(&val) {
                    self.consume();
                    Expr::DeBruijn(idx)
                } else {
                    self.consume();
                    Expr::Identifier(val)
                }
            }
            Token::String(s) => {
                let val = s.clone();
                self.consume();
                Expr::String(val)
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
            Token::Len => {
                self.consume();
                Expr::Len(Box::new(self.parse_expr()))
            }
            Token::StrSub => {
                self.consume();
                let s = self.parse_expr();
                let b = self.parse_expr();
                let l = self.parse_expr();
                Expr::Sub(Box::new(s), Box::new(b), Box::new(l))
            }
            Token::Cat => {
                self.consume();
                let left = self.parse_expr();
                let right = self.parse_expr();
                Expr::Cat(Box::new(left), Box::new(right))
            }
            Token::Let => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    let val = self.parse_expr();
                    self.add_variable(name.clone());
                    let body = self.parse_expr();
                    Expr::Let(name, Box::new(val), Box::new(body))
                } else {
                    self.report_error("E002");
                }
            }
            Token::Define => {
                self.consume();
                let name = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                self.push_scope();
                let mut params = Vec::new();
                while let Token::Identifier(p) = &self.current_token {
                    let p_name = p.clone();
                    self.consume();
                    let mut expand = false;
                    if let Token::Bang = self.current_token {
                        self.consume();
                        expand = true;
                    }
                    self.add_variable(p_name.clone());
                    params.push(Param { name: p_name, expand });
                }
                let body = self.parse_expr();
                self.pop_scope();
                Expr::Define(name, params, Box::new(body), false)
            }
            Token::Shape => {
                self.consume();
                let name = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let mut fields = Vec::new();
                while let Token::Identifier(f) = &self.current_token {
                    fields.push(f.clone());
                    self.consume();
                }
                Expr::Shape(name, fields, false)
            }
            Token::New => {
                self.consume();
                let name = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let count = self.parse_expr();
                Expr::New(name, Box::new(count))
            }
            Token::Get => {
                self.consume();
                let inst = self.parse_expr();
                let field = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let idx = self.parse_expr();
                Expr::Get(Box::new(inst), field, Box::new(idx))
            }
            Token::Set => {
                self.consume();
                let inst = self.parse_expr();
                let field = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let idx = self.parse_expr();
                let val = self.parse_expr();
                Expr::Set(Box::new(inst), field, Box::new(idx), Box::new(val))
            }
            Token::Question => {
                self.consume();
                let cond = self.parse_expr();
                let t = self.parse_expr();
                let f = self.parse_expr();
                Expr::If(Box::new(cond), Box::new(t), Box::new(f))
            }
            Token::Bang => {
                self.consume();
                if let Token::Identifier(name) = self.consume() {
                    Expr::Expand(name)
                } else {
                    self.report_error("E002");
                }
            }
            Token::Export => {
                self.consume();
                let mut inner = self.parse_expr();
                match &mut inner {
                    Expr::Define(_, _, _, exported) => *exported = true,
                    Expr::Shape(_, _, exported) => *exported = true,
                    _ => self.report_error("E015"),
                }
                inner
            }
            Token::Money => {
                self.consume();
                match self.current_token {
                    Token::Str => {
                        self.consume();
                        Expr::MoneyStr(Box::new(self.parse_expr()))
                    }
                    Token::Add | Token::Sub | Token::Mul | Token::Div => {
                        let op = self.consume();
                        let left = self.parse_expr();
                        let right = self.parse_expr();
                        Expr::MoneyOp(op, Box::new(left), Box::new(right))
                    }
                    Token::Integer(i) => {
                        self.consume();
                        Expr::Integer(i * 10000)
                    }
                    Token::Float(f) => {
                        self.consume();
                        Expr::Integer((f * 10000.0) as i64)
                    }
                    _ => self.report_error("E002"),
                }
            }
            Token::Import => {
                self.consume();
                let module = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let symbol = if let Token::Identifier(s) = self.consume() { s } else { self.report_error("E002") };
                let (funcs, shapes) = self.load_signature(&module);
                
                // If the symbol is a shape, return it as a Shape expr so codegen registers it
                if let Some((name, fields)) = shapes.iter().find(|(name, _)| name == &symbol) {
                    Expr::Shape(name.clone(), fields.clone(), false)
                } else {
                    let arity = funcs.iter().find(|(name, _)| name == &symbol).map(|(_, a)| *a).unwrap_or(1);
                    Expr::Import(module, symbol, arity)
                }
            }
            Token::Apply(arity) => {
                let arity = *arity;
                self.consume();
                let func = self.parse_expr();
                let mut args = Vec::new();
                for _ in 0..arity {
                    args.push(self.parse_expr());
                }
                Expr::Apply(Box::new(func), args)
            }
            Token::DeBruijn(index) => {
                let val = *index;
                self.consume();
                Expr::DeBruijn(val)
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
            Token::Pack(arity) => {
                let arity = *arity;
                self.consume();
                if arity == 1 {
                    Expr::Pack(Box::new(self.parse_expr()))
                } else {
                    let s = self.parse_expr();
                    if let Token::String(shape_name) = self.consume() {
                        Expr::Unpack(Box::new(s), shape_name)
                    } else {
                        self.report_error("E002");
                    }
                }
            }
            Token::Map => {
                self.consume();
                let inst = self.parse_expr();
                let field = if let Token::String(s) = self.consume() { s } else { self.report_error("E002") };
                let func = self.parse_expr();
                Expr::Map(Box::new(inst), field, Box::new(func))
            }
            Token::Filter => {
                self.consume();
                let inst = self.parse_expr();
                let func = self.parse_expr();
                Expr::Filter(Box::new(inst), Box::new(func))
            }
            Token::Panic => {
                self.consume();
                Expr::Panic(Box::new(self.parse_expr()))
            }
            Token::Trap => {
                self.consume();
                let try_expr = self.parse_expr();
                let fallback_expr = self.parse_expr();
                Expr::Trap(Box::new(try_expr), Box::new(fallback_expr))
            }
            Token::TimeNow => {
                self.consume();
                if let Token::Add | Token::Sub = self.current_token {
                    let op = self.consume();
                    let left = self.parse_expr();
                    let right = self.parse_expr();
                    Expr::TimeOp(op, Box::new(left), Box::new(right))
                } else if let Token::Env = self.current_token {
                    self.consume();
                    Expr::TimeZone
                } else {
                    Expr::TimeNow
                }
            }
            Token::TimeNano => {
                self.consume();
                Expr::TimeNano
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
                let sc = self.parse_expr();
                Expr::TimeSet(Box::new(y), Box::new(m), Box::new(d), Box::new(h), Box::new(mn), Box::new(sc))
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
            _ => self.report_error("E001"),
        };
        res
    }

    pub fn parse_module(&mut self) -> Vec<Expr> {
        let mut expressions = Vec::new();
        while self.current_token != Token::EOF {
            expressions.push(self.parse_expr());
        }
        expressions
    }
}
