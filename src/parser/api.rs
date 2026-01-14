//! API service parsing - handles API service definitions (::)

use crate::ast::{ApiServiceDef, Endpoint, EventHandler, EventType, Expression, HttpMethod};
use crate::lexer::TokenKind;
use crate::parser::{ParseError, Parser};

impl Parser {
    pub(super) fn api_service_def(&mut self, name: String) -> Result<ApiServiceDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut rest = None;
        let mut endpoints = Vec::new();
        let mut headers = None;
        let mut auth = None;
        let mut timeout = None;
        let mut subscribe = None;
        let mut event_handlers = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.check(TokenKind::Rest) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                if let Expression::String { value } = self.expression()? {
                    rest = Some(value);
                }
            } else if self.check(TokenKind::Subscribe) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                if let Expression::String { value } = self.expression()? {
                    subscribe = Some(value);
                }
            } else if self.check(TokenKind::On) {
                // Event handler: on message => ActionName
                self.advance();
                let event = match self.peek().kind {
                    TokenKind::Message => {
                        self.advance();
                        EventType::Message
                    }
                    TokenKind::Error => {
                        self.advance();
                        EventType::Error
                    }
                    TokenKind::Open => {
                        self.advance();
                        EventType::Open
                    }
                    TokenKind::Close => {
                        self.advance();
                        EventType::Close
                    }
                    _ => {
                        let token = self.peek();
                        return Err(ParseError::UnexpectedToken {
                            expected: "event type (message, error, open, close)".to_string(),
                            found: token.lexeme.clone(),
                            line: token.line,
                            column: token.column,
                        });
                    }
                };
                self.expect(TokenKind::FatArrow)?;
                let action = self.expect_identifier()?;
                event_handlers.push(EventHandler { event, action });
            } else if self.check(TokenKind::Headers) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBrace)?;
                let mut header_props = Vec::new();
                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    header_props.push(self.property()?);
                    // Handle optional comma between properties (JSON-like syntax)
                    if !self.check(TokenKind::RBrace) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RBrace)?;
                headers = Some(header_props);
            } else if self.check(TokenKind::Auth) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                auth = Some(self.expression()?);
            } else if self.check(TokenKind::Timeout) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                if let Expression::Number { value } = self.expression()? {
                    timeout = Some(value as u32);
                }
            } else if self.check(TokenKind::Identifier) {
                // Custom endpoint
                endpoints.push(self.endpoint()?);
            } else {
                // Skip unknown
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ApiServiceDef {
            name,
            rest,
            endpoints,
            headers,
            auth,
            timeout,
            subscribe,
            event_handlers,
        })
    }

    fn endpoint(&mut self) -> Result<Endpoint, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;

        let method = match self.peek().kind {
            TokenKind::Get => {
                self.advance();
                HttpMethod::Get
            }
            TokenKind::Post => {
                self.advance();
                HttpMethod::Post
            }
            TokenKind::Put => {
                self.advance();
                HttpMethod::Put
            }
            TokenKind::Patch => {
                self.advance();
                HttpMethod::Patch
            }
            TokenKind::Delete => {
                self.advance();
                HttpMethod::Delete
            }
            _ => {
                let token = self.peek();
                return Err(ParseError::UnexpectedToken {
                    expected: "HTTP method".to_string(),
                    found: token.lexeme.clone(),
                    line: token.line,
                    column: token.column,
                });
            }
        };

        self.expect(TokenKind::LParen)?;
        let path = if let Expression::String { value } = self.expression()? {
            value
        } else {
            String::new()
        };
        self.expect(TokenKind::RParen)?;

        let mut request_type = None;
        let mut response_type = None;
        let mut error_type = None;
        let mut params_type = None;

        // Check for type annotations
        // Syntax 1: `-> ResponseType` (simple response type)
        // Syntax 2: `{ request: Type, response: Type, error: Type, params: Type }` (full block)
        if self.check(TokenKind::Arrow) {
            // Simple response type: `-> User`
            self.advance();
            response_type = Some(self.parse_type_annotation()?);
        } else if self.check(TokenKind::LBrace) {
            // Full block syntax: `{ request: X, response: Y, error: Z, params: P }`
            self.advance();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                // Handle field names - `error` is a keyword so we need special handling
                let field_name = if self.check(TokenKind::Error) {
                    self.advance();
                    "error".to_string()
                } else {
                    self.expect_identifier()?
                };
                self.expect(TokenKind::Colon)?;
                let type_ann = self.parse_type_annotation()?;

                match field_name.as_str() {
                    "request" => request_type = Some(type_ann),
                    "response" => response_type = Some(type_ann),
                    "error" => error_type = Some(type_ann),
                    "params" => params_type = Some(type_ann),
                    _ => {
                        // Unknown field, ignore for forward compatibility
                    }
                }

                // Optional comma
                let _ = self.match_token(TokenKind::Comma);
            }
            self.expect(TokenKind::RBrace)?;
        }

        Ok(Endpoint {
            name,
            method,
            path,
            request_type,
            response_type,
            error_type,
            params_type,
        })
    }
}
