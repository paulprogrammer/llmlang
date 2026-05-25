use std::fmt;
use crate::compiler::ast::{Expr, Param};
use crate::compiler::lexer::Token;

pub struct PrettyExpr<'a> {
    pub expr: &'a Expr,
    pub depth: usize,
}

impl<'a> PrettyExpr<'a> {
    pub fn new(expr: &'a Expr, depth: usize) -> Self {
        Self { expr, depth }
    }

    fn indent(&self) -> String {
        "    ".repeat(self.depth)
    }
}

fn format_token(token: &Token) -> &str {
    match token {
        Token::Add => "+",
        Token::Sub => "-",
        Token::Mul => "*",
        Token::Div => "/",
        Token::Eq => "==",
        Token::Lt => "<",
        Token::Gt => ">",
        _ => "???",
    }
}

impl<'a> fmt::Display for PrettyExpr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ind = self.indent();
        match self.expr {
            Expr::Integer(i) => write!(f, "{}", i),
            Expr::Float(val) => write!(f, "{}", val),
            Expr::DeBruijn(idx) => write!(f, "D{}", idx), // Just fallback, usually won't print this directly
            Expr::Identifier(s) => write!(f, "{}", s),
            Expr::BinaryOp(tok, left, right) => {
                write!(f, "{} {} {}", format_token(tok), PrettyExpr::new(left, self.depth), PrettyExpr::new(right, self.depth))
            }
            Expr::Apply(func, args) => {
                write!(f, "@{} {}", args.len(), PrettyExpr::new(func, self.depth))?;
                for arg in args {
                    write!(f, " {}", PrettyExpr::new(arg, self.depth))?;
                }
                Ok(())
            }
            Expr::Move(expr) => write!(f, "> {}", PrettyExpr::new(expr, self.depth)),
            Expr::Borrow(expr) => write!(f, "$ {}", PrettyExpr::new(expr, self.depth)),
            Expr::MutBorrow(expr) => write!(f, "~ {}", PrettyExpr::new(expr, self.depth)),
            Expr::Define(name, params, body, exported) => {
                let exp = if *exported { "*" } else { "" };
                write!(f, "{}: {}", exp, name)?;
                for p in params {
                    if p.expand {
                        write!(f, " !{}", p.name)?;
                    } else {
                        write!(f, " {}", p.name)?;
                    }
                }
                write!(f, "\n    {}{}", ind, PrettyExpr::new(body, self.depth + 1))
            }
            Expr::Shape(name, fields, exported) => {
                let exp = if *exported { "*" } else { "" };
                write!(f, "{}# {}", exp, name)?;
                for field in fields {
                    write!(f, " {}", field)?;
                }
                Ok(())
            }
            Expr::New(shape, count) => write!(f, "N {} {}", shape, PrettyExpr::new(count, self.depth)),
            Expr::Get(inst, field, idx) => write!(f, "G {} {} {}", PrettyExpr::new(inst, self.depth), field, PrettyExpr::new(idx, self.depth)),
            Expr::Set(inst, field, idx, val) => write!(f, "S {} {} {} {}", PrettyExpr::new(inst, self.depth), field, PrettyExpr::new(idx, self.depth), PrettyExpr::new(val, self.depth)),
            Expr::If(cond, t_branch, f_branch) => {
                write!(f, "? {}\n    {}{}\n    {}{}", 
                    PrettyExpr::new(cond, self.depth),
                    ind, PrettyExpr::new(t_branch, self.depth + 1),
                    ind, PrettyExpr::new(f_branch, self.depth + 1))
            }
            Expr::Expand(name) => write!(f, "!{}", name),
            Expr::Let(name, val, body) => {
                write!(f, "L {} {}\n{}{}", name, PrettyExpr::new(val, self.depth), ind, PrettyExpr::new(body, self.depth))
            }
            Expr::Import(alias, sym, arity) => write!(f, "I {} {} {}", alias, sym, arity),
            Expr::String(s) => write!(f, "\"{}\"", s.replace("\"", "\\\"")),
            Expr::Len(expr) => write!(f, "sl {}", PrettyExpr::new(expr, self.depth)),
            Expr::Cat(l, r) => write!(f, "sc {} {}", PrettyExpr::new(l, self.depth), PrettyExpr::new(r, self.depth)),
            Expr::Sub(s, st, len) => write!(f, "ss {} {} {}", PrettyExpr::new(s, self.depth), PrettyExpr::new(st, self.depth), PrettyExpr::new(len, self.depth)),
            Expr::Loc(s, pat) => write!(f, "sf {} {}", PrettyExpr::new(s, self.depth), PrettyExpr::new(pat, self.depth)),
            Expr::Reg(s, pat) => write!(f, "sr {} {}", PrettyExpr::new(s, self.depth), PrettyExpr::new(pat, self.depth)),
            Expr::Read(h) => write!(f, "( {}", PrettyExpr::new(h, self.depth)),
            Expr::Write(h, s) => write!(f, ") {} {}", PrettyExpr::new(h, self.depth), PrettyExpr::new(s, self.depth)),
            Expr::Str(expr) => write!(f, "str {}", PrettyExpr::new(expr, self.depth)),
            Expr::Split(s, d, i) => write!(f, "sp {} {} {}", PrettyExpr::new(s, self.depth), PrettyExpr::new(d, self.depth), PrettyExpr::new(i, self.depth)),
            Expr::TimeNow => write!(f, "tn"),
            Expr::TimeNano => write!(f, "tns"),
            Expr::TimeGet(t, i) => write!(f, "tg {} {}", PrettyExpr::new(t, self.depth), PrettyExpr::new(i, self.depth)),
            Expr::TimeSet(y, m, d, h, mn, s) => write!(f, "ts {} {} {} {} {} {}", 
                PrettyExpr::new(y, self.depth), PrettyExpr::new(m, self.depth), PrettyExpr::new(d, self.depth), 
                PrettyExpr::new(h, self.depth), PrettyExpr::new(mn, self.depth), PrettyExpr::new(s, self.depth)),
            Expr::Env(k) => write!(f, "env {}", PrettyExpr::new(k, self.depth)),
            Expr::Seq(l, r) => write!(f, ". {}\n{}{}", PrettyExpr::new(l, self.depth), ind, PrettyExpr::new(r, self.depth)),
            Expr::Pack(expr) => write!(f, "jp {}", PrettyExpr::new(expr, self.depth)),
            Expr::Metadata(tag, val, t) => write!(f, "M {} {} {}", PrettyExpr::new(tag, self.depth), PrettyExpr::new(val, self.depth), PrettyExpr::new(t, self.depth)),
            Expr::OtelEmit(t, a1, a2, a3) => write!(f, "oe {} {} {} {}", PrettyExpr::new(t, self.depth), PrettyExpr::new(a1, self.depth), PrettyExpr::new(a2, self.depth), PrettyExpr::new(a3, self.depth)),
            Expr::Unpack(expr, shape) => write!(f, "ju \"{}\" {}", shape, PrettyExpr::new(expr, self.depth)),
            Expr::Map(inst, field, func) => write!(f, "map {} \"{}\" {}", PrettyExpr::new(inst, self.depth), field, PrettyExpr::new(func, self.depth)),
            Expr::Filter(inst, func) => write!(f, "flt {} {}", PrettyExpr::new(inst, self.depth), PrettyExpr::new(func, self.depth)),
            Expr::MoneyOp(tok, a, b) => write!(f, "% {} {} {}", format_token(tok), PrettyExpr::new(a, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::MoneyStr(expr) => write!(f, "% str {}", PrettyExpr::new(expr, self.depth)),
            Expr::TimeOp(tok, a, b) => write!(f, "tn {} {} {}", format_token(tok), PrettyExpr::new(a, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::TimeZone => write!(f, "tz"),
            Expr::Panic(expr) => write!(f, "` {}", PrettyExpr::new(expr, self.depth)),
            Expr::Trap(try_b, fall_b) => write!(f, "^ {} {}", PrettyExpr::new(try_b, self.depth), PrettyExpr::new(fall_b, self.depth)),
            Expr::HttpClient(m, u, b) => write!(f, "http {} {} {}", PrettyExpr::new(m, self.depth), PrettyExpr::new(u, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::HttpServer(op, arg) => write!(f, "srv {} {}", PrettyExpr::new(op, self.depth), PrettyExpr::new(arg, self.depth)),
            Expr::HttpHeader(req, name) => write!(f, "hdr {} {}", PrettyExpr::new(req, self.depth), PrettyExpr::new(name, self.depth)),
            Expr::FileOpen(p, m) => write!(f, "fo {} {}", PrettyExpr::new(p, self.depth), PrettyExpr::new(m, self.depth)),
        }
    }
}
