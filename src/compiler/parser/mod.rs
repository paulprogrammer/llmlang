use crate::compiler::lexer::{Token, Lexer};
use crate::compiler::ast::{Expr, Param};
use crate::compiler::error::CompileError;

pub trait SignatureResolver {
    fn resolve(&self, module: &str, import_paths: &[String], filename: &str) -> Result<String, String>;
}

pub struct FsSignatureResolver;

impl SignatureResolver for FsSignatureResolver {
    fn resolve(&self, module: &str, import_paths: &[String], filename: &str) -> Result<String, String> {
        if module == "http" {
            return Ok(": get 1\n: post 2\n: serve 1\n: https_serve 4\n: accept 1\n: respond 2\n: decode 1\n: get_header 2\n".to_string());
        }
        if module == "db" {
            return Ok(": connect 2\n: connect_binding 2\n: query 4\n: exec 3\n: error 1\n".to_string());
        }
        if module == "json" {
            return Ok(": parse 1\n: stringify 1\n: get_int 2\n: get_float 2\n: get_str 2\n: get_obj 2\n: get_arr 2\n: arr_len 1\n: arr_get 2\n".to_string());
        }
        if module == "file" {
            return Ok(": open 2\n: close 1\n".to_string());
        }
        if module == "crypto" {
            return Ok(": sign 2\n: verify 3\n: encrypt 2\n: decrypt 2\n".to_string());
        }
        if module == "cms" {
            return Ok(": unwrap 2\n".to_string());
        }
        use std::path::Path;
        let sig_filename = format!("{}.llmi", module);
        let mut found_path = None;

        // 1. Try search paths specified via -I first
        for path_str in import_paths {
            let p = Path::new(path_str).join(&sig_filename);
            if p.exists() {
                found_path = Some(p);
                break;
            }
        }

        // 2. Try the directory of the file currently being compiled
        if found_path.is_none() {
            if let Some(parent) = Path::new(filename).parent() {
                let p = parent.join(&sig_filename);
                if p.exists() {
                    found_path = Some(p);
                }
            }
        }

        // 3. Try the current working directory
        if found_path.is_none() {
            let p = Path::new(&sig_filename);
            if p.exists() {
                found_path = Some(p.to_path_buf());
            }
        }

        if let Some(path) = found_path {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        } else {
            Err("E017".to_string()) // Import module missing
        }
    }
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    filename: String,
    scopes: Vec<Vec<String>>,
    pub import_paths: Vec<String>,
    pub imported_shapes: Vec<(String, String)>,
    pub resolver: Box<dyn SignatureResolver>,
    pub generated_funcs: Vec<Expr>,
}

impl Parser {
    pub fn new(mut lexer: Lexer, filename: String) -> Result<Self, CompileError> {
        lexer.filename = filename.clone();
        let current_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token,
            filename,
            scopes: Vec::new(),
            import_paths: Vec::new(),
            imported_shapes: Vec::new(),
            resolver: Box::new(FsSignatureResolver),
            generated_funcs: Vec::new(),
        })
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

    fn resolve_shape_name(&self, name: String) -> String {
        for (sym, module) in &self.imported_shapes {
            if sym == &name {
                return format!("{}_{}", module, name);
            }
        }
        name
    }

    fn load_signature(&self, module: &str) -> Result<(Vec<(String, usize)>, Vec<(String, Vec<String>)>), CompileError> {
        let content = self.resolver.resolve(module, &self.import_paths, &self.filename).map_err(|err_code| {
            self.error(&err_code)
        })?;
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
        Ok((funcs, shapes))
    }

    fn error(&self, code: &str) -> CompileError {
        CompileError::new(code, &self.filename, self.lexer.line)
    }

    fn consume(&mut self) -> Result<Token, CompileError> {
        let tok = self.current_token.clone();
        self.current_token = self.lexer.next_token()?;
        Ok(tok)
    }

    pub fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        let res = match &self.current_token {
            Token::Integer(i) => {
                let val = *i;
                self.consume()?;
                Expr::Integer(val)
            }
            Token::Float(f) => {
                let val = *f;
                self.consume()?;
                Expr::Float(val)
            }
            Token::Identifier(s) => {
                let val = s.clone();
                if let Some(idx) = self.resolve_variable(&val) {
                    self.consume()?;
                    Expr::DeBruijn(idx)
                } else {
                    self.consume()?;
                    Expr::Identifier(val)
                }
            }
            Token::String(s) => {
                let val = s.clone();
                self.consume()?;
                Expr::String(val)
            }
            Token::Move => {
                self.consume()?;
                Expr::Move(Box::new(self.parse_expr()?))
            }
            Token::Borrow => {
                self.consume()?;
                Expr::Borrow(Box::new(self.parse_expr()?))
            }
            Token::MutBorrow => {
                self.consume()?;
                Expr::MutBorrow(Box::new(self.parse_expr()?))
            }
            Token::Len => {
                self.consume()?;
                Expr::Len(Box::new(self.parse_expr()?))
            }
            Token::StrSub => {
                self.consume()?;
                let s = self.parse_expr()?;
                let b = self.parse_expr()?;
                let l = self.parse_expr()?;
                Expr::Sub(Box::new(s), Box::new(b), Box::new(l))
            }
            Token::Cat => {
                self.consume()?;
                let left = self.parse_expr()?;
                let right = self.parse_expr()?;
                Expr::Cat(Box::new(left), Box::new(right))
            }
            Token::Let => {
                self.consume()?;
                if let Token::Identifier(name) = self.consume()? {
                    let val = self.parse_expr()?;
                    self.add_variable(name.clone());
                    let body = self.parse_expr()?;
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.pop();
                    }
                    Expr::Let(name, Box::new(val), Box::new(body))
                } else {
                    return Err(self.error("E002"));
                }
            }
            Token::Define => {
                self.consume()?;
                let name = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                self.push_scope();
                let mut params = Vec::new();
                while let Token::Identifier(p) = &self.current_token {
                    let p_name = p.clone();
                    self.consume()?;
                    let mut expand = false;
                    if let Token::Bang = self.current_token {
                        self.consume()?;
                        expand = true;
                    }
                    self.add_variable(p_name.clone());
                    params.push(Param { name: p_name, expand });
                }
                let body = self.parse_expr()?;
                self.pop_scope();
                Expr::Define(name, params, Box::new(body), false)
            }
            Token::Shape => {
                self.consume()?;
                let name = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let mut fields = Vec::new();
                while let Token::Identifier(f) = &self.current_token {
                    fields.push(f.clone());
                    self.consume()?;
                }
                Expr::Shape(name, fields, false)
            }
            Token::New => {
                self.consume()?;
                let name = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let resolved_name = self.resolve_shape_name(name);
                let count = self.parse_expr()?;
                Expr::New(resolved_name, Box::new(count))
            }
            Token::Get => {
                self.consume()?;
                let inst = self.parse_expr()?;
                let field = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let idx = self.parse_expr()?;
                Expr::Get(Box::new(inst), field, Box::new(idx))
            }
            Token::Set => {
                self.consume()?;
                let inst = self.parse_expr()?;
                let field = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let idx = self.parse_expr()?;
                let val = self.parse_expr()?;
                Expr::Set(Box::new(inst), field, Box::new(idx), Box::new(val))
            }
            Token::Question => {
                self.consume()?;
                let cond = self.parse_expr()?;
                let t = self.parse_expr()?;
                let f = self.parse_expr()?;
                Expr::If(Box::new(cond), Box::new(t), Box::new(f))
            }
            Token::Bang => {
                self.consume()?;
                if let Token::Identifier(name) = self.consume()? {
                    Expr::Expand(name)
                } else {
                    return Err(self.error("E002"));
                }
            }
            Token::Metadata => {
                self.consume()?;
                let tag = self.parse_expr()?;
                let val = self.parse_expr()?;
                let target = self.parse_expr()?;
                if let Expr::String(tag_str) = &tag {
                    if tag_str == "test" {
                        let mut inner = &target;
                        while let Expr::Metadata(_, _, t) = inner {
                            inner = t;
                        }
                        if !matches!(inner, Expr::Define(_, _, _, _)) {
                            return Err(self.error("E019"));
                        }
                    }
                }
                Expr::Metadata(Box::new(tag), Box::new(val), Box::new(target))
            }
            Token::Export => {
                self.consume()?;
                let mut inner = self.parse_expr()?;
                match &mut inner {
                    Expr::Define(_, _, _, exported) => *exported = true,
                    Expr::Shape(_, _, exported) => *exported = true,
                    _ => return Err(self.error("E015")),
                }
                inner
            }
            Token::Money => {
                self.consume()?;
                match &self.current_token {
                    Token::Str => {
                        self.consume()?;
                        Expr::MoneyStr(Box::new(self.parse_expr()?))
                    }
                    Token::Add | Token::Sub | Token::Mul | Token::Div => {
                        let op = self.consume()?;
                        let left = self.parse_expr()?;
                        let right = self.parse_expr()?;
                        Expr::MoneyOp(op, Box::new(left), Box::new(right))
                    }
                    Token::Integer(i) => {
                        let val = *i;
                        self.consume()?;
                        Expr::Integer(val * 10000)
                    }
                    Token::Float(f) => {
                        let val = *f;
                        self.consume()?;
                        Expr::Integer((val * 10000.0) as i64)
                    }
                    _ => return Err(self.error("E002")),
                }
            }
            Token::Import => {
                self.consume()?;
                let module = match self.consume()? {
                    Token::Identifier(s) => s,
                    Token::HttpClient => "http".to_string(),
                    _ => return Err(self.error("E002")),
                };
                let symbol = if let Token::Identifier(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let (funcs, shapes) = self.load_signature(&module)?;
                
                // If the symbol is a shape, return it as a Shape expr so codegen registers it
                if let Some((name, fields)) = shapes.iter().find(|(name, _)| name == &symbol) {
                    self.imported_shapes.push((name.clone(), module.clone()));
                    let mangled_name = format!("{}_{}", module, name);
                    Expr::Shape(mangled_name, fields.clone(), false)
                } else if let Some((_, arity)) = funcs.iter().find(|(name, _)| name == &symbol) {
                    Expr::Import(module, symbol, *arity)
                } else {
                    return Err(self.error("E018"));
                }
            }
            Token::Apply(arity) => {
                let arity = *arity;
                self.consume()?;
                let func = self.parse_expr()?;
                let mut args = Vec::new();
                for _ in 0..arity {
                    args.push(self.parse_expr()?);
                }
                Expr::Apply(Box::new(func), args)
            }
            Token::DeBruijn(index) => {
                let val = *index;
                self.consume()?;
                Expr::DeBruijn(val)
            }
            Token::Loc => {
                self.consume()?;
                let s = self.parse_expr()?;
                let p = self.parse_expr()?;
                Expr::Loc(Box::new(s), Box::new(p))
            }
            Token::Reg => {
                self.consume()?;
                let s = self.parse_expr()?;
                let r = self.parse_expr()?;
                Expr::Reg(Box::new(s), Box::new(r))
            }
            Token::Read => {
                self.consume()?;
                Expr::Read(Box::new(self.parse_expr()?))
            }
            Token::Write => {
                self.consume()?;
                let h = self.parse_expr()?;
                let s = self.parse_expr()?;
                Expr::Write(Box::new(h), Box::new(s))
            }
            Token::Str => {
                self.consume()?;
                Expr::Str(Box::new(self.parse_expr()?))
            }
            Token::Split => {
                self.consume()?;
                let s = self.parse_expr()?;
                let d = self.parse_expr()?;
                let i = self.parse_expr()?;
                Expr::Split(Box::new(s), Box::new(d), Box::new(i))
            }
            Token::Pack(arity) => {
                let arity = *arity;
                self.consume()?;
                if arity == 1 {
                    Expr::Pack(Box::new(self.parse_expr()?))
                } else {
                    let s = self.parse_expr()?;
                    if let Token::String(shape_name) = self.consume()? {
                        let resolved_name = self.resolve_shape_name(shape_name);
                        Expr::Unpack(Box::new(s), resolved_name)
                    } else {
                        return Err(self.error("E002"));
                    }
                }
            }
            Token::OtelEmit(_) => {
                self.consume()?;
                let t = self.parse_expr()?;
                let a1 = self.parse_expr()?;
                let a2 = self.parse_expr()?;
                let a3 = self.parse_expr()?;
                Expr::OtelEmit(Box::new(t), Box::new(a1), Box::new(a2), Box::new(a3))
            }
            Token::Map => {
                self.consume()?;
                let inst = self.parse_expr()?;
                let field = if let Token::String(s) = self.consume()? { s } else { return Err(self.error("E002")); };
                let func = self.parse_expr()?;
                Expr::Map(Box::new(inst), field, Box::new(func))
            }
            Token::Filter => {
                self.consume()?;
                let inst = self.parse_expr()?;
                let func = self.parse_expr()?;
                Expr::Filter(Box::new(inst), Box::new(func))
            }
            Token::Panic => {
                self.consume()?;
                Expr::Panic(Box::new(self.parse_expr()?))
            }
            Token::Trap => {
                self.consume()?;
                let try_expr = self.parse_expr()?;
                let fallback_expr = self.parse_expr()?;
                Expr::Trap(Box::new(try_expr), Box::new(fallback_expr))
            }
            Token::TimeNow => {
                self.consume()?;
                if let Token::Add | Token::Sub = self.current_token {
                    let op = self.consume()?;
                    let left = self.parse_expr()?;
                    let right = self.parse_expr()?;
                    Expr::TimeOp(op, Box::new(left), Box::new(right))
                } else {
                    Expr::TimeNow
                }
            }
            Token::TimeZone => {
                self.consume()?;
                Expr::TimeZone
            }
            Token::TimeNano => {
                self.consume()?;
                Expr::TimeNano
            }
            Token::TimeGet => {
                self.consume()?;
                let t = self.parse_expr()?;
                let i = self.parse_expr()?;
                Expr::TimeGet(Box::new(t), Box::new(i))
            }
            Token::TimeSet => {
                self.consume()?;
                let y = self.parse_expr()?;
                let m = self.parse_expr()?;
                let d = self.parse_expr()?;
                let h = self.parse_expr()?;
                let mn = self.parse_expr()?;
                let sc = self.parse_expr()?;
                Expr::TimeSet(Box::new(y), Box::new(m), Box::new(d), Box::new(h), Box::new(mn), Box::new(sc))
            }
            Token::Env => {
                self.consume()?;
                Expr::Env(Box::new(self.parse_expr()?))
            }
            Token::FileOpen => {
                self.consume()?;
                let path = self.parse_expr()?;
                let mode = self.parse_expr()?;
                Expr::FileOpen(Box::new(path), Box::new(mode))
            }
            Token::HttpClient => {
                self.consume()?;
                let method = self.parse_expr()?;
                let url = self.parse_expr()?;
                let body = self.parse_expr()?;
                Expr::HttpClient(Box::new(method), Box::new(url), Box::new(body))
            }
            Token::HttpServer => {
                self.consume()?;
                let op = self.parse_expr()?;
                let arg = self.parse_expr()?;
                Expr::HttpServer(Box::new(op), Box::new(arg))
            }
            Token::HttpHeader => {
                self.consume()?;
                let req = self.parse_expr()?;
                let name = self.parse_expr()?;
                Expr::HttpHeader(Box::new(req), Box::new(name))
            }
            Token::Router => {
                self.parse_router()?
            }
            Token::Dot => {
                self.consume()?;
                let e1 = self.parse_expr()?;
                let e2 = self.parse_expr()?;
                Expr::Seq(Box::new(e1), Box::new(e2))
            }
            Token::Add | Token::Sub | Token::Mul | Token::Div |
            Token::Eq | Token::Lt | Token::Gt | 
            Token::BitAnd | Token::BitOr | Token::BitXor => {
                let op = self.consume()?;
                let left = self.parse_expr()?;
                let right = self.parse_expr()?;
                Expr::BinaryOp(op, Box::new(left), Box::new(right))
            }
            _ => return Err(self.error("E001")),
        };
        Ok(res)
    }

    fn parse_router(&mut self) -> Result<Expr, CompileError> {
        self.consume()?; // consume Token::Router (rt)
        
        let port_expr = self.parse_expr()?;
        
        let num_routes = match self.consume()? {
            Token::Integer(n) => n,
            _ => return Err(self.error("E001")), // expected integer for route count
        };
        
        let mut routes = Vec::new();
        for _ in 0..num_routes {
            let method = self.parse_expr()?;
            let path = self.parse_expr()?;
            let handler = self.parse_expr()?;
            routes.push((method, path, handler));
        }
        
        let fallback = self.parse_expr()?;
        
        // Generate a unique name for the routing loop
        let loop_idx = self.generated_funcs.len() + 1;
        let loop_name = format!("_rt_loop_{}", loop_idx);
        
        // Build the dispatch tree using nested conditions
        let loop_identifier = Expr::Identifier(loop_name.clone());
        let recurse_call = Expr::Apply(
            Box::new(loop_identifier),
            vec![Expr::Move(Box::new(Expr::DeBruijn(4)))]
        );
        
        let mut dispatch_tree = Expr::Seq(
            Box::new(Expr::Apply(Box::new(fallback), vec![Expr::Move(Box::new(Expr::DeBruijn(3)))])),
            Box::new(recurse_call.clone())
        );
        
        for (method_expr, path_expr, handler_expr) in routes.into_iter().rev() {
            let method_pattern = match method_expr {
                Expr::String(s) => format!("^{}$", s),
                _ => return Err(self.error("E001")),
            };
            
            // Analyze path template and extract dynamic parameter segment indices
            let (path_pattern, param_segment_indices) = match path_expr {
                Expr::String(s) => {
                    let mut pattern_segments = Vec::new();
                    let mut indices = Vec::new();
                    let mut strtok_idx = 0;
                    let segments: Vec<&str> = s.split('/').collect();
                    for seg in segments {
                        if seg.is_empty() {
                            pattern_segments.push("".to_string());
                        } else {
                            if seg.starts_with(':') && seg.len() > 1 {
                                pattern_segments.push("[^/]+".to_string());
                                indices.push(strtok_idx);
                            } else {
                                pattern_segments.push(seg.to_string());
                            }
                            strtok_idx += 1;
                        }
                    }
                    let pattern = format!("^{}$", pattern_segments.join("/"));
                    (pattern, indices)
                }
                _ => return Err(self.error("E001")),
            };
            
            let method_eq = Expr::Reg(
                Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(0)))), // method is DeBruijn(0)
                Box::new(Expr::String(method_pattern)),
            );
            let path_eq = Expr::Reg(
                Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(1)))), // path is DeBruijn(1)
                Box::new(Expr::String(path_pattern)),
            );
            let cond = Expr::BinaryOp(
                Token::BitAnd,
                Box::new(method_eq),
                Box::new(path_eq),
            );
            
            // Build arguments list for handler: [> req, sp $ path "/" idx1, ...]
            let mut handler_args = vec![Expr::Move(Box::new(Expr::DeBruijn(3)))]; // req is DeBruijn(3)
            for idx in param_segment_indices {
                let extract_expr = Expr::Split(
                    Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(1)))), // path is DeBruijn(1)
                    Box::new(Expr::String("/".to_string())),
                    Box::new(Expr::Integer(idx as i64)),
                );
                handler_args.push(extract_expr);
            }
            
            let true_branch = Expr::Seq(
                Box::new(Expr::Apply(Box::new(handler_expr), handler_args)),
                Box::new(recurse_call.clone())
            );
            
            dispatch_tree = Expr::If(
                Box::new(cond),
                Box::new(true_branch),
                Box::new(dispatch_tree),
            );
        }
        
        let log_raw = Expr::Seq(
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::Cat(Box::new(Expr::String("raw_path: ".to_string())), Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(2)))))))),
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::String("\n".to_string()))))
        );
        let log_path = Expr::Seq(
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::Cat(Box::new(Expr::String("path: ".to_string())), Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(1)))))))),
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::String("\n".to_string()))))
        );
        let log_method = Expr::Seq(
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::Cat(Box::new(Expr::String("method: ".to_string())), Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(0)))))))),
            Box::new(Expr::Write(Box::new(Expr::Integer(1)), Box::new(Expr::String("\n".to_string()))))
        );
        let debug_prints = Expr::Seq(Box::new(log_raw), Box::new(Expr::Seq(Box::new(log_path), Box::new(log_method))));
        
        let dispatch_tree = Expr::Seq(Box::new(debug_prints), Box::new(dispatch_tree));

        // Stack layout builders:
        // L method srv 1 $ req (body has method at DeBruijn(0))
        let method_val = Expr::HttpServer(
            Box::new(Expr::Integer(1)),
            Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(2)))), // req is DeBruijn(2) before method is bound
        );
        let method_let = Expr::Let("method".to_string(), Box::new(method_val), Box::new(dispatch_tree));
        
        // L path sp $ raw_path "?" 0 (body has path at DeBruijn(0))
        let path_val = Expr::Split(
            Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(0)))), // raw_path is DeBruijn(0) before path is bound
            Box::new(Expr::String("?".to_string())),
            Box::new(Expr::Integer(0)),
        );
        let path_let = Expr::Let("path".to_string(), Box::new(path_val), Box::new(method_let));
        
        // L raw_path srv 2 $ req (body has raw_path at DeBruijn(0))
        let raw_path_val = Expr::HttpServer(
            Box::new(Expr::Integer(2)),
            Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(0)))), // req is DeBruijn(0) before raw_path is bound
        );
        let raw_path_let = Expr::Let("raw_path".to_string(), Box::new(raw_path_val), Box::new(path_let));
        
        // L req ( $ server (body has req at DeBruijn(0))
        let req_val = Expr::Read(Box::new(Expr::Borrow(Box::new(Expr::DeBruijn(0))))); // server is DeBruijn(0) before req is bound
        let req_let = Expr::Let("req".to_string(), Box::new(req_val), Box::new(raw_path_let));
        
        let loop_param = Param { name: "server".to_string(), expand: false };
        let loop_def = Expr::Define(
            loop_name.clone(),
            vec![loop_param],
            Box::new(req_let),
            false
        );
        
        self.generated_funcs.push(loop_def);
        
        let server_val = Expr::HttpServer(
            Box::new(Expr::Integer(0)),
            Box::new(port_expr)
        );
        
        let start_app = Expr::Apply(
            Box::new(Expr::Identifier(loop_name)),
            vec![Expr::Move(Box::new(Expr::DeBruijn(0)))]
        );
        
        let entry_expr = Expr::Let(
            "s".to_string(),
            Box::new(server_val),
            Box::new(start_app)
        );
        
        Ok(entry_expr)
    }

    pub fn parse_module(&mut self) -> Result<Vec<Expr>, CompileError> {
        let mut expressions = Vec::new();
        while self.current_token != Token::EOF {
            let expr = self.parse_expr()?;
            if !self.generated_funcs.is_empty() {
                expressions.extend(self.generated_funcs.drain(..));
            }
            expressions.push(expr);
        }
        Ok(expressions)
    }
}
