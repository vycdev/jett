use jett_common::Span;

// ---------------------------------------------------------------------------
// Top-level program
// ---------------------------------------------------------------------------

/// A complete source file.
#[derive(Debug, Clone)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

/// A top-level item in a module.
#[derive(Debug, Clone)]
pub enum Item {
    Namespace(NamespaceDecl),
    Function(FunctionDef),
    Struct(StructDef),
    Enum(EnumDef),
    VarDecl(VarDecl),
    Verify(VerifyBlock),
}

// ---------------------------------------------------------------------------
// Verify blocks
// ---------------------------------------------------------------------------

/// A `verify` block: `verify <name>:` followed by an indented block of
/// assert statements.  Executed at compile time by the comptime engine.
#[derive(Debug, Clone)]
pub struct VerifyBlock {
    pub name: Ident,
    pub body: Block,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NamespaceDecl {
    pub name: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub view: bool,
    pub mutable: bool,
    pub name: Ident,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<FunctionDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: Ident,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl(VarDecl),
    Assign(AssignStmt),
    Return(ReturnStmt),
    If(IfStmt),
    For(ForStmt),
    While(WhileStmt),
    Match(MatchStmt),
    Expr(ExprStmt),
    Use(UseDecl),
    Assert(AssertStmt),
    Break(Span),
    Continue(Span),
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub mutable: bool,
    pub ty: TypeExpr,
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub target: Expr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Block,
    pub else_ifs: Vec<(Expr, Block)>,
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub variable: Ident,
    pub view: bool,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Ident,
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssertStmt {
    pub condition: Expr,
    pub message: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchStmt {
    pub expr: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

/// A pattern in a `match` arm.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Simple name match: `red`, `green`
    Ident(Ident),
    /// Destructuring variant: `circle(r)`, `rect(w, h)`
    Variant(Ident, Vec<Ident>),
    /// Catch-all: `other`
    Other(Span),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TypeExpr {
    /// A simple named type: `int64`, `string`, `Point`, `nothing`
    Named(Ident),
    /// A generic type: `list[string]`, `map[string, int64]`, `result[T, E]`
    Generic(Ident, Vec<TypeExpr>, Span),
    /// `view T` — a view/borrow of a type (used in `for item in view expr:`)
    View(Box<TypeExpr>, Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(ident) => ident.span,
            TypeExpr::Generic(_, _, span) => *span,
            TypeExpr::View(_, span) => *span,
        }
    }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Expr {
    /// Integer literal: `42`
    IntLiteral(i64, Span),
    /// Float literal: `3.14`
    FloatLiteral(f64, Span),
    /// String literal: `"hello"`
    StringLiteral(String, Span),
    /// Boolean literal: `true` / `false`
    BoolLiteral(bool, Span),
    /// `nothing` keyword
    Nothing(Span),
    /// Identifier: `x`, `name`
    Ident(Ident),
    /// Binary operation: `a + b`
    Binary(Box<Expr>, BinOp, Box<Expr>, Span),
    /// Unary operation: `not x`
    Unary(UnaryOp, Box<Expr>, Span),
    /// Field access: `expr.field`
    FieldAccess(Box<Expr>, Ident, Span),
    /// Function call: `foo(a, b)` or `Type.method(a, b)`
    Call(Box<Expr>, Vec<CallArg>, Span),
    /// Generic function call: `name[Type](args)`
    GenericCall(Box<Expr>, Vec<TypeExpr>, Vec<CallArg>, Span),
    /// Parenthesized expression
    Paren(Box<Expr>, Span),
    /// `view expr`
    View(Box<Expr>, Span),
    /// List construction: `list(a, b, c)`
    ListConstruct(Vec<Expr>, Span),
    /// Map construction: `map(key1: val1, key2: val2)`
    MapConstruct(Vec<(Expr, Expr)>, Span),
    /// Handle block: `expr handle error:` or `expr handle:`
    Handle(Box<Expr>, Option<Ident>, Block, Span),
    /// `ok(expr)`
    Ok(Box<Expr>, Span),
    /// `fail(expr)`
    Fail(Box<Expr>, Span),
    /// `some(expr)`
    Some(Box<Expr>, Span),
    /// `none`
    None(Span),
    /// `default expr` (inside handle block)
    Default(Box<Expr>, Span),
    /// Enum variant reference: `Color.red` (Type.variant)
    EnumVariant(Ident, Ident, Span),
    /// String interpolation: `"hello {name}, you are {age} years old"`
    StringInterpolation(Vec<StringPart>, Span),
    /// Error node for recovery
    Error(Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, s)
            | Expr::FloatLiteral(_, s)
            | Expr::StringLiteral(_, s)
            | Expr::BoolLiteral(_, s)
            | Expr::Nothing(s)
            | Expr::Binary(_, _, _, s)
            | Expr::Unary(_, _, s)
            | Expr::FieldAccess(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::GenericCall(_, _, _, s)
            | Expr::Paren(_, s)
            | Expr::View(_, s)
            | Expr::ListConstruct(_, s)
            | Expr::MapConstruct(_, s)
            | Expr::Handle(_, _, _, s)
            | Expr::Ok(_, s)
            | Expr::Fail(_, s)
            | Expr::Some(_, s)
            | Expr::None(s)
            | Expr::Default(_, s)
            | Expr::EnumVariant(_, _, s)
            | Expr::StringInterpolation(_, s)
            | Expr::Error(s) => *s,
            Expr::Ident(ident) => ident.span,
        }
    }
}

/// A part of a string interpolation expression.
#[derive(Debug, Clone)]
pub enum StringPart {
    /// A literal string segment.
    Literal(String),
    /// An interpolated expression: `{expr}`.
    Expr(Box<Expr>),
}

/// A call argument, optionally named: `foo(x: 10, 20)`
#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Modulo,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

// ---------------------------------------------------------------------------
// Identifier
// ---------------------------------------------------------------------------

/// A resolved or unresolved identifier with its source location.
#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}
