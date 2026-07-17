use std::fmt;
use crate::compiler::ast::Expr;
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
        Token::Eq => "=",
        Token::Lt => "<",
        Token::Gt => "gt",
        Token::BitAnd => "&",
        Token::BitOr => "|",
        Token::BitXor => "xor",
        _ => "???",
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

impl<'a> fmt::Display for PrettyExpr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ind = self.indent();
        match self.expr {
            Expr::Integer(i) => write!(f, "{}", i),
            Expr::Float(val) => {
                // f64 Display drops the ".0" on whole floats, which would re-lex as an Integer.
                let s = format!("{}", val);
                if s.contains('.') { write!(f, "{}", s) } else { write!(f, "{}.0", s) }
            }
            Expr::DeBruijn(idx) => write!(f, "^{}", idx),
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
                let exp = if *exported { "X " } else { "" };
                write!(f, "{}: {}", exp, name)?;
                for p in params {
                    if p.expand {
                        write!(f, " {}`", p.name)?;
                    } else {
                        write!(f, " {}", p.name)?;
                    }
                }
                write!(f, "\n    {}{}", ind, PrettyExpr::new(body, self.depth + 1))
            }
            Expr::Shape(name, fields, exported) => {
                let exp = if *exported { "X " } else { "" };
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
            Expr::Expand(name) => write!(f, "`{}", name),
            Expr::Let(name, val, body) => {
                write!(f, "L {} {}\n{}{}", name, PrettyExpr::new(val, self.depth), ind, PrettyExpr::new(body, self.depth))
            }
            // The parser reads `I <module> <symbol>` and looks the arity up in the
            // module signature; emitting it would re-parse as a stray integer.
            Expr::Import(module, sym, _arity) => write!(f, "I {} {}", module, sym),
            Expr::String(s) => write!(f, "\"{}\"", escape_string(s)),
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
            Expr::Unpack(expr, shape) => write!(f, "ju {} \"{}\"", PrettyExpr::new(expr, self.depth), shape),
            Expr::Map(inst, field, func) => write!(f, "map {} \"{}\" {}", PrettyExpr::new(inst, self.depth), field, PrettyExpr::new(func, self.depth)),
            Expr::Filter(inst, func) => write!(f, "flt {} {}", PrettyExpr::new(inst, self.depth), PrettyExpr::new(func, self.depth)),
            Expr::MoneyOp(tok, a, b) => write!(f, "% {} {} {}", format_token(tok), PrettyExpr::new(a, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::MoneyStr(expr) => write!(f, "% str {}", PrettyExpr::new(expr, self.depth)),
            Expr::TimeOp(tok, a, b) => write!(f, "tn {} {} {}", format_token(tok), PrettyExpr::new(a, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::TimeZone => write!(f, "tz"),
            Expr::Panic(expr) => write!(f, "! {}", PrettyExpr::new(expr, self.depth)),
            Expr::Trap(try_b, fall_b) => write!(f, "^ {} {}", PrettyExpr::new(try_b, self.depth), PrettyExpr::new(fall_b, self.depth)),
            Expr::HttpClient(m, u, b) => write!(f, "http {} {} {}", PrettyExpr::new(m, self.depth), PrettyExpr::new(u, self.depth), PrettyExpr::new(b, self.depth)),
            Expr::HttpServer(op, arg) => write!(f, "srv {} {}", PrettyExpr::new(op, self.depth), PrettyExpr::new(arg, self.depth)),
            Expr::HttpHeader(req, name) => write!(f, "hdr {} {}", PrettyExpr::new(req, self.depth), PrettyExpr::new(name, self.depth)),
            Expr::FileOpen(p, m) => write!(f, "fo {} {}", PrettyExpr::new(p, self.depth), PrettyExpr::new(m, self.depth)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PrettyExpr;
    use crate::compiler::ast::Expr;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;

    fn parse(src: &str, filename: &str) -> Vec<Expr> {
        let mut parser = Parser::new(Lexer::new(src), filename.to_string()).expect("lexer init");
        parser
            .parse_module()
            .unwrap_or_else(|e| panic!("parse failed: {:?}\nsource:\n{}", e, src))
    }

    fn print_module(exprs: &[Expr]) -> String {
        exprs
            .iter()
            .map(|e| PrettyExpr::new(e, 0).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The core property: parse → print → parse must yield an identical AST.
    fn assert_roundtrip(src: &str) {
        assert_roundtrip_named(src, "roundtrip.llm");
    }

    fn assert_roundtrip_named(src: &str, filename: &str) {
        let first = parse(src, filename);
        let printed = print_module(&first);
        let second = parse(&printed, filename);
        assert_eq!(
            first, second,
            "AST changed across print/re-parse.\noriginal:\n{}\nprinted:\n{}",
            src, printed
        );
    }

    #[test]
    fn roundtrip_debruijn_in_function_body() {
        // Bound params become DeBruijn indices and must print as ^N, not DN.
        assert_roundtrip(": add2 x y\n    + $ x $ y");
    }

    #[test]
    fn roundtrip_comparison_and_bitwise_ops() {
        assert_roundtrip("= 1 2");
        assert_roundtrip("gt 2 1");
        assert_roundtrip("lt 1 2");
        assert_roundtrip("& 1 0");
        assert_roundtrip("| 1 0");
        assert_roundtrip("xor 1 0");
    }

    #[test]
    fn roundtrip_panic_and_trap() {
        assert_roundtrip("! \"boom\"");
        assert_roundtrip("^ ! \"boom\" 0");
        assert_roundtrip("^ ^ ! \"deep\" 1 2");
    }

    #[test]
    fn roundtrip_expand_param_and_reference() {
        assert_roundtrip(": variadic xs`\n    `xs");
    }

    #[test]
    fn roundtrip_exported_define_and_shape() {
        assert_roundtrip("X : pub_fn x\n    $ x");
        assert_roundtrip("X # Point x y");
        assert_roundtrip("# Private a b");
    }

    #[test]
    fn roundtrip_unpack_arg_order() {
        assert_roundtrip("ju \"{\\\"x\\\":[1]}\" \"Point\"");
    }

    #[test]
    fn roundtrip_import_omits_arity() {
        assert_roundtrip("I json parse");
        assert_roundtrip("I http get");
    }

    #[test]
    fn roundtrip_whole_floats_stay_floats() {
        assert_roundtrip("2.0");
        assert_roundtrip("3.5");
        assert_eq!(parse("2.0", "f.llm"), vec![Expr::Float(2.0)]);
        assert_eq!(
            parse(&print_module(&[Expr::Float(2.0)]), "f.llm"),
            vec![Expr::Float(2.0)]
        );
    }

    #[test]
    fn roundtrip_string_escapes() {
        assert_roundtrip("\"quote:\\\" backslash:\\\\ newline:\\n tab:\\t backtick:`\"");
    }

    #[test]
    fn roundtrip_core_constructs() {
        assert_roundtrip("L x 1\n+ $ x 1");
        assert_roundtrip("? = 1 1 10 20");
        assert_roundtrip("@2 f 1 2");
        assert_roundtrip(". ) 1 \"hi\" 0");
        assert_roundtrip("> 1");
        assert_roundtrip("~ 1");
        assert_roundtrip("M \"doc\" \"note\" : f x\n    $ x");
        assert_roundtrip("# P v\nL p N P 3\nS $ p v 0 9");
        assert_roundtrip("# P v\nL p N P 3\nG $ p v 0");
        assert_roundtrip("map x \"f\" g");
        assert_roundtrip("flt x g");
        assert_roundtrip("% + 1 2");
        assert_roundtrip("% str 5");
        assert_roundtrip("tn + tn 5");
        assert_roundtrip("sl \"abc\"");
        assert_roundtrip("sc \"a\" \"b\"");
        assert_roundtrip("ss \"abc\" 0 1");
        assert_roundtrip("sf \"abc\" \"b\"");
        assert_roundtrip("sr \"abc\" \"^a\"");
        assert_roundtrip("sp \"a,b\" \",\" 0");
        assert_roundtrip("str 42");
        assert_roundtrip("( 0");
        assert_roundtrip("env \"HOME\"");
        assert_roundtrip("jp 1");
        assert_roundtrip("tns");
        assert_roundtrip("tz");
        assert_roundtrip("tg tn 0");
        assert_roundtrip("ts 2026 7 17 0 0 0");
        assert_roundtrip("oe 1 2 3 4");
        assert_roundtrip("http \"GET\" \"http://x\" \"\"");
        assert_roundtrip("srv 0 8080");
        assert_roundtrip("hdr 1 \"Host\"");
        assert_roundtrip("fo \"f.txt\" \"r\"");
    }

    #[test]
    fn roundtrip_real_test_files() {
        for name in ["math.llm", "fault_tolerance.llm", "patch_test.llm", "business.llm"] {
            let path = format!("{}/tests/lang/{}", env!("CARGO_MANIFEST_DIR"), name);
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
            assert_roundtrip_named(&src, &path);
        }
    }
}
