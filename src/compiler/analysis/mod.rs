use crate::compiler::ast::{Expr};

impl Expr {
    pub fn returns_ptr(&self) -> bool {
        match self {
            Expr::String(_) | Expr::New(_, _) | Expr::Unpack(_, _) | Expr::Map(_, _, _) | Expr::Filter(_, _) => true,
            Expr::Cat(_, _) | Expr::Sub(_, _, _) | Expr::Read(_) | Expr::Str(_) | Expr::Split(_, _, _) | Expr::Pack(_) | Expr::Env(_) | Expr::MoneyStr(_) | Expr::TimeZone => true,
            Expr::Let(_, _, body) | Expr::Seq(_, body) => body.returns_ptr(),
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) => e.returns_ptr(),
            Expr::Trap(t, f) => t.returns_ptr() || f.returns_ptr(),
            _ => false,
        }
    }

    pub fn is_pure(&self) -> bool {
        match self {
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::DeBruijn(_) | Expr::Identifier(_) | Expr::Expand(_) => true,
            Expr::BinaryOp(_, l, r) => l.is_pure() && r.is_pure(),
            Expr::Apply(_, args) => args.iter().all(|a| a.is_pure()),
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) | Expr::Len(e) | Expr::Str(e) => e.is_pure(),
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Seq(l, r) => l.is_pure() && r.is_pure(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => s.is_pure() && b.is_pure() && l.is_pure(),
            Expr::If(c, t, f) => c.is_pure() && t.is_pure() && f.is_pure(),
            Expr::Let(_, v, b) => v.is_pure() && b.is_pure(),
            Expr::New(_, c) => c.is_pure(),
            Expr::Get(i, _, idx) => i.is_pure() && idx.is_pure(),
            Expr::Set(_, _, _, _) | Expr::Write(_, _) | Expr::Read(_) | Expr::TimeNow | Expr::TimeNano | Expr::Env(_) | Expr::Panic(_) | Expr::Trap(_, _) => false,
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
            Expr::Import(_, _) => false,
            Expr::Shape(_, _, _) => true,
        }
    }

    pub fn complexity(&self) -> usize {
        match self {
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::DeBruijn(_) | Expr::Identifier(_) | Expr::Expand(_) => 1,
            Expr::BinaryOp(_, l, r) => 1 + l.complexity() + r.complexity(),
            Expr::Apply(_, args) => 10 + args.iter().map(|a| a.complexity()).sum::<usize>(),
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) | Expr::Len(e) | Expr::Str(e) => 1 + e.complexity(),
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Seq(l, r) => 5 + l.complexity() + r.complexity(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => 5 + s.complexity() + b.complexity() + l.complexity(),
            Expr::If(c, t, f) => 1 + c.complexity() + t.complexity() + f.complexity(),
            Expr::Let(_, v, b) => 1 + v.complexity() + b.complexity(),
            Expr::New(_, c) => 10 + c.complexity(),
            Expr::Get(i, _, idx) => 2 + i.complexity() + idx.complexity(),
            Expr::Set(i, _, idx, v) => 2 + i.complexity() + idx.complexity() + v.complexity(),
            Expr::Define(_, _, body, _) => body.complexity(),
            Expr::Read(h) => 1 + h.complexity(),
            Expr::Write(h, s) => 1 + h.complexity() + s.complexity(),
            Expr::TimeNow | Expr::TimeNano => 5,
            Expr::TimeGet(t, i) => 10 + t.complexity() + i.complexity(),
            Expr::TimeSet(y, m, d, h, mn, s) => 10 + y.complexity() + m.complexity() + d.complexity() + h.complexity() + mn.complexity() + s.complexity(),
            Expr::Env(k) => 5 + k.complexity(),
            Expr::Pack(e) => 10 + e.complexity(),
            Expr::Unpack(e, _) => 20 + e.complexity(),
            Expr::Map(e, _, f) => 50 + e.complexity() + f.complexity(),
            Expr::Filter(e, f) => 50 + e.complexity() + f.complexity(),
            Expr::MoneyOp(_, l, r) => 5 + l.complexity() + r.complexity(),
            Expr::MoneyStr(e) => 10 + e.complexity(),
            Expr::TimeOp(_, l, r) => 5 + l.complexity() + r.complexity(),
            Expr::TimeZone => 5,
            Expr::Panic(e) => 10 + e.complexity(),
            Expr::Trap(t, f) => 20 + t.complexity() + f.complexity(),
            Expr::Import(_, _) | Expr::Shape(_, _, _) => 1,
        }
    }

    pub fn get_calls(&self) -> Vec<String> {
        let mut calls = Vec::new();
        self.collect_calls(&mut calls);
        calls
    }

    fn collect_calls(&self, calls: &mut Vec<String>) {
        match self {
            Expr::BinaryOp(_, left, right) | Expr::Cat(left, right) | Expr::Loc(left, right) | Expr::Reg(left, right) | Expr::Seq(left, right) | Expr::MoneyOp(_, left, right) | Expr::TimeOp(_, left, right) => {
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
            Expr::Move(expr) | Expr::Borrow(expr) | Expr::MutBorrow(expr) | Expr::Len(expr) | Expr::Str(expr) | Expr::Read(expr) | Expr::Env(expr) | Expr::Pack(expr) | Expr::Unpack(expr, _) | Expr::MoneyStr(expr) | Expr::Panic(expr) => {
                expr.collect_calls(calls);
            }
            Expr::Trap(t, f) => { t.collect_calls(calls); f.collect_calls(calls); }
            Expr::Map(e, _, f) => { 
                if let Expr::Identifier(name) = &**f { calls.push(name.clone()); }
                e.collect_calls(calls); f.collect_calls(calls); 
            }
            Expr::Filter(e, f) => { 
                if let Expr::Identifier(name) = &**f { calls.push(name.clone()); }
                e.collect_calls(calls); f.collect_calls(calls); 
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
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => { s.collect_calls(calls); b.collect_calls(calls); l.collect_calls(calls); }
            Expr::TimeNow | Expr::TimeNano | Expr::TimeZone => {}
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
            Expr::DeBruijn(n) => s.push_str(&format!("^{}", n)),
            Expr::Identifier(_) => s.push_str("n"),
            Expr::BinaryOp(tok, left, right) => {
                s.push_str(&format!("{:?}", tok));
                left.collect_fingerprint(s);
                right.collect_fingerprint(s);
            }
            Expr::Apply(func_expr, args) => {
                s.push_str(&format!("@{}", args.len()));
                func_expr.collect_fingerprint(s);
                for arg in args {
                    arg.collect_fingerprint(s);
                }
            }
            Expr::Move(e) => { s.push_str("⮞"); e.collect_fingerprint(s); }
            Expr::Borrow(e) => { s.push_str("⚓"); e.collect_fingerprint(s); }
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
            Expr::Len(e) => { s.push_str("ℓ"); e.collect_fingerprint(s); }
            Expr::Cat(l, r) => { s.push_str("⧉"); l.collect_fingerprint(s); r.collect_fingerprint(s); }
            Expr::Sub(str, b, l) => { s.push_str("✂"); str.collect_fingerprint(s); b.collect_fingerprint(s); l.collect_fingerprint(s); }
            Expr::Loc(str, p) => { s.push_str("🔍"); str.collect_fingerprint(s); p.collect_fingerprint(s); }
            Expr::Reg(str, r) => { s.push_str("≈"); str.collect_fingerprint(s); r.collect_fingerprint(s); }
            Expr::Read(h) => { s.push_str("📥"); h.collect_fingerprint(s); }
            Expr::Write(h, str) => { s.push_str("📤"); h.collect_fingerprint(s); str.collect_fingerprint(s); }
            Expr::Str(e) => { s.push_str("🧵"); e.collect_fingerprint(s); }
            Expr::Split(str, d, idx) => { s.push_str("🪓"); str.collect_fingerprint(s); d.collect_fingerprint(s); idx.collect_fingerprint(s); }
            Expr::TimeNow => { s.push_str("🕒"); }
            Expr::TimeNano => { s.push_str("🕒⌛"); }
            Expr::TimeZone => { s.push_str("🕒🌍"); }
            Expr::TimeGet(t, i) => { s.push_str("📅"); t.collect_fingerprint(s); i.collect_fingerprint(s); }
            Expr::TimeSet(y, m, d, h, mn, sc) => { 
                s.push_str("📆"); 
                y.collect_fingerprint(s); m.collect_fingerprint(s); d.collect_fingerprint(s); 
                h.collect_fingerprint(s); mn.collect_fingerprint(s); sc.collect_fingerprint(s); 
            }
            Expr::Env(k) => { s.push_str("🌍"); k.collect_fingerprint(s); }
            Expr::Expand(_) => s.push_str("!"),
            Expr::Let(_, val, body) => {
                s.push_str("L");
                val.collect_fingerprint(s);
                body.collect_fingerprint(s);
            }
            Expr::Import(_, _) => s.push_str("I"),
            Expr::Seq(e1, e2) => { s.push_str("."); e1.collect_fingerprint(s); e2.collect_fingerprint(s); }
            Expr::Pack(e) => { s.push_str("📦"); e.collect_fingerprint(s); }
            Expr::Unpack(e, shape) => { s.push_str(&format!("🎁{}", shape)); e.collect_fingerprint(s); }
            Expr::Map(e, field, f) => { s.push_str(&format!("⟴{}", field)); e.collect_fingerprint(s); f.collect_fingerprint(s); }
            Expr::Filter(e, f) => { s.push_str("▽"); e.collect_fingerprint(s); f.collect_fingerprint(s); }
            Expr::MoneyOp(tok, l, r) => { s.push_str(&format!("💰{:?}", tok)); l.collect_fingerprint(s); r.collect_fingerprint(s); }
            Expr::MoneyStr(e) => { s.push_str("💰🧵"); e.collect_fingerprint(s); }
            Expr::TimeOp(tok, l, r) => { s.push_str(&format!("🕒{:?}", tok)); l.collect_fingerprint(s); r.collect_fingerprint(s); }
            Expr::Panic(e) => { s.push_str("🚨"); e.collect_fingerprint(s); }
            Expr::Trap(t, f) => { s.push_str("🛡️"); t.collect_fingerprint(s); f.collect_fingerprint(s); }
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
            Expr::Shape(_, _, exported) => {
                if *exported {
                    // Shapes don't have calls but might be exported for signature.
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
            Expr::Import(_, symbol) => reachable.contains(symbol),
            _ => true, // Keep Shapes, etc.
        }
    }).collect()
}
