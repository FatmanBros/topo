//! Parser for topo language
//!
//! Converts a stream of tokens into an AST.

use crate::ast::*;
use crate::lexer::{Token, TokenKind};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected token: expected {expected}, found {found} at line {line}, column {column}")]
    UnexpectedToken {
        expected: String,
        found: String,
        line: usize,
        column: usize,
    },

    #[error("Unexpected end of file")]
    UnexpectedEof,

    #[error("Invalid definition operator at line {line}, column {column}")]
    InvalidDefinitionOperator { line: usize, column: usize },
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut declarations = Vec::new();

        while !self.is_at_end() {
            declarations.push(self.declaration()?);
        }

        Ok(Program { declarations })
    }

    // ========================================================================
    // Top-level declarations
    // ========================================================================

    fn declaration(&mut self) -> Result<Declaration, ParseError> {
        // All declarations start with an identifier
        let name = self.expect_identifier()?;

        // Check for optional parameters: Name(param1, param2)
        let params = if self.check(TokenKind::LParen) {
            self.advance();
            let mut params = Vec::new();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                params.push(self.expect_identifier_or_keyword()?);
                if !self.check(TokenKind::RParen) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RParen)?;
            params
        } else {
            Vec::new()
        };

        // Check which type of definition this is based on the operator
        if self.check(TokenKind::Arrow) {
            // Component: Name(params) -> { }
            self.advance();
            Ok(Declaration::Component(self.component_def(name, params)?))
        } else if self.check(TokenKind::ColonColon) {
            // API Service: Name :: { }
            self.advance();
            Ok(Declaration::ApiService(self.api_service_def(name)?))
        } else if self.check(TokenKind::Pipe) {
            // Store: Name | { }
            self.advance();
            Ok(Declaration::Store(self.store_def(name)?))
        } else if self.check(TokenKind::Star) {
            // Theme: Name * { }
            self.advance();
            Ok(Declaration::Theme(self.theme_def(name)?))
        } else if self.check(TokenKind::LBrace) {
            // Method: Name { }
            Ok(Declaration::Method(self.method_def(name)?))
        } else {
            let token = self.peek();
            Err(ParseError::InvalidDefinitionOperator {
                line: token.line,
                column: token.column,
            })
        }
    }

    // ========================================================================
    // Component Definition (->)
    // ========================================================================

    fn component_def(&mut self, name: String, params: Vec<String>) -> Result<ComponentDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            properties.push(self.property()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ComponentDef { name, params, properties })
    }

    // ========================================================================
    // Theme Definition (*)
    // ========================================================================

    fn theme_def(&mut self, name: String) -> Result<ThemeDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            properties.push(self.property()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ThemeDef { name, properties })
    }

    // ========================================================================
    // Method Definition ({})
    // ========================================================================

    fn method_def(&mut self, name: String) -> Result<MethodDef, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let body = self.expression()?;
        self.expect(TokenKind::RBrace)?;

        Ok(MethodDef { name, body })
    }

    // ========================================================================
    // API Service Definition (::)
    // ========================================================================

    fn api_service_def(&mut self, name: String) -> Result<ApiServiceDef, ParseError> {
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

        Ok(Endpoint { name, method, path })
    }

    // ========================================================================
    // Store Definition (|)
    // ========================================================================

    fn store_def(&mut self, name: String) -> Result<StoreDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut state = None;
        let mut actions = None;
        let mut reducers = None;
        let mut effects = None;
        let mut selectors = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            match self.peek().kind {
                TokenKind::State => {
                    self.advance();
                    state = Some(self.state_block()?);
                }
                TokenKind::Actions => {
                    self.advance();
                    actions = Some(self.actions_block()?);
                }
                TokenKind::Reducers => {
                    self.advance();
                    reducers = Some(self.reducers_block()?);
                }
                TokenKind::Effects => {
                    self.advance();
                    effects = Some(self.effects_block()?);
                }
                TokenKind::Selectors => {
                    self.advance();
                    selectors = Some(self.selectors_block()?);
                }
                _ => {
                    self.advance();
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(StoreDef {
            name,
            state,
            actions,
            reducers,
            effects,
            selectors,
        })
    }

    fn state_block(&mut self) -> Result<StateBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            fields.push(self.property()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(StateBlock { fields })
    }

    fn actions_block(&mut self) -> Result<ActionsBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut actions = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let name = self.expect_identifier()?;
            let mut params = Vec::new();

            if self.check(TokenKind::LParen) {
                self.advance();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    let param_name = self.expect_identifier_or_keyword()?;
                    let type_annotation = if self.check(TokenKind::Colon) {
                        self.advance();
                        Some(self.expect_identifier()?)
                    } else {
                        None
                    };
                    params.push(Param {
                        name: param_name,
                        type_annotation,
                    });

                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
            }

            actions.push(ActionDef { name, params });
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ActionsBlock { actions })
    }

    fn reducers_block(&mut self) -> Result<ReducersBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut handlers = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.check(TokenKind::On) {
                self.advance();
                handlers.push(self.reducer_handler()?);
            } else {
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ReducersBlock { handlers })
    }

    fn reducer_handler(&mut self) -> Result<ReducerHandler, ParseError> {
        let action = self.expect_identifier()?;
        let mut params = Vec::new();

        if self.check(TokenKind::LParen) {
            self.advance();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                params.push(self.expect_identifier_or_keyword()?);
                if !self.check(TokenKind::RParen) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RParen)?;
        }

        self.expect(TokenKind::LBrace)?;

        let mut body = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.property()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ReducerHandler {
            action,
            params,
            body,
        })
    }

    fn effects_block(&mut self) -> Result<EffectsBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut handlers = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.check(TokenKind::On) {
                self.advance();
                handlers.push(self.effect_handler()?);
            } else {
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(EffectsBlock { handlers })
    }

    fn effect_handler(&mut self) -> Result<EffectHandler, ParseError> {
        let action = self.expect_identifier()?;
        let mut params = Vec::new();

        if self.check(TokenKind::LParen) {
            self.advance();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                params.push(self.expect_identifier_or_keyword()?);
                if !self.check(TokenKind::RParen) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RParen)?;
        }

        self.expect(TokenKind::LBrace)?;

        let body = self.statements()?;

        self.expect(TokenKind::RBrace)?;

        Ok(EffectHandler {
            action,
            params,
            body,
        })
    }

    fn selectors_block(&mut self) -> Result<SelectorsBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut selectors = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let name = self.expect_identifier()?;
            self.expect(TokenKind::LBrace)?;
            let body = self.expression()?;
            self.expect(TokenKind::RBrace)?;
            selectors.push(SelectorDef { name, body });
        }

        self.expect(TokenKind::RBrace)?;

        Ok(SelectorsBlock { selectors })
    }

    // ========================================================================
    // Statements (for Effects)
    // ========================================================================

    fn statements(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
        }

        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Statement, ParseError> {
        if self.check(TokenKind::Try) {
            self.advance();
            return self.try_catch_statement();
        }

        if self.check(TokenKind::Dispatch) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            let action = self.expect_identifier()?;
            let mut args = Vec::new();
            if self.check(TokenKind::LParen) {
                self.advance();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    args.push(self.expression()?);
                    if !self.check(TokenKind::RParen) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RParen)?;
            }
            return Ok(Statement::Dispatch { action, args });
        }

        // Assignment: name: value
        if self.check(TokenKind::Identifier) {
            let name = self.expect_identifier()?;
            if self.check(TokenKind::Colon) {
                self.advance();
                let value = self.expression()?;
                return Ok(Statement::Assignment { name, value });
            }
            // Not an assignment, treat as expression
            return Ok(Statement::Expression(Expression::Identifier { name }));
        }

        // Expression statement
        let expr = self.expression()?;
        Ok(Statement::Expression(expr))
    }

    fn try_catch_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let try_block = self.statements()?;
        self.expect(TokenKind::RBrace)?;

        self.expect(TokenKind::Catch)?;
        self.expect(TokenKind::LParen)?;
        let catch_param = self.expect_identifier_or_keyword()?;
        self.expect(TokenKind::RParen)?;

        self.expect(TokenKind::LBrace)?;
        let catch_block = self.statements()?;
        self.expect(TokenKind::RBrace)?;

        Ok(Statement::TryCatch {
            try_block,
            catch_param,
            catch_block,
        })
    }

    // ========================================================================
    // Properties
    // ========================================================================

    fn property(&mut self) -> Result<Property, ParseError> {
        // Parse annotations before the property
        let annotations = self.parse_annotations()?;

        let key = self.expect_property_key()?;
        self.expect(TokenKind::Colon)?;
        let value = self.expression()?;

        Ok(Property {
            key,
            value,
            annotations,
        })
    }

    // ========================================================================
    // Annotations
    // ========================================================================

    fn parse_annotations(&mut self) -> Result<Vec<Annotation>, ParseError> {
        let mut annotations = Vec::new();

        while self.check(TokenKind::At) {
            self.advance();
            annotations.push(self.parse_annotation()?);
        }

        Ok(annotations)
    }

    fn parse_annotation(&mut self) -> Result<Annotation, ParseError> {
        let name = self.expect_identifier()?;
        let mut args = Vec::new();

        // Check for optional arguments: @min(3), @pattern("[a-z]+")
        if self.check(TokenKind::LParen) {
            self.advance();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                args.push(self.expression()?);
                if !self.check(TokenKind::RParen) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RParen)?;
        }

        Ok(Annotation { name, args })
    }

    /// Accept identifiers or keywords as property keys
    fn expect_property_key(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        // Allow keywords to be used as property keys
        let is_valid_key = matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Error
                | TokenKind::Message
                | TokenKind::Open
                | TokenKind::Close
                | TokenKind::State
                | TokenKind::Actions
                | TokenKind::Reducers
                | TokenKind::Effects
                | TokenKind::Selectors
                | TokenKind::Rest
                | TokenKind::Get
                | TokenKind::Post
                | TokenKind::Put
                | TokenKind::Patch
                | TokenKind::Delete
                | TokenKind::Headers
                | TokenKind::Auth
                | TokenKind::Timeout
                | TokenKind::Subscribe
        );

        if is_valid_key {
            self.advance();
            Ok(token.lexeme)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "property key".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            })
        }
    }

    // ========================================================================
    // Expressions
    // ========================================================================

    fn expression(&mut self) -> Result<Expression, ParseError> {
        self.or_expression()
    }

    fn or_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.and_expression()?;

        while self.check(TokenKind::PipePipe) {
            self.advance();
            let right = self.and_expression()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn and_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.equality()?;

        while self.check(TokenKind::AmpAmp) {
            self.advance();
            let right = self.equality()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn equality(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.comparison()?;

        loop {
            let op = if self.check(TokenKind::EqEq) {
                self.advance();
                BinaryOperator::Eq
            } else if self.check(TokenKind::BangEq) {
                self.advance();
                BinaryOperator::Ne
            } else {
                break;
            };

            let right = self.comparison()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.term()?;

        loop {
            let op = if self.check(TokenKind::Lt) {
                self.advance();
                BinaryOperator::Lt
            } else if self.check(TokenKind::LtEq) {
                self.advance();
                BinaryOperator::Le
            } else if self.check(TokenKind::Gt) {
                self.advance();
                BinaryOperator::Gt
            } else if self.check(TokenKind::GtEq) {
                self.advance();
                BinaryOperator::Ge
            } else {
                break;
            };

            let right = self.term()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn term(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.factor()?;

        loop {
            let op = if self.check(TokenKind::Plus) {
                self.advance();
                BinaryOperator::Add
            } else if self.check(TokenKind::Minus) {
                self.advance();
                BinaryOperator::Sub
            } else {
                break;
            };

            let right = self.factor()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn factor(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.unary()?;

        loop {
            let op = if self.check(TokenKind::Star) {
                self.advance();
                BinaryOperator::Mul
            } else if self.check(TokenKind::Slash) {
                self.advance();
                BinaryOperator::Div
            } else if self.check(TokenKind::Percent) {
                self.advance();
                BinaryOperator::Mod
            } else {
                break;
            };

            let right = self.unary()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn unary(&mut self) -> Result<Expression, ParseError> {
        if self.check(TokenKind::Bang) {
            self.advance();
            let operand = self.unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(operand),
            });
        }

        if self.check(TokenKind::Minus) {
            self.advance();
            let operand = self.unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Neg,
                operand: Box::new(operand),
            });
        }

        if self.check(TokenKind::Await) {
            self.advance();
            let expr = self.unary()?;
            return Ok(Expression::Await {
                expr: Box::new(expr),
            });
        }

        self.call()
    }

    fn call(&mut self) -> Result<Expression, ParseError> {
        let mut expr = self.primary()?;

        loop {
            if self.check(TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    args.push(self.expression()?);
                    if !self.check(TokenKind::RParen) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RParen)?;
                expr = Expression::Call {
                    callee: Box::new(expr),
                    args,
                };
            } else if self.check(TokenKind::Dot) {
                self.advance();
                let property = self.expect_identifier()?;
                expr = Expression::MemberAccess {
                    object: Box::new(expr),
                    property,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::String => {
                self.advance();
                // Remove quotes from the lexeme
                let value = token.lexeme[1..token.lexeme.len() - 1].to_string();
                Ok(Expression::String { value })
            }
            TokenKind::Number => {
                self.advance();
                let value: f64 = token.lexeme.parse().unwrap_or(0.0);
                Ok(Expression::Number { value })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::Boolean { value: true })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Boolean { value: false })
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression::Null)
            }
            TokenKind::For => {
                // for item in items { body }
                self.advance();
                let item = self.expect_identifier_or_keyword()?;
                // Expect 'in' - we'll use Identifier for now
                let in_token = self.peek().clone();
                if in_token.lexeme != "in" {
                    return Err(ParseError::UnexpectedToken {
                        expected: "in".to_string(),
                        found: in_token.lexeme,
                        line: in_token.line,
                        column: in_token.column,
                    });
                }
                self.advance();
                let items = self.expression()?;
                self.expect(TokenKind::LBrace)?;
                let body = self.expression()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Expression::ForIn {
                    item,
                    items: Box::new(items),
                    body: Box::new(body),
                })
            }
            TokenKind::Identifier => {
                self.advance();
                Ok(Expression::Identifier {
                    name: token.lexeme.clone(),
                })
            }
            TokenKind::Dollar => {
                self.advance();
                self.reference()
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    elements.push(self.expression()?);
                    if !self.check(TokenKind::RBracket) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Expression::Array { elements })
            }
            TokenKind::LBrace => {
                self.advance();
                let mut properties = Vec::new();
                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    properties.push(self.property()?);
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Expression::Object { properties })
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "expression".to_string(),
                found: token.lexeme.clone(),
                line: token.line,
                column: token.column,
            }),
        }
    }

    fn reference(&mut self) -> Result<Expression, ParseError> {
        let store = self.expect_identifier()?;
        let mut path = Vec::new();

        while self.check(TokenKind::Dot) {
            self.advance();
            path.push(self.expect_identifier()?);
        }

        Ok(Expression::Reference { store, path })
    }

    // ========================================================================
    // Utility methods
    // ========================================================================

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        &self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let token = self.peek();
            Err(ParseError::UnexpectedToken {
                expected: kind.to_string(),
                found: token.lexeme.clone(),
                line: token.line,
                column: token.column,
            })
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Identifier {
            self.advance();
            Ok(token.lexeme)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            })
        }
    }

    /// Accept identifiers or keywords as valid names (for parameters, etc.)
    fn expect_identifier_or_keyword(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        let is_valid = matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Error
                | TokenKind::Message
                | TokenKind::Open
                | TokenKind::Close
                | TokenKind::State
                | TokenKind::Actions
                | TokenKind::Reducers
                | TokenKind::Effects
                | TokenKind::Selectors
                | TokenKind::Rest
                | TokenKind::Get
                | TokenKind::Post
                | TokenKind::Put
                | TokenKind::Patch
                | TokenKind::Delete
                | TokenKind::Headers
                | TokenKind::Auth
                | TokenKind::Timeout
                | TokenKind::Subscribe
        );

        if is_valid {
            self.advance();
            Ok(token.lexeme)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "identifier".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Result<Program, ParseError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_parse_component() {
        let source = r#"
            Button -> {
                type: button
                content: "Click"
            }
        "#;

        let program = parse(source).unwrap();
        assert_eq!(program.declarations.len(), 1);

        if let Declaration::Component(comp) = &program.declarations[0] {
            assert_eq!(comp.name, "Button");
            assert_eq!(comp.properties.len(), 2);
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_store() {
        let source = r#"
            Counter | {
                State {
                    count: 0
                }
                Actions {
                    Increment
                    Decrement
                }
            }
        "#;

        let program = parse(source).unwrap();
        assert_eq!(program.declarations.len(), 1);

        if let Declaration::Store(store) = &program.declarations[0] {
            assert_eq!(store.name, "Counter");
            assert!(store.state.is_some());
            assert!(store.actions.is_some());
        } else {
            panic!("Expected store declaration");
        }
    }

    #[test]
    fn test_parse_api_service() {
        let source = r#"
            User :: {
                rest: "/api/users"
            }
        "#;

        let program = parse(source).unwrap();
        assert_eq!(program.declarations.len(), 1);

        if let Declaration::ApiService(api) = &program.declarations[0] {
            assert_eq!(api.name, "User");
            assert_eq!(api.rest, Some("/api/users".to_string()));
        } else {
            panic!("Expected API service declaration");
        }
    }
}
