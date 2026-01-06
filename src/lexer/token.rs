//! Token definitions for topo language

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    // Literals
    Identifier,
    String,
    Number,
    True,
    False,
    Null,

    // Delimiters
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]
    LParen,     // (
    RParen,     // )
    Comma,      // ,
    Dot,        // .
    Colon,      // :
    ColonColon, // ::
    Arrow,      // ->
    Pipe,       // |
    Dollar,     // $

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Eq,         // =
    EqEq,       // ==
    Bang,       // !
    BangEq,     // !=
    Lt,         // <
    LtEq,       // <=
    Gt,         // >
    GtEq,       // >=
    AmpAmp,     // &&
    PipePipe,   // ||

    // Keywords - Control flow
    If,
    Else,
    For,
    On,
    Try,
    Catch,
    Await,
    Async,
    Dispatch,
    Return,

    // Keywords - Structure
    Extends,
    Import,
    Export,
    Layout,

    // Keywords - Store blocks
    State,
    Actions,
    Reducers,
    Effects,
    Selectors,

    // Keywords - API
    Rest,
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Headers,
    Auth,
    Timeout,

    // Keywords - Subscribe (WebSocket/SSE)
    Subscribe,
    Message,
    Error,
    Open,
    Close,
    FatArrow,   // =>

    // End of file
    Eof,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Number => write!(f, "number"),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Null => write!(f, "null"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::ColonColon => write!(f, "::"),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::Dollar => write!(f, "$"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::Eq => write!(f, "="),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::Bang => write!(f, "!"),
            TokenKind::BangEq => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::LtEq => write!(f, "<="),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::GtEq => write!(f, ">="),
            TokenKind::AmpAmp => write!(f, "&&"),
            TokenKind::PipePipe => write!(f, "||"),
            TokenKind::If => write!(f, "if"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::For => write!(f, "for"),
            TokenKind::On => write!(f, "on"),
            TokenKind::Try => write!(f, "try"),
            TokenKind::Catch => write!(f, "catch"),
            TokenKind::Await => write!(f, "await"),
            TokenKind::Async => write!(f, "async"),
            TokenKind::Dispatch => write!(f, "dispatch"),
            TokenKind::Return => write!(f, "return"),
            TokenKind::Extends => write!(f, "extends"),
            TokenKind::Import => write!(f, "import"),
            TokenKind::Export => write!(f, "export"),
            TokenKind::Layout => write!(f, "layout"),
            TokenKind::State => write!(f, "State"),
            TokenKind::Actions => write!(f, "Actions"),
            TokenKind::Reducers => write!(f, "Reducers"),
            TokenKind::Effects => write!(f, "Effects"),
            TokenKind::Selectors => write!(f, "Selectors"),
            TokenKind::Rest => write!(f, "rest"),
            TokenKind::Get => write!(f, "get"),
            TokenKind::Post => write!(f, "post"),
            TokenKind::Put => write!(f, "put"),
            TokenKind::Patch => write!(f, "patch"),
            TokenKind::Delete => write!(f, "delete"),
            TokenKind::Headers => write!(f, "headers"),
            TokenKind::Auth => write!(f, "auth"),
            TokenKind::Timeout => write!(f, "timeout"),
            TokenKind::Subscribe => write!(f, "subscribe"),
            TokenKind::Message => write!(f, "message"),
            TokenKind::Error => write!(f, "error"),
            TokenKind::Open => write!(f, "open"),
            TokenKind::Close => write!(f, "close"),
            TokenKind::FatArrow => write!(f, "=>"),
            TokenKind::Eof => write!(f, "EOF"),
        }
    }
}
