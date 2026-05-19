use crate::compiler::lexer::Token;

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
    Move(Box<Expr>),                       // ⮞ expr
    Borrow(Box<Expr>),                     // ⚓ expr
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
    String(String),
    Len(Box<Expr>),                         // ℓ expr
    Cat(Box<Expr>, Box<Expr>),              // ⧉ left right
    Sub(Box<Expr>, Box<Expr>, Box<Expr>),   // ✂ string start length
    Loc(Box<Expr>, Box<Expr>),              // 🔍 string pattern
    Reg(Box<Expr>, Box<Expr>),              // ≈ string regex
    Read(Box<Expr>),                        // 📥 handle
    Write(Box<Expr>, Box<Expr>),            // 📤 handle string
    Str(Box<Expr>),                         // 🧵 int
    Split(Box<Expr>, Box<Expr>, Box<Expr>), // 🪓 string delim index
    TimeNow,                                // 🕒
    TimeGet(Box<Expr>, Box<Expr>),          // 📅 T index
    TimeSet(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>), // 📆 Y M D H m S
    Env(Box<Expr>),                         // 🌍 key
    Seq(Box<Expr>, Box<Expr>),              // . expr1 expr2
}

impl Expr {
    pub fn is_pure(&self) -> bool {
        match self {
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::DeBruijn(_) | Expr::Identifier(_) | Expr::Expand(_) => true,
            Expr::BinaryOp(_, l, r) => l.is_pure() && r.is_pure(),
            Expr::Apply(_, args) => args.iter().all(|a| a.is_pure()), // Conservatively assume func body is pure if args are
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) | Expr::Len(e) | Expr::Str(e) => e.is_pure(),
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) => l.is_pure() && r.is_pure(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => s.is_pure() && b.is_pure() && l.is_pure(),
            Expr::If(c, t, f) => c.is_pure() && t.is_pure() && f.is_pure(),
            Expr::Let(_, v, b) => v.is_pure() && b.is_pure(),
            Expr::New(_, c) => c.is_pure(),
            Expr::Get(i, _, idx) => i.is_pure() && idx.is_pure(),
            Expr::Set(_, _, _, _) | Expr::Write(_, _) | Expr::Read(_) | Expr::TimeNow => false,
            Expr::TimeGet(t, i) => t.is_pure() && i.is_pure(),
            Expr::TimeSet(y, m, d, h, mn, s) => y.is_pure() && m.is_pure() && d.is_pure() && h.is_pure() && mn.is_pure() && s.is_pure(),
            Expr::Env(_) => false,
            Expr::Seq(e1, e2) => e1.is_pure() && e2.is_pure(),
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
            Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) => 5 + l.complexity() + r.complexity(),
            Expr::Sub(s, b, l) | Expr::Split(s, b, l) => 5 + s.complexity() + b.complexity() + l.complexity(),
            Expr::If(c, t, f) => 1 + c.complexity() + t.complexity() + f.complexity(),
            Expr::Let(_, v, b) => 1 + v.complexity() + b.complexity(),
            Expr::New(_, c) => 10 + c.complexity(),
            Expr::Get(i, _, idx) => 2 + i.complexity() + idx.complexity(),
            Expr::Set(i, _, idx, v) => 2 + i.complexity() + idx.complexity() + v.complexity(),
            Expr::Define(_, _, body, _) => body.complexity(),
            Expr::Read(h) => 1 + h.complexity(),
            Expr::Write(h, s) => 1 + h.complexity() + s.complexity(),
            Expr::TimeNow => 5,
            Expr::TimeGet(t, i) => 10 + t.complexity() + i.complexity(),
            Expr::TimeSet(y, m, d, h, mn, s) => 10 + y.complexity() + m.complexity() + d.complexity() + h.complexity() + mn.complexity() + s.complexity(),
            Expr::Env(k) => 5 + k.complexity(),
            Expr::Seq(e1, e2) => 1 + e1.complexity() + e2.complexity(),
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
            Expr::Len(e) => e.collect_calls(calls),
            Expr::Cat(l, r) => { l.collect_calls(calls); r.collect_calls(calls); }
            Expr::Sub(s, b, l) => { s.collect_calls(calls); b.collect_calls(calls); l.collect_calls(calls); }
            Expr::Loc(s, p) => { s.collect_calls(calls); p.collect_calls(calls); }
            Expr::Reg(s, r) => { s.collect_calls(calls); r.collect_calls(calls); }
            Expr::Read(h) => h.collect_calls(calls),
            Expr::Write(h, s) => { h.collect_calls(calls); s.collect_calls(calls); }
            Expr::Str(e) => e.collect_calls(calls),
            Expr::Split(s, d, i) => { s.collect_calls(calls); d.collect_calls(calls); i.collect_calls(calls); }
            Expr::TimeNow => {}
            Expr::TimeGet(t, i) => { t.collect_calls(calls); i.collect_calls(calls); }
            Expr::TimeSet(y, m, d, h, mn, s) => { 
                y.collect_calls(calls); m.collect_calls(calls); d.collect_calls(calls); 
                h.collect_calls(calls); mn.collect_calls(calls); s.collect_calls(calls); 
            }
            Expr::Env(k) => k.collect_calls(calls),
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
        }
    }
}
