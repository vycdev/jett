mod lexer;
mod token;

pub use lexer::{CommentTrivia, LexError, LexResult, Lexer, tokenize};
pub use token::{Token, TokenKind};
