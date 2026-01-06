//! Lexer (Tokenizer) for topo language
//!
//! Converts source code into a stream of tokens.

mod token;

pub use token::{Token, TokenKind};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LexerError {
    #[error("Unexpected character '{0}' at line {1}, column {2}")]
    UnexpectedChar(char, usize, usize),

    #[error("Unterminated string at line {0}, column {1}")]
    UnterminatedString(usize, usize),

    #[error("Invalid number at line {0}, column {1}")]
    InvalidNumber(usize, usize),
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    column: usize,
    start_line: usize,
    start_column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            line: 1,
            column: 1,
            start_line: 1,
            start_column: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();

        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            line: self.line,
            column: self.column,
        });

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Option<Token>, LexerError> {
        self.skip_whitespace_and_comments();

        self.start_line = self.line;
        self.start_column = self.column;

        let Some((start, ch)) = self.advance() else {
            return Ok(None);
        };

        let kind = match ch {
            // Single character tokens
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '%' => TokenKind::Percent,
            '$' => TokenKind::Dollar,
            '@' => TokenKind::At,
            '?' => TokenKind::Question,

            // Two character tokens
            '-' => {
                if self.match_char('>') {
                    TokenKind::Arrow // ->
                } else {
                    TokenKind::Minus
                }
            }
            ':' => {
                if self.match_char(':') {
                    TokenKind::ColonColon // ::
                } else {
                    TokenKind::Colon
                }
            }
            '|' => {
                if self.match_char('|') {
                    TokenKind::PipePipe // ||
                } else {
                    TokenKind::Pipe // |
                }
            }
            '&' => {
                if self.match_char('&') {
                    TokenKind::AmpAmp // &&
                } else {
                    return Err(LexerError::UnexpectedChar(ch, self.line, self.column));
                }
            }
            '=' => {
                if self.match_char('=') {
                    TokenKind::EqEq // ==
                } else if self.match_char('>') {
                    TokenKind::FatArrow // =>
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if self.match_char('=') {
                    TokenKind::BangEq // !=
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.match_char('=') {
                    TokenKind::LtEq // <=
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.match_char('=') {
                    TokenKind::GtEq // >=
                } else {
                    TokenKind::Gt
                }
            }
            '/' => {
                // Division (comments are handled in skip_whitespace_and_comments)
                TokenKind::Slash
            }

            // String literals
            '"' => self.string()?,

            // Numbers
            c if c.is_ascii_digit() => self.number(start)?,

            // Identifiers and keywords
            c if c.is_ascii_alphabetic() || c == '_' => self.identifier(start),

            _ => return Err(LexerError::UnexpectedChar(ch, self.start_line, self.start_column)),
        };

        let end = self.current_position();
        let lexeme = self.source[start..end].to_string();

        Ok(Some(Token {
            kind,
            lexeme,
            line: self.start_line,
            column: self.start_column,
        }))
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let result = self.chars.next();
        if let Some((_, ch)) = result {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        result
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, ch)| *ch)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn current_position(&mut self) -> usize {
        self.chars.peek().map(|(pos, _)| *pos).unwrap_or(self.source.len())
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    self.advance();
                }
                Some('/') => {
                    // Save current state to check for comments
                    let mut temp_chars = self.chars.clone();
                    temp_chars.next(); // consume '/'

                    match temp_chars.peek() {
                        Some((_, '/')) => {
                            // Line comment
                            self.advance(); // consume '/'
                            self.advance(); // consume '/'
                            while let Some(ch) = self.peek() {
                                if ch == '\n' {
                                    break;
                                }
                                self.advance();
                            }
                        }
                        Some((_, '*')) => {
                            // Block comment
                            self.advance(); // consume '/'
                            self.advance(); // consume '*'
                            let mut depth = 1;
                            while depth > 0 {
                                match self.advance() {
                                    Some((_, '*')) => {
                                        if self.match_char('/') {
                                            depth -= 1;
                                        }
                                    }
                                    Some((_, '/')) => {
                                        if self.match_char('*') {
                                            depth += 1;
                                        }
                                    }
                                    None => break,
                                    _ => {}
                                }
                            }
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self) -> Result<TokenKind, LexerError> {
        let start_line = self.start_line;
        let start_column = self.start_column;

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance(); // consume closing quote
                return Ok(TokenKind::String);
            }
            if ch == '\n' {
                return Err(LexerError::UnterminatedString(start_line, start_column));
            }
            if ch == '\\' {
                self.advance(); // consume backslash
                self.advance(); // consume escaped character
            } else {
                self.advance();
            }
        }

        Err(LexerError::UnterminatedString(start_line, start_column))
    }

    fn number(&mut self, _start: usize) -> Result<TokenKind, LexerError> {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal
        if self.peek() == Some('.') {
            // Check if next char is a digit
            let mut temp = self.chars.clone();
            temp.next(); // consume '.'
            if let Some((_, ch)) = temp.peek() {
                if ch.is_ascii_digit() {
                    self.advance(); // consume '.'
                    while let Some(ch) = self.peek() {
                        if ch.is_ascii_digit() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        Ok(TokenKind::Number)
    }

    fn identifier(&mut self, start: usize) -> TokenKind {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let end = self.current_position();
        let text = &self.source[start..end];

        // Check for keywords
        match text {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "on" => TokenKind::On,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "await" => TokenKind::Await,
            "async" => TokenKind::Async,
            "dispatch" => TokenKind::Dispatch,
            "return" => TokenKind::Return,
            "extends" => TokenKind::Extends,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "from" => TokenKind::From,
            "layout" => TokenKind::Layout,
            // Store blocks
            "State" => TokenKind::State,
            "Actions" => TokenKind::Actions,
            "Reducers" => TokenKind::Reducers,
            "Effects" => TokenKind::Effects,
            "Selectors" => TokenKind::Selectors,
            // API
            "rest" => TokenKind::Rest,
            "get" => TokenKind::Get,
            "post" => TokenKind::Post,
            "put" => TokenKind::Put,
            "patch" => TokenKind::Patch,
            "delete" => TokenKind::Delete,
            "headers" => TokenKind::Headers,
            "auth" => TokenKind::Auth,
            "timeout" => TokenKind::Timeout,
            // Subscribe (WebSocket/SSE)
            "subscribe" => TokenKind::Subscribe,
            "message" => TokenKind::Message,
            "error" => TokenKind::Error,
            "open" => TokenKind::Open,
            "close" => TokenKind::Close,
            _ => TokenKind::Identifier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_definition() {
        let source = r#"Button -> {
            type: button
            content: "Click"
        }"#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].lexeme, "Button");
        assert_eq!(tokens[1].kind, TokenKind::Arrow);
        assert_eq!(tokens[2].kind, TokenKind::LBrace);
    }

    #[test]
    fn test_store_definition() {
        let source = "Counter | { State { count: 0 } }";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].kind, TokenKind::Pipe);
        assert_eq!(tokens[2].kind, TokenKind::LBrace);
    }

    #[test]
    fn test_api_service_definition() {
        let source = r#"User :: { rest: "/api/users" }"#;

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].kind, TokenKind::ColonColon);
        assert_eq!(tokens[2].kind, TokenKind::LBrace);
    }

    #[test]
    fn test_reference() {
        let source = "$Counter.count";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Dollar);
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::Dot);
        assert_eq!(tokens[3].kind, TokenKind::Identifier);
    }
}
