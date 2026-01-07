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
        // Check for import statement
        if self.check(TokenKind::Import) {
            return self.import_statement();
        }

        // Check for Test definition: Test("name") { ... } or xTest("name") { ... }
        if self.check(TokenKind::Test) {
            return self.test_definition(false);
        }
        if self.check(TokenKind::XTest) {
            return self.test_definition(true);
        }

        // Check for BeforeEach/AfterEach hooks
        if self.check(TokenKind::BeforeEach) {
            return self.before_each_definition();
        }
        if self.check(TokenKind::AfterEach) {
            return self.after_each_definition();
        }
        // Check for BeforeOnce/AfterOnce hooks
        if self.check(TokenKind::BeforeOnce) {
            return self.before_once_definition();
        }
        if self.check(TokenKind::AfterOnce) {
            return self.after_once_definition();
        }

        // Check for anonymous store: | { ... }
        if self.check(TokenKind::Pipe) {
            self.advance();
            return Ok(Declaration::Store(self.store_def(None)?));
        }

        // All other declarations start with an identifier
        let name = self.expect_identifier()?;

        // Check for optional typed parameters: Name(param1: type, param2: type)
        let params = if self.check(TokenKind::LParen) {
            self.advance();
            let mut params = Vec::new();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                let param_name = self.expect_identifier_or_keyword()?;
                let type_annotation = if self.check(TokenKind::Colon) {
                    self.advance();
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };
                params.push(TypedParam {
                    name: param_name,
                    type_annotation,
                });
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
            Ok(Declaration::Store(self.store_def(Some(name))?))
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
    // Import Statement
    // ========================================================================

    fn import_statement(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::Import)?;

        // Check for named imports: import { Name, Other } from "path"
        let names = if self.check(TokenKind::LBrace) {
            self.advance();
            let mut names = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                names.push(self.expect_identifier()?);
                if !self.check(TokenKind::RBrace) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RBrace)?;
            self.expect(TokenKind::From)?;
            names
        } else {
            Vec::new()
        };

        // Expect the path string
        let path = self.expect_string()?;

        Ok(Declaration::Import(ImportDef { path, names }))
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        if token.kind == TokenKind::String {
            self.advance();
            // Remove quotes from the string
            let s = token.lexeme.trim_matches('"').to_string();
            Ok(s)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "string".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            })
        }
    }

    // ========================================================================
    // Component Definition (->)
    // ========================================================================

    fn component_def(&mut self, name: String, params: Vec<TypedParam>) -> Result<ComponentDef, ParseError> {
        // Check if this is an alias: Alias(args) -> Base(args, defaultValue)
        if self.check(TokenKind::Identifier) {
            let base = self.expect_identifier()?;
            self.expect(TokenKind::LParen)?;
            let mut args = Vec::new();
            while !self.check(TokenKind::RParen) && !self.is_at_end() {
                args.push(self.expression()?);
                if !self.check(TokenKind::RParen) {
                    let _ = self.match_token(TokenKind::Comma);
                }
            }
            self.expect(TokenKind::RParen)?;
            return Ok(ComponentDef {
                name,
                params,
                properties: Vec::new(),
                init: None,
                destroy: None,
                alias: Some(ComponentAlias { base, args }),
            });
        }

        // Regular component definition: Name(params) -> { ... }
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();
        let mut init = None;
        let mut destroy = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let prop = self.property()?;

            // Handle lifecycle hooks specially
            match prop.key.as_str() {
                "init" => init = Some(prop.value),
                "destroy" => destroy = Some(prop.value),
                _ => properties.push(prop),
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ComponentDef { name, params, properties, init, destroy, alias: None })
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

    fn store_def(&mut self, name: Option<String>) -> Result<StoreDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut state = None;
        let mut actions = None;
        let mut commands = None;
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
                TokenKind::Commands => {
                    self.advance();
                    commands = Some(self.commands_block()?);
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
            commands,
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
                        Some(self.parse_type_annotation()?)
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

    fn commands_block(&mut self) -> Result<CommandsBlock, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut commands = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let name = self.expect_identifier()?;
            let mut params = Vec::new();

            if self.check(TokenKind::LParen) {
                self.advance();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    let param_name = self.expect_identifier_or_keyword()?;
                    let type_annotation = if self.check(TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_type_annotation()?)
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

            commands.push(ActionDef { name, params });
        }

        self.expect(TokenKind::RBrace)?;

        Ok(CommandsBlock { commands })
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

        // Check for type annotation: key: type = value
        let (type_annotation, value) = if self.is_type_start() {
            let type_ann = self.parse_type_annotation()?;
            self.expect(TokenKind::Eq)?;
            let val = self.expression()?;
            (Some(type_ann), val)
        } else {
            (None, self.expression()?)
        };

        Ok(Property {
            key,
            value,
            type_annotation,
            annotations,
        })
    }

    /// Check if the next token could start a type annotation
    fn is_type_start(&self) -> bool {
        let token = self.peek();
        // Type annotations start with identifiers like "string", "number", "boolean", etc.
        // We distinguish from values by checking what follows
        if token.kind != TokenKind::Identifier {
            return false;
        }

        let name = &token.lexeme;

        // Check what follows this identifier
        let next_token = self.peek_ahead(1);

        // If followed by '.', '(' - it's an expression (Store.Action, func())
        if matches!(next_token.kind, TokenKind::Dot | TokenKind::LParen) {
            return false;
        }

        // Primitive type names are always types
        if matches!(
            name.as_str(),
            "string" | "number" | "boolean" | "any" | "void" | "null" | "undefined"
        ) {
            return true;
        }

        // Uppercase identifiers followed by '=' or '[]' or '?' or '|' are types
        if name.chars().next().map_or(false, |c| c.is_uppercase()) {
            return matches!(
                next_token.kind,
                TokenKind::Eq | TokenKind::LBracket | TokenKind::Question | TokenKind::Pipe
            );
        }

        false
    }

    /// Peek ahead by n tokens (0 = current)
    fn peek_ahead(&self, n: usize) -> &Token {
        self.tokens.get(self.current + n).unwrap_or(&self.tokens[self.tokens.len() - 1])
    }

    // ========================================================================
    // Type Annotations
    // ========================================================================

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        let mut base_type = self.parse_primary_type()?;

        // Check for array suffix: type[]
        while self.check(TokenKind::LBracket) {
            self.advance();
            self.expect(TokenKind::RBracket)?;
            base_type = TypeAnnotation::Array {
                element_type: Box::new(base_type),
            };
        }

        // Check for optional suffix: type?
        if self.check(TokenKind::Question) {
            self.advance();
            base_type = TypeAnnotation::Optional {
                inner_type: Box::new(base_type),
            };
        }

        // Check for union: type | type
        if self.check(TokenKind::Pipe) {
            let mut types = vec![base_type];
            while self.check(TokenKind::Pipe) {
                self.advance();
                let next_type = self.parse_primary_type()?;
                types.push(next_type);
            }
            return Ok(TypeAnnotation::Union { types });
        }

        Ok(base_type)
    }

    fn parse_primary_type(&mut self) -> Result<TypeAnnotation, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::Identifier => {
                self.advance();
                let name = token.lexeme;

                // Check if it's a primitive type
                if matches!(
                    name.as_str(),
                    "string" | "number" | "boolean" | "any" | "void" | "null" | "undefined"
                ) {
                    Ok(TypeAnnotation::Primitive { name })
                } else {
                    // Otherwise it's a reference to another type
                    Ok(TypeAnnotation::Reference { name })
                }
            }
            TokenKind::LBrace => {
                // Object type: { name: string, age: number }
                self.advance();
                let mut fields = Vec::new();

                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    let field_name = self.expect_identifier()?;
                    self.expect(TokenKind::Colon)?;
                    let field_type = self.parse_type_annotation()?;
                    fields.push(TypedField {
                        name: field_name,
                        type_annotation: field_type,
                    });

                    if !self.check(TokenKind::RBrace) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }

                self.expect(TokenKind::RBrace)?;
                Ok(TypeAnnotation::Object { fields })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "type".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
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
        let name = self.expect_annotation_name()?;
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

    /// Accept identifiers or keywords as annotation names (e.g. @hidden, @key, @required)
    fn expect_annotation_name(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        let is_valid = matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Hidden
                | TokenKind::Type
                | TokenKind::Text
                | TokenKind::State
        );

        if is_valid {
            self.advance();
            Ok(token.lexeme)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "annotation name".to_string(),
                found: format!("{}", token.kind),
                line: token.line,
                column: token.column,
            })
        }
    }

    /// Accept identifiers or keywords as property keys
    fn expect_property_key(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        // Allow keywords to be used as property keys
        let is_valid_key = matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Type
                | TokenKind::Button
                | TokenKind::Submit
                | TokenKind::Text
                | TokenKind::Click
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
                | TokenKind::Visible
                | TokenKind::Hidden
                | TokenKind::Url
                | TokenKind::Fill
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
    // Test Definition
    // ========================================================================

    fn test_definition(&mut self, skip: bool) -> Result<Declaration, ParseError> {
        // Consume Test or xTest
        self.advance();
        // Expect Test("name") or xTest("name") syntax
        self.expect(TokenKind::LParen)?;
        let name = self.expect_string()?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::Test(TestDef { name, statements, skip }))
    }

    fn before_each_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::BeforeEach)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::BeforeEach(TestHookDef { statements }))
    }

    fn after_each_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::AfterEach)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::AfterEach(TestHookDef { statements }))
    }

    fn before_once_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::BeforeOnce)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::BeforeOnce(TestHookDef { statements }))
    }

    fn after_once_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::AfterOnce)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::AfterOnce(TestHookDef { statements }))
    }

    fn test_statement(&mut self) -> Result<TestStatement, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::Goto => {
                // goto("/path")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let path = self.expect_string()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Goto { path })
            }
            TokenKind::Click => {
                // click(target)
                self.advance();
                self.expect(TokenKind::LParen)?;
                let target = self.test_target()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Click { target })
            }
            TokenKind::Fill => {
                // fill(target, "value")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let target = self.test_target()?;
                self.expect(TokenKind::Comma)?;
                let value = self.expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Fill { target, value })
            }
            TokenKind::Type => {
                // type(target, "value")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let target = self.test_target()?;
                self.expect(TokenKind::Comma)?;
                let value = self.expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Type { target, value })
            }
            TokenKind::Expect => {
                // expect(target, { visible }) or expect(target, "value")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let target = self.test_target()?;
                self.expect(TokenKind::Comma)?;
                let assertion = self.test_assertion()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Expect { target, assertion })
            }
            TokenKind::Mock => {
                // mock(Service.method, { response })
                self.advance();
                self.expect(TokenKind::LParen)?;
                let service = self.expect_identifier()?;
                self.expect(TokenKind::Dot)?;
                let method = self.expect_identifier()?;
                self.expect(TokenKind::Comma)?;
                let response = self.expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Mock {
                    service,
                    method,
                    response,
                })
            }
            TokenKind::Wait => {
                // wait(1000)
                self.advance();
                self.expect(TokenKind::LParen)?;
                let value = self.expect_number()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Wait { ms: value as u32 })
            }
            TokenKind::Capture => {
                // capture() or capture("filename")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let filename = if self.check(TokenKind::String) {
                    Some(self.expect_string()?)
                } else {
                    None
                };
                self.expect(TokenKind::RParen)?;
                Ok(TestStatement::Capture { filename })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "test statement".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
    }

    fn test_target(&mut self) -> Result<TestTarget, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::Dollar => {
                // $Store.field (legacy syntax, still supported)
                self.advance();
                let store = self.expect_identifier()?;
                self.expect(TokenKind::Dot)?;
                let field = self.expect_identifier()?;
                Ok(TestTarget::Field { store, field })
            }
            TokenKind::Identifier if token.lexeme == "page" => {
                // page.url
                self.advance();
                self.expect(TokenKind::Dot)?;
                // Allow 'url' keyword as property name
                let prop_token = self.peek().clone();
                let property = if prop_token.kind == TokenKind::Url {
                    self.advance();
                    "url".to_string()
                } else {
                    self.expect_identifier()?
                };
                Ok(TestTarget::PageProperty { property })
            }
            TokenKind::Identifier => {
                // Component.field (e.g., LoginFormCard.email)
                let store = self.expect_identifier()?;
                self.expect(TokenKind::Dot)?;
                let field = self.expect_identifier()?;
                Ok(TestTarget::Field { store, field })
            }
            TokenKind::Text => {
                // text "content"
                self.advance();
                let content = self.expect_string()?;
                Ok(TestTarget::Text { content })
            }
            TokenKind::Submit => {
                self.advance();
                Ok(TestTarget::Submit)
            }
            TokenKind::Button => {
                // button("text")
                self.advance();
                self.expect(TokenKind::LParen)?;
                let content = self.expect_string()?;
                self.expect(TokenKind::RParen)?;
                Ok(TestTarget::Button { content })
            }
            TokenKind::Url => {
                self.advance();
                Ok(TestTarget::Url)
            }
            TokenKind::String => {
                // CSS selector
                let selector = self.expect_string()?;
                Ok(TestTarget::Selector { selector })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "test target".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
    }

    /// Parse assertion: { visible } or "value"
    fn test_assertion(&mut self) -> Result<TestAssertion, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::LBrace => {
                // { visible }, { hidden }, { disabled }, { empty }
                self.advance();
                let condition_token = self.peek().clone();
                let assertion = match condition_token.kind {
                    TokenKind::Visible => {
                        self.advance();
                        TestAssertion::Visible
                    }
                    TokenKind::Hidden => {
                        self.advance();
                        TestAssertion::Hidden
                    }
                    TokenKind::Identifier if condition_token.lexeme == "disabled" => {
                        self.advance();
                        TestAssertion::Disabled
                    }
                    TokenKind::Identifier if condition_token.lexeme == "empty" => {
                        self.advance();
                        TestAssertion::Empty
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "visible, hidden, disabled, or empty".to_string(),
                            found: condition_token.lexeme,
                            line: condition_token.line,
                            column: condition_token.column,
                        });
                    }
                };
                self.expect(TokenKind::RBrace)?;
                Ok(assertion)
            }
            TokenKind::String => {
                // "value"
                let value = self.expect_string()?;
                Ok(TestAssertion::Value { value })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "{ condition } or \"value\"".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
    }

    fn expect_number(&mut self) -> Result<f64, ParseError> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Number {
            self.advance();
            token.lexeme.parse::<f64>().map_err(|_| ParseError::UnexpectedToken {
                expected: "number".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            })
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "number".to_string(),
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
        self.ternary()
    }

    fn ternary(&mut self) -> Result<Expression, ParseError> {
        let condition = self.or_expression()?;

        if self.check(TokenKind::Question) {
            self.advance();
            let then_branch = self.expression()?;
            self.expect(TokenKind::Colon)?;
            let else_branch = self.expression()?;
            return Ok(Expression::Conditional {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            });
        }

        Ok(condition)
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
                let property = self.expect_identifier_or_keyword()?;

                // Check for .for() method - items.for(item => { body }) or items.for((item, index) => { body })
                if property == "for" && self.check(TokenKind::LParen) {
                    self.advance(); // consume '('

                    let (item, index) = if self.check(TokenKind::LParen) {
                        // (item, index) form
                        self.advance(); // consume inner '('
                        let item = self.expect_identifier()?;
                        let index = if self.match_token(TokenKind::Comma) {
                            Some(self.expect_identifier()?)
                        } else {
                            None
                        };
                        self.expect(TokenKind::RParen)?; // consume inner ')'
                        (item, index)
                    } else {
                        // item form (single identifier)
                        let item = self.expect_identifier()?;
                        (item, None)
                    };

                    self.expect(TokenKind::FatArrow)?; // consume '=>'
                    self.expect(TokenKind::LBrace)?;
                    let body = self.expression()?;
                    self.expect(TokenKind::RBrace)?;
                    self.expect(TokenKind::RParen)?; // consume outer ')'

                    expr = Expression::ForIn {
                        item,
                        index,
                        items: Box::new(expr),
                        body: Box::new(body),
                    };
                } else {
                    expr = Expression::MemberAccess {
                        object: Box::new(expr),
                        property,
                    };
                }
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
                    index: None,
                    items: Box::new(items),
                    body: Box::new(body),
                })
            }
            TokenKind::Identifier
            | TokenKind::Type
            | TokenKind::Button
            | TokenKind::Submit
            | TokenKind::Text => {
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
                    // Check for spread operator
                    if self.check(TokenKind::DotDotDot) {
                        self.advance();
                        let expr = self.expression()?;
                        elements.push(Expression::Spread { expr: Box::new(expr) });
                    } else {
                        elements.push(self.expression()?);
                    }
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
                    // Handle optional comma between properties
                    if !self.check(TokenKind::RBrace) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
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
                | TokenKind::Type
                | TokenKind::Text
                | TokenKind::Button
                | TokenKind::Submit
                | TokenKind::Click
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
                | TokenKind::Visible
                | TokenKind::Hidden
                | TokenKind::Url
                | TokenKind::Fill
                | TokenKind::For
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
            assert_eq!(store.name, Some("Counter".to_string()));
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

    #[test]
    fn test_parse_typed_component_params() {
        let source = r#"
            UserCard(name: string, age: number) -> {
                type: div
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            assert_eq!(comp.params.len(), 2);
            assert_eq!(comp.params[0].name, "name");
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(TypeAnnotation::Primitive { name }) if name == "string"
            ));
            assert_eq!(comp.params[1].name, "age");
            assert!(matches!(
                &comp.params[1].type_annotation,
                Some(TypeAnnotation::Primitive { name }) if name == "number"
            ));
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_typed_property() {
        let source = r#"
            Form -> {
                name: string = ""
                count: number = 0
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            assert_eq!(comp.properties.len(), 2);
            assert!(matches!(
                &comp.properties[0].type_annotation,
                Some(TypeAnnotation::Primitive { name }) if name == "string"
            ));
            assert!(matches!(
                &comp.properties[1].type_annotation,
                Some(TypeAnnotation::Primitive { name }) if name == "number"
            ));
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_array_type() {
        let source = r#"
            List -> {
                items: string[] = []
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.properties[0].type_annotation,
                Some(TypeAnnotation::Array { element_type })
                    if matches!(element_type.as_ref(), TypeAnnotation::Primitive { name } if name == "string")
            ));
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_optional_type() {
        let source = r#"
            Form -> {
                name: string? = null
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.properties[0].type_annotation,
                Some(TypeAnnotation::Optional { inner_type })
                    if matches!(inner_type.as_ref(), TypeAnnotation::Primitive { name } if name == "string")
            ));
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_union_type() {
        let source = r#"
            Value -> {
                data: string | number = ""
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            if let Some(TypeAnnotation::Union { types }) = &comp.properties[0].type_annotation {
                assert_eq!(types.len(), 2);
                assert!(matches!(&types[0], TypeAnnotation::Primitive { name } if name == "string"));
                assert!(matches!(&types[1], TypeAnnotation::Primitive { name } if name == "number"));
            } else {
                panic!("Expected union type");
            }
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_reference_type() {
        let source = r#"
            UserCard(user: User) -> {
                type: div
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Component(comp) = &program.declarations[0] {
            assert_eq!(comp.params.len(), 1);
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(TypeAnnotation::Reference { name }) if name == "User"
            ));
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_action_with_typed_params() {
        let source = r#"
            Counter | {
                State { count: 0 }
                Actions {
                    Add(amount: number)
                    SetName(name: string)
                }
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Store(store) = &program.declarations[0] {
            let actions = store.actions.as_ref().unwrap();
            assert_eq!(actions.actions.len(), 2);

            let add_action = &actions.actions[0];
            assert_eq!(add_action.name, "Add");
            assert_eq!(add_action.params.len(), 1);
            assert!(matches!(
                &add_action.params[0].type_annotation,
                Some(TypeAnnotation::Primitive { name }) if name == "number"
            ));
        } else {
            panic!("Expected store declaration");
        }
    }
}
