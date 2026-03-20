use jett_common::Span;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// All possible token kinds in Jett.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords
    Function,
    Return,
    Returns,
    If,
    Else,
    For,
    In,
    Into,
    While,
    Struct,
    Enum,
    Match,
    Use,
    Mutable,
    Handle,
    Error,
    Default,
    Result,
    Ok,
    Fail,
    Clone,
    View,
    Type,
    Where,
    Machine,
    States,
    Transitions,
    To,
    At,
    Is,
    Actor,
    Receive,
    Send,
    Ask,
    Respond,
    Spawn,
    Run,
    Join,
    Cancel,
    Comptime,
    Verify,
    Property,
    Given,
    Trace,
    Breakpoint,
    Secret,
    Declassify,
    Coarsen,
    Serialize,
    Namespace,
    Bitfield,
    Bit,
    Bits,
    Network,
    Implement,
    Interface,
    Mutual,
    Assert,
    Some,
    None,
    Nothing,
    True,
    False,
    Modulo,
    As,
    Break,
    Continue,
    And,
    Within,
    Self_,
    Value,
    Transition,
    Optional,
    Other,
    Not,

    // Type keywords
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Float32,
    Float64,
    String_,
    Bool_,
    Bytes_,
    List_,
    Map_,
    Set_,

    // Literals
    IntLiteral,
    FloatLiteral,
    StringStart,
    StringMid,
    StringEnd,
    StringLiteral,

    // Symbols
    Eq,         // =
    EqEq,       // ==
    NotEq,      // !=
    Lt,         // <
    Gt,         // >
    LtEq,       // <=
    GtEq,       // >=
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    AmpAmp,     // &&
    PipePipe,   // ||
    Bang,        // !
    Dot,         // .
    Comma,       // ,
    Colon,       // :
    LParen,      // (
    RParen,      // )
    LBracket,    // [
    RBracket,    // ]
    Hash,        // #

    // Structural
    Newline,
    Indent,
    Dedent,

    // Special
    Eof,
    InvalidToken,

    // Identifiers
    Ident,
}
