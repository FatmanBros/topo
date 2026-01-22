//! API service parsing - handles API service definitions (::)

use crate::ast::{
    ApiServiceDef, Endpoint, EventHandler, EventType, Expression, HttpMethod,
    ServerBlock, ServerHandler, ServerStatement, TypedParam,
};
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
        let mut server = None;
        let mut mock = None;

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
            } else if self.check(TokenKind::Server) {
                // Server block: server { on endpoint(params, ctx) { ... } }
                server = Some(self.server_block()?);
            } else if self.check(TokenKind::Mock) {
                // Mock data file: mock: "./mocks/users.json"
                self.advance();
                self.expect(TokenKind::Colon)?;
                if let Expression::String { value } = self.expression()? {
                    mock = Some(value);
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
            server,
            mock,
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

    /// Parse server block: `server { ... }`
    fn server_block(&mut self) -> Result<ServerBlock, ParseError> {
        self.expect(TokenKind::Server)?;
        self.expect(TokenKind::LBrace)?;

        let mut context = None;
        let mut middleware = Vec::new();
        let mut handlers = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.check(TokenKind::Context) {
                // context: ContextType
                self.advance();
                self.expect(TokenKind::Colon)?;
                context = Some(self.parse_type_annotation()?);
            } else if self.check(TokenKind::Middleware) {
                // middleware: [middlewareFn1, middlewareFn2]
                self.advance();
                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBracket)?;
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    middleware.push(self.expression()?);
                    let _ = self.match_token(TokenKind::Comma);
                }
                self.expect(TokenKind::RBracket)?;
            } else if self.check(TokenKind::On) {
                // on endpointName(params, ctx) { ... }
                handlers.push(self.server_handler()?);
            } else {
                // Skip unknown
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ServerBlock {
            context,
            middleware,
            handlers,
        })
    }

    /// Parse server handler: `on endpointName(param1, param2, ctx) { ... }`
    fn server_handler(&mut self) -> Result<ServerHandler, ParseError> {
        self.expect(TokenKind::On)?;
        let endpoint = self.expect_identifier()?;

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        let mut ctx_param = None;

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            let param_name = self.expect_identifier()?;

            // Check for type annotation: `param: Type`
            let type_ann = if self.check(TokenKind::Colon) {
                self.advance();
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            // Last param named `ctx` is the context parameter
            if self.check(TokenKind::RParen) || (self.check(TokenKind::Comma) && self.peek_next_is_rparen()) {
                if param_name == "ctx" {
                    ctx_param = Some(param_name);
                } else {
                    params.push(TypedParam {
                        name: param_name,
                        type_annotation: type_ann,
                    });
                }
            } else {
                params.push(TypedParam {
                    name: param_name,
                    type_annotation: type_ann,
                });
            }

            let _ = self.match_token(TokenKind::Comma);
        }

        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut body = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.server_statement()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ServerHandler {
            endpoint,
            params,
            ctx_param,
            body,
        })
    }

    /// Parse server statement inside handler body
    fn server_statement(&mut self) -> Result<ServerStatement, ParseError> {
        // return: expression
        if self.check(TokenKind::Return) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            let value = self.expression()?;
            return Ok(ServerStatement::Return { value });
        }

        // throw: ErrorType("message")
        if self.check(TokenKind::Throw) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            let error_type = self.expect_identifier()?;
            self.expect(TokenKind::LParen)?;
            let message = self.expression()?;
            self.expect(TokenKind::RParen)?;
            return Ok(ServerStatement::Throw { error_type, message });
        }

        // if (condition) { ... } else { ... }
        if self.check(TokenKind::If) {
            self.advance();
            self.expect(TokenKind::LParen)?;
            let condition = self.expression()?;
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::LBrace)?;

            let mut then_block = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                then_block.push(self.server_statement()?);
            }
            self.expect(TokenKind::RBrace)?;

            let else_block = if self.check(TokenKind::Else) {
                self.advance();
                self.expect(TokenKind::LBrace)?;
                let mut block = Vec::new();
                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    block.push(self.server_statement()?);
                }
                self.expect(TokenKind::RBrace)?;
                Some(block)
            } else {
                None
            };

            return Ok(ServerStatement::If {
                condition,
                then_block,
                else_block,
            });
        }

        // try { ... } catch(e) { ... }
        if self.check(TokenKind::Try) {
            self.advance();
            self.expect(TokenKind::LBrace)?;

            let mut try_block = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                try_block.push(self.server_statement()?);
            }
            self.expect(TokenKind::RBrace)?;

            self.expect(TokenKind::Catch)?;
            self.expect(TokenKind::LParen)?;
            let catch_param = self.expect_identifier()?;
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::LBrace)?;

            let mut catch_block = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                catch_block.push(self.server_statement()?);
            }
            self.expect(TokenKind::RBrace)?;

            return Ok(ServerStatement::TryCatch {
                try_block,
                catch_param,
                catch_block,
            });
        }

        // assignment: name: expression
        if self.check(TokenKind::Identifier) {
            let name = self.expect_identifier()?;
            if self.check(TokenKind::Colon) {
                self.advance();
                let value = self.expression()?;
                return Ok(ServerStatement::Assignment { name, value });
            }
            // If not an assignment, treat as expression
            // We need to construct an expression from the identifier
            return Ok(ServerStatement::Expression(Expression::Identifier { name }));
        }

        // Default: expression
        let expr = self.expression()?;
        Ok(ServerStatement::Expression(expr))
    }

    /// Helper: check if next token after current is RParen
    fn peek_next_is_rparen(&self) -> bool {
        if self.current + 1 < self.tokens.len() {
            self.tokens[self.current + 1].kind == TokenKind::RParen
        } else {
            false
        }
    }
}
