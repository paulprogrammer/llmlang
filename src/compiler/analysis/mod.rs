pub mod verify;

use std::collections::HashMap;
use crate::compiler::ast::{Expr};

impl Expr {
    pub fn returns_ptr_with_stack(&self, stack_ptrs: &[bool], fn_returns_ptr: &HashMap<String, bool>) -> bool {
        match self {
            Expr::String(_) | Expr::New(_, _) | Expr::Unpack(_, _) | Expr::Map(_, _, _) | Expr::Filter(_, _) => true,
            Expr::Cat(_, _) | Expr::Sub(_, _, _) | Expr::Read(_) | Expr::Str(_) | Expr::Split(_, _, _) | Expr::Pack(_) | Expr::Env(_) | Expr::MoneyStr(_) | Expr::TimeZone | Expr::FileOpen(_, _) => true,
            Expr::HttpClient(_, _, _) => true,
            Expr::HttpServer(op, _) => match &**op {
                Expr::Integer(0) | Expr::Integer(1) | Expr::Integer(2) | Expr::Integer(3) => true,
                _ => false,
            },
            Expr::HttpHeader(_, _) => true,
            Expr::Let(_, val_expr, body_expr) => {
                let val_ptr = val_expr.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr);
                let mut new_stack = stack_ptrs.to_vec();
                new_stack.push(val_ptr);
                body_expr.returns_ptr_with_stack(&new_stack, fn_returns_ptr)
            }
            Expr::Seq(_, body) => body.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr),
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) => e.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr),
            Expr::Trap(t, f) | Expr::If(_, t, f) => t.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr) || f.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr),
            Expr::Set(_, _, _, v) => v.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr),
            Expr::DeBruijn(idx) => {
                if *idx < stack_ptrs.len() {
                    stack_ptrs[stack_ptrs.len() - 1 - idx]
                } else {
                    false
                }
            }
            Expr::Apply(f, _) => {
                if let Expr::Identifier(ref name) = **f {
                    let is_ptr_fn = match name.as_str() {
                        "http_get" | "http_post" | "get" | "post" |
                        "json_parse" | "parse" |
                        "json_stringify" | "stringify" |
                        "json_get_str" | "get_str" |
                        "sign" | "encrypt" | "decrypt" | "unwrap" |
                        "serve" | "https_serve" | "accept" |
                        "decode" | "encode_b64url" | "decode_b64url" | "greet" |
                        "connect" | "connect_binding" | "query" | "error" |
                        "db_connect" | "db_connect_binding" | "db_query" | "db_error" => true,
                        _ => name.ends_with("_get") || name.ends_with("_post") ||
                             name.ends_with("_parse") || name.ends_with("_stringify") ||
                             name.ends_with("_get_str") || name.ends_with("_sign") ||
                             name.ends_with("_encrypt") || name.ends_with("_decrypt") ||
                             name.ends_with("_unwrap") || name.ends_with("_serve") ||
                             name.ends_with("_https_serve") || name.ends_with("_accept") ||
                             name.ends_with("_decode") || name.ends_with("_encode_b64url") ||
                             name.ends_with("_decode_b64url") || name.ends_with("_greet") ||
                             name.ends_with("_connect") || name.ends_with("_connect_binding") ||
                             name.ends_with("_query") || name.ends_with("_error")
                    };
                    if is_ptr_fn {
                        return true;
                    }
                    if let Some(&ret) = fn_returns_ptr.get(name) {
                        return ret;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn returns_ptr(&self) -> bool {
        self.returns_ptr_with_stack(&[], &HashMap::new())
    }

    pub fn is_pure(&self) -> bool {
        match self {
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Identifier(_) | Expr::DeBruijn(_) | Expr::Expand(_) => true,
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) | Expr::Len(e) | Expr::Str(e) => e.is_pure(),
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Seq(l, r) | Expr::BinaryOp(_, l, r) => l.is_pure() && r.is_pure(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => s.is_pure() && b.is_pure() && l.is_pure(),
            Expr::If(c, t, f) => c.is_pure() && t.is_pure() && f.is_pure(),
            Expr::Let(_, v, b) => v.is_pure() && b.is_pure(),
            Expr::New(_, c) => c.is_pure(),
            Expr::Get(i, _, idx) => i.is_pure() && idx.is_pure(),
            Expr::Set(_, _, _, _) | Expr::Write(_, _) | Expr::Read(_) | Expr::TimeNow | Expr::TimeNano | Expr::Env(_) | Expr::Panic(_) | Expr::Trap(_, _) | Expr::HttpClient(_, _, _) | Expr::HttpServer(_, _) | Expr::HttpHeader(_, _) | Expr::FileOpen(_, _) => false,
            Expr::TimeGet(t, i) => t.is_pure() && i.is_pure(),
            Expr::TimeSet(y, m, d, h, mn, s) => y.is_pure() && m.is_pure() && d.is_pure() && h.is_pure() && mn.is_pure() && s.is_pure(),
            Expr::Pack(e) => e.is_pure(),
            Expr::Unpack(e, _) => e.is_pure(),
            Expr::Map(e, _, f) => e.is_pure() && f.is_pure(),
            Expr::Filter(e, f) => e.is_pure() && f.is_pure(),
            Expr::MoneyOp(_, l, r) => l.is_pure() && r.is_pure(),
            Expr::MoneyStr(e) => e.is_pure(),
            Expr::TimeOp(_, l, r) => l.is_pure() && r.is_pure(),
            Expr::TimeZone => true,
            Expr::Define(_, _, body, _) => body.is_pure(),
            Expr::Apply(f, args) => {
                if !f.is_pure() { return false; }
                for arg in args { if !arg.is_pure() { return false; } }
                true
            }
            Expr::Import(..) => false,
            Expr::Shape(_, _, _) => true,
        }
    }

    pub fn complexity(&self) -> usize {
        match self {
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Identifier(_) | Expr::DeBruijn(_) | Expr::Expand(_) => 1,
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) | Expr::Len(e) | Expr::Str(e) => 1 + e.complexity(),
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Seq(l, r) | Expr::BinaryOp(_, l, r) => 5 + l.complexity() + r.complexity(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => 5 + s.complexity() + b.complexity() + l.complexity(),
            Expr::If(c, t, f) => 1 + c.complexity() + t.complexity() + f.complexity(),
            Expr::Let(_, v, b) => 1 + v.complexity() + b.complexity(),
            Expr::New(_, c) => 10 + c.complexity(),
            Expr::Get(i, _, idx) => 2 + i.complexity() + idx.complexity(),
            Expr::Set(i, _, idx, v) => 2 + i.complexity() + idx.complexity() + v.complexity(),
            Expr::Define(_, _, body, _) => body.complexity(),
            Expr::Read(h) => 1 + h.complexity(),
            Expr::Write(h, s) => 1 + h.complexity() + s.complexity(),
            Expr::Apply(f, args) => {
                let mut sum = f.complexity();
                for arg in args { sum += arg.complexity(); }
                sum + 5
            }
            Expr::TimeNow | Expr::TimeNano | Expr::Env(_) => 5,
            Expr::TimeGet(t, i) => 5 + t.complexity() + i.complexity(),
            Expr::TimeSet(y, m, d, h, mn, s) => 10 + y.complexity() + m.complexity() + d.complexity() + h.complexity() + mn.complexity() + s.complexity(),
            Expr::Pack(e) => 10 + e.complexity(),
            Expr::Unpack(e, _) => 10 + e.complexity(),
            Expr::Map(e, _, f) => 20 + e.complexity() + f.complexity(),
            Expr::Filter(e, f) => 20 + e.complexity() + f.complexity(),
            Expr::MoneyOp(_, l, r) => 5 + l.complexity() + r.complexity(),
            Expr::MoneyStr(e) => 10 + e.complexity(),
            Expr::TimeOp(_, l, r) => 5 + l.complexity() + r.complexity(),
            Expr::TimeZone => 5,
            Expr::Panic(e) => 10 + e.complexity(),
            Expr::Trap(t, f) => 20 + t.complexity() + f.complexity(),
            Expr::HttpClient(method, url, body) => 10 + method.complexity() + url.complexity() + body.complexity(),
            Expr::HttpServer(op, arg) => 10 + op.complexity() + arg.complexity(),
            Expr::HttpHeader(req, name) => 10 + req.complexity() + name.complexity(),
            Expr::FileOpen(path, mode) => 10 + path.complexity() + mode.complexity(),
            Expr::Import(..) | Expr::Shape(_, _, _) => 1,
        }
    }

    pub fn get_calls(&self) -> Vec<String> {
        let mut calls = Vec::new();
        self.collect_calls(&mut calls);
        calls
    }

    fn collect_calls(&self, calls: &mut Vec<String>) {
        match self {
            Expr::Identifier(name) | Expr::Expand(name) => {
                calls.push(name.clone());
            }
            Expr::Apply(func_expr, args) => {
                func_expr.collect_calls(calls);
                for arg in args {
                    arg.collect_calls(calls);
                }
            }
            Expr::Move(expr) | Expr::Borrow(expr) | Expr::MutBorrow(expr) | Expr::Len(expr) | Expr::Str(expr) | Expr::Read(expr) | Expr::Env(expr) | Expr::Pack(expr) | Expr::MoneyStr(expr) | Expr::Panic(expr) => {
                expr.collect_calls(calls);
            }
            Expr::Unpack(expr, shape) => {
                calls.push(shape.clone());
                expr.collect_calls(calls);
            }
            Expr::New(shape, count) => {
                calls.push(shape.clone());
                count.collect_calls(calls);
            }
            Expr::Trap(t, f) => { t.collect_calls(calls); f.collect_calls(calls); }
            Expr::Get(inst, _, idx) => {
                inst.collect_calls(calls);
                idx.collect_calls(calls);
            }
            Expr::Set(inst, _, idx, val) => {
                inst.collect_calls(calls);
                idx.collect_calls(calls);
                val.collect_calls(calls);
            }
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => { s.collect_calls(calls); b.collect_calls(calls); l.collect_calls(calls); }
            Expr::HttpClient(method, url, body) => { method.collect_calls(calls); url.collect_calls(calls); body.collect_calls(calls); }
            Expr::HttpServer(op, arg) => { op.collect_calls(calls); arg.collect_calls(calls); }
            Expr::HttpHeader(req, name) => { req.collect_calls(calls); name.collect_calls(calls); }
            Expr::FileOpen(path, mode) => { path.collect_calls(calls); mode.collect_calls(calls); }
            Expr::BinaryOp(_, l, r) | Expr::Seq(l, r) | Expr::TimeOp(_, l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Write(l, r) | Expr::MoneyOp(_, l, r) => {
                l.collect_calls(calls); r.collect_calls(calls);
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
            Expr::Define(_, _, body, _) => {
                body.collect_calls(calls);
            }
            Expr::Map(e, _, f) => { e.collect_calls(calls); f.collect_calls(calls); }
            Expr::Filter(e, f) => { e.collect_calls(calls); f.collect_calls(calls); }
            Expr::TimeGet(t, i) => { t.collect_calls(calls); i.collect_calls(calls); }
            Expr::TimeSet(y, m, d, h, mn, s) => { 
                y.collect_calls(calls); m.collect_calls(calls); d.collect_calls(calls); 
                h.collect_calls(calls); mn.collect_calls(calls); s.collect_calls(calls); 
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
            Expr::String(_) => s.push_str("s"),
            Expr::Identifier(_) => s.push_str("v"),
            Expr::DeBruijn(i) => s.push_str(&format!("^{}", i)),
            Expr::Expand(_) => s.push_str("!"),
            Expr::BinaryOp(op, l, r) => {
                s.push_str(&format!("{:?}", op));
                l.collect_fingerprint(s);
                r.collect_fingerprint(s);
            }
            Expr::Move(e) => { s.push_str(">"); e.collect_fingerprint(s); }
            Expr::Borrow(e) => { s.push_str("$"); e.collect_fingerprint(s); }
            Expr::MutBorrow(e) => { s.push_str("~"); e.collect_fingerprint(s); }
            Expr::If(c, t, f) => {
                s.push_str("?");
                c.collect_fingerprint(s);
                t.collect_fingerprint(s);
                f.collect_fingerprint(s);
            }
            Expr::Apply(_, args) => {
                s.push_str(&format!("@{}", args.len()));
                for arg in args { arg.collect_fingerprint(s); }
            }
            Expr::Let(_, val, body) => {
                s.push_str("L");
                val.collect_fingerprint(s);
                body.collect_fingerprint(s);
            }
            Expr::Import(..) => s.push_str("I"),
            Expr::Seq(e1, e2) => { s.push_str("."); e1.collect_fingerprint(s); e2.collect_fingerprint(s); }
            Expr::Pack(e) => { s.push_str("jp"); e.collect_fingerprint(s); }
            Expr::Unpack(e, _) => { s.push_str("ju"); e.collect_fingerprint(s); }
            Expr::Map(e, _, f) => { s.push_str("map"); e.collect_fingerprint(s); f.collect_fingerprint(s); }
            Expr::Filter(e, f) => { s.push_str("flt"); e.collect_fingerprint(s); f.collect_fingerprint(s); }
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
            Expr::Define(_, params, body, _) => {
                s.push_str(":");
                for _ in params { s.push_str("p"); }
                body.collect_fingerprint(s);
            }
            Expr::HttpClient(method, url, body) => {
                s.push_str("http");
                method.collect_fingerprint(s);
                url.collect_fingerprint(s);
                body.collect_fingerprint(s);
            }
            Expr::HttpServer(op, arg) => {
                s.push_str("srv");
                op.collect_fingerprint(s);
                arg.collect_fingerprint(s);
            }
            Expr::HttpHeader(req, name) => {
                s.push_str("hdr");
                req.collect_fingerprint(s);
                name.collect_fingerprint(s);
            }
            _ => s.push_str("?"),
        }
    }
}

use std::collections::HashSet;

pub fn prune_dead_code(expressions: Vec<Expr>) -> Vec<Expr> {
    let mut reachable = HashSet::new();
    let mut to_visit = Vec::new();

    // 1. Identify roots (main and exported symbols)
    for expr in &expressions {
        match expr {
            Expr::Define(name, _, _, exported) => {
                if name == "main" || *exported {
                    reachable.insert(name.clone());
                    to_visit.push(expr);
                }
            }
            Expr::Shape(name, _, exported) => {
                if *exported {
                    reachable.insert(name.clone());
                }
            }
            _ => {}
        }
    }

    // 2. Transitive closure of call graph
    let mut visited = HashSet::new();
    while let Some(expr) = to_visit.pop() {
        let calls = expr.get_calls();
        for call in calls {
            if !reachable.contains(&call) {
                reachable.insert(call.clone());
            }
        }
        
        // Mark as visited so we don't process same definition twice
        if let Expr::Define(name, _, _, _) = expr {
            visited.insert(name.clone());
        }

        // Find any newly reachable definitions to visit
        for def in &expressions {
            if let Expr::Define(name, _, _, _) = def {
                if reachable.contains(name) && !visited.contains(name) {
                    to_visit.push(def);
                }
            }
        }
    }

    // 3. Filter expressions
    expressions.into_iter().filter(|expr| {
        match expr {
            Expr::Define(name, _, _, _) => reachable.contains(name),
            Expr::Import(_, symbol, _) => reachable.contains(symbol),
            Expr::Shape(name, _, exported) => reachable.contains(name) || *exported,
            _ => true, 
        }
    }).collect()
}
