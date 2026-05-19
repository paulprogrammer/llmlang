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
    TimeNano,                               // 🕒⌛
    TimeGet(Box<Expr>, Box<Expr>),          // 📅 T index
    TimeSet(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>), // 📆 Y M D H m S
    Env(Box<Expr>),                         // 🌍 key
    Seq(Box<Expr>, Box<Expr>),              // . expr1 expr2
    Pack(Box<Expr>),                        // 📦 expr (Serialize)
    Unpack(Box<Expr>, String),              // 📦2 expr "Shape" (Deserialize)
    Map(Box<Expr>, String, Box<Expr>),      // ⟴ inst "field" func
    Filter(Box<Expr>, Box<Expr>),           // ▽ inst func
    MoneyOp(Token, Box<Expr>, Box<Expr>),   // 💰+ a b
    MoneyStr(Box<Expr>),                    // 💰🧵 expr
    TimeOp(Token, Box<Expr>, Box<Expr>),    // 🕒+ T seconds
    TimeZone,                               // 🕒🌍
    Panic(Box<Expr>),                       // 🚨 message
    Trap(Box<Expr>, Box<Expr>),             // 🛡️ try fallback
}
