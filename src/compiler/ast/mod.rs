pub mod display;
use crate::compiler::lexer::Token;

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub name: String,
    pub expand: bool, // true if marked with !
}

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    DeBruijn(usize),
    Identifier(String),
    BinaryOp(Token, Box<Expr>, Box<Expr>), // +, -, *, /
    Apply(Box<Expr>, Vec<Expr>),           // @ func arg1 arg2 ...
    Move(Box<Expr>),                       // > expr
    Borrow(Box<Expr>),                     // $ expr
    MutBorrow(Box<Expr>),                  // ~ expr
    Define(String, Vec<Param>, Box<Expr>, bool), // : name (args) body, exported?
    Shape(String, Vec<String>, bool),             // # name field1 field2 ..., exported?
    New(String, Box<Expr>),                 // N shape_name count
    Get(Box<Expr>, String, Box<Expr>),      // G instance field index
    Set(Box<Expr>, String, Box<Expr>, Box<Expr>), // S instance field index value
    If(Box<Expr>, Box<Expr>, Box<Expr>),    // ? cond true_branch false_branch
    Expand(String),                         // ` name (reference to expand param)
    Let(String, Box<Expr>, Box<Expr>),      // L name val body
    Import(String, String, usize),          // I module_alias symbol_name arity
    String(String),
    Len(Box<Expr>),                         // sl expr
    Cat(Box<Expr>, Box<Expr>),              // sc left right
    Sub(Box<Expr>, Box<Expr>, Box<Expr>),   // ss string start length
    Loc(Box<Expr>, Box<Expr>),              // sf string pattern
    Reg(Box<Expr>, Box<Expr>),              // sr string regex
    Read(Box<Expr>),                        // ( handle
    Write(Box<Expr>, Box<Expr>),            // ) handle string
    Str(Box<Expr>),                         // str int
    Split(Box<Expr>, Box<Expr>, Box<Expr>), // sp string delim index
    TimeNow,                                // tn
    TimeNano,                               // tns
    TimeGet(Box<Expr>, Box<Expr>),          // tg T index
    TimeSet(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>), // ts Y M D H m S
    Env(Box<Expr>),                         // env key
    Seq(Box<Expr>, Box<Expr>),              // . expr1 expr2
    Pack(Box<Expr>),                        // jp expr (Serialize / JSON pack)
    Unpack(Box<Expr>, String),              // ju expr "Shape" (Deserialize / JSON unpack)
    Metadata(Box<Expr>, Box<Expr>, Box<Expr>), // M tag value target
    OtelEmit(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>), // oe type arg1 arg2 arg3
    Map(Box<Expr>, String, Box<Expr>),      // map inst "field" func
    Filter(Box<Expr>, Box<Expr>),           // flt inst func
    MoneyOp(Token, Box<Expr>, Box<Expr>),   // % op a b
    MoneyStr(Box<Expr>),                    // % str expr
    TimeOp(Token, Box<Expr>, Box<Expr>),    // tn op T seconds
    TimeZone,                               // tz
    Panic(Box<Expr>),                       // ! message
    Trap(Box<Expr>, Box<Expr>),             // ^ try fallback
    HttpClient(Box<Expr>, Box<Expr>, Box<Expr>), // http method url body
    HttpServer(Box<Expr>, Box<Expr>),       // srv op_code arg
    HttpHeader(Box<Expr>, Box<Expr>),       // hdr req header_name
    FileOpen(Box<Expr>, Box<Expr>),         // fo path mode
}

/// Shared shape-inference logic used by both codegen (`codegen/shape.rs`) and
/// semantic verification (`analysis/verify.rs`). `stack_shapes` is indexed
/// like the runtime stack: index 0 is the bottom, so a De Bruijn index of 0
/// refers to the last element.
pub fn infer_shape_from_stack(expr: &Expr, stack_shapes: &[Option<String>]) -> Option<String> {
    match expr {
        Expr::New(name, _) => Some(name.clone()),
        Expr::Unpack(_, name) => Some(name.clone()),
        Expr::Map(e, _, _) => infer_shape_from_stack(e, stack_shapes),
        Expr::Filter(e, _) => infer_shape_from_stack(e, stack_shapes),
        Expr::DeBruijn(idx) => {
            if *idx < stack_shapes.len() {
                stack_shapes[stack_shapes.len() - 1 - idx].clone()
            } else {
                None
            }
        }
        Expr::Move(inner) | Expr::Borrow(inner) | Expr::MutBorrow(inner) => infer_shape_from_stack(inner, stack_shapes),
        _ => None,
    }
}
