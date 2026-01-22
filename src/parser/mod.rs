//! Parser for topo language
//!
//! Converts a stream of tokens into an AST.

mod api;
mod expression;
mod routes;
mod store;
mod test;

use crate::ast::*;
use crate::lexer::{Token, TokenKind};
use thiserror::Error;

/// Maximum recursion depth to prevent stack overflow with deeply nested input.
/// This value accounts for the deep call chain in expression parsing
/// (expression -> ternary -> pipe_expression -> ... -> primary).
/// In debug builds, each stack frame is larger, so we use a conservative limit.
const MAX_RECURSION_DEPTH: usize = 64;

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

    #[error("Maximum recursion depth exceeded at line {line}, column {column}")]
    MaxRecursionDepthExceeded { line: usize, column: usize },
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    recursion_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            recursion_depth: 0,
        }
    }

    /// Enter a recursive parsing context. Returns error if max depth exceeded.
    fn enter_recursion(&mut self) -> Result<(), ParseError> {
        self.recursion_depth += 1;
        if self.recursion_depth > MAX_RECURSION_DEPTH {
            let token = self.peek();
            return Err(ParseError::MaxRecursionDepthExceeded {
                line: token.line,
                column: token.column,
            });
        }
        Ok(())
    }

    /// Exit a recursive parsing context.
    fn exit_recursion(&mut self) {
        self.recursion_depth = self.recursion_depth.saturating_sub(1);
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

        // Check for GuardSetup definition
        if self.check(TokenKind::GuardSetup) {
            return self.guard_setup_definition();
        }

        // Check for Routes definition: Routes { ... }
        if self.check(TokenKind::Routes) {
            return self.routes_definition();
        }

        // Check for Schema definition: Schema { ... }
        if self.check(TokenKind::Schema) {
            return self.schema_definition();
        }

        // Check for activate/deactivate guard: activate AuthGuard ? { } or deactivate UnsavedChanges ? { }
        if self.check(TokenKind::Activate) || self.check(TokenKind::Deactivate) {
            let guard_type = if self.check(TokenKind::Activate) {
                self.advance();
                GuardType::Activate
            } else {
                self.advance();
                GuardType::Deactivate
            };
            let name = self.expect_identifier()?;
            self.expect(TokenKind::Question)?;
            return Ok(Declaration::Guard(self.guard_def_with_type(name, guard_type)?));
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
            // Component or Function: Name(params) -> ...
            self.advance();
            self.arrow_def(name, params)
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
        } else if self.check(TokenKind::Question) {
            // Guard: Name ? { }
            self.advance();
            Ok(Declaration::Guard(self.guard_def(name)?))
        } else if self.check(TokenKind::Bang) {
            // Resolver: Name ! { }
            self.advance();
            Ok(Declaration::Resolver(self.resolver_def(name, params)?))
        } else if self.check(TokenKind::At) {
            // Directive: Name @ { }
            self.advance();
            Ok(Declaration::Directive(self.directive_def(name, params)?))
        } else if self.check(TokenKind::DoubleGt) {
            // Animation: Name >> { }
            self.advance();
            Ok(Declaration::Animation(self.animation_def(name)?))
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

    /// Handle arrow definitions: Component, Alias, Guarded Component, or Pure Function
    /// - `Name -> { ... }` - Component
    /// - `Name -> Base(args)` - Alias
    /// - `Name -> Guard1, Guard2 { ... }` - Guarded Component
    /// - `Name -> expression` - Pure Function
    fn arrow_def(&mut self, name: String, params: Vec<TypedParam>) -> Result<Declaration, ParseError> {
        // Case 1: `{ ... }` -> Regular component
        if self.check(TokenKind::LBrace) {
            return Ok(Declaration::Component(self.component_body(name, params, Vec::new())?));
        }

        // Case 2: Starts with identifier -> could be alias, guarded component, or function
        if self.check(TokenKind::Identifier) {
            let peek_next = self.peek_ahead(1);

            // `Identifier(` -> Alias: Name -> Base(args)
            if peek_next.kind == TokenKind::LParen {
                let base = self.expect_identifier()?;
                self.advance(); // consume LParen
                let mut args = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    args.push(self.expression()?);
                    if !self.check(TokenKind::RParen) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RParen)?;
                return Ok(Declaration::Component(ComponentDef {
                    name,
                    params,
                    properties: Vec::new(),
                    init: None,
                    destroy: None,
                    alias: Some(ComponentAlias { base, args }),
                    guards: Vec::new(),
                    directives: Vec::new(),
                }));
            }

            // `Identifier,` or `Identifier {` -> Guarded component: Name -> Guard1, Guard2 { ... }
            if peek_next.kind == TokenKind::Comma || peek_next.kind == TokenKind::LBrace {
                let first_guard = self.expect_identifier()?;
                let mut guards = vec![first_guard];

                // Parse additional guards separated by comma
                while self.check(TokenKind::Comma) {
                    self.advance(); // consume comma
                    guards.push(self.expect_identifier()?);
                }

                // Now parse the component body
                return Ok(Declaration::Component(self.component_body(name, params, guards)?));
            }
        }

        // Case 3: Any other expression -> Pure function: Name -> expression
        let body = self.expression()?;
        Ok(Declaration::Function(FunctionDef { name, params, body }))
    }

    /// Parse component body: { properties... }
    fn component_body(&mut self, name: String, params: Vec<TypedParam>, guards: Vec<String>) -> Result<ComponentDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();
        let mut init = None;
        let mut destroy = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let prop = self.property()?;

            match prop.key.as_str() {
                "init" => init = Some(prop.value),
                "destroy" => destroy = Some(prop.value),
                _ => properties.push(prop),
            }

            // Handle optional comma between properties (JSON-like syntax)
            if !self.check(TokenKind::RBrace) {
                let _ = self.match_token(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ComponentDef { name, params, properties, init, destroy, alias: None, guards, directives: Vec::new() })
    }
    // ========================================================================
    // Theme Definition (*)
    // ========================================================================

    fn theme_def(&mut self, name: String) -> Result<ThemeDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            properties.push(self.property()?);

            // Handle optional comma between properties (JSON-like syntax)
            if !self.check(TokenKind::RBrace) {
                let _ = self.match_token(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(ThemeDef { name, properties })
    }

    // ========================================================================
    // Guard Definition (?)
    // ========================================================================

    fn guard_def(&mut self, name: String) -> Result<GuardDef, ParseError> {
        self.guard_def_with_type(name, GuardType::Activate)
    }

    fn guard_def_with_type(&mut self, name: String, guard_type: GuardType) -> Result<GuardDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut check = None;
        let mut redirect = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let key = self.expect_property_key()?;
            self.expect(TokenKind::Colon)?;

            match key.as_str() {
                "check" => {
                    check = Some(self.expression()?);
                }
                "redirect" => {
                    if let Expression::String { value } = self.expression()? {
                        redirect = Some(value);
                    }
                }
                _ => {
                    // Skip unknown properties
                    let _ = self.expression()?;
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        // Legacy guard style requires both check and redirect
        let check_expr = check.ok_or_else(|| ParseError::UnexpectedToken {
            expected: "check property".to_string(),
            found: "missing".to_string(),
            line: self.peek().line,
            column: self.peek().column,
        })?;

        let redirect_path = redirect.ok_or_else(|| ParseError::UnexpectedToken {
            expected: "redirect property".to_string(),
            found: "missing".to_string(),
            line: self.peek().line,
            column: self.peek().column,
        })?;

        Ok(GuardDef {
            name,
            guard_type,
            body: Vec::new(),
            check: Some(check_expr),
            redirect: Some(redirect_path),
        })
    }

    // ========================================================================
    // Resolver Definition (!)
    // ========================================================================

    fn resolver_def(&mut self, name: String, params: Vec<TypedParam>) -> Result<ResolverDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut fetch = None;
        let mut fallback = None;
        let mut cache = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let key = self.expect_property_key()?;
            self.expect(TokenKind::Colon)?;

            match key.as_str() {
                "fetch" => {
                    fetch = Some(self.expression()?);
                }
                "fallback" => {
                    fallback = Some(self.expression()?);
                }
                "cache" => {
                    if let Expression::Number { value } = self.expression()? {
                        cache = Some(value as u64);
                    }
                }
                _ => {
                    // Skip unknown properties
                    let _ = self.expression()?;
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        let fetch_expr = fetch.ok_or_else(|| ParseError::UnexpectedToken {
            expected: "fetch property".to_string(),
            found: "missing".to_string(),
            line: self.peek().line,
            column: self.peek().column,
        })?;

        let fallback_expr = fallback.ok_or_else(|| ParseError::UnexpectedToken {
            expected: "fallback property".to_string(),
            found: "missing".to_string(),
            line: self.peek().line,
            column: self.peek().column,
        })?;

        Ok(ResolverDef {
            name,
            params: params.into_iter().map(|p| p.name).collect(),
            fetch: fetch_expr,
            fallback: fallback_expr,
            cache,
        })
    }

    // ========================================================================
    // Directive Definition (@)
    // ========================================================================

    fn directive_def(&mut self, name: String, params: Vec<TypedParam>) -> Result<DirectiveDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut on_mount = None;
        let mut on_destroy = None;
        let mut on_update = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let key = self.expect_property_key()?;
            self.expect(TokenKind::Colon)?;

            match key.as_str() {
                "onMount" | "mount" => {
                    on_mount = Some(self.expression()?);
                }
                "onDestroy" | "destroy" => {
                    on_destroy = Some(self.expression()?);
                }
                "onUpdate" | "update" => {
                    on_update = Some(self.expression()?);
                }
                _ => {
                    // Skip unknown properties
                    let _ = self.expression()?;
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(DirectiveDef {
            name,
            params: params.into_iter().map(|p| p.name).collect(),
            on_mount,
            on_destroy,
            on_update,
        })
    }

    fn guard_setup_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::GuardSetup)?;
        self.expect(TokenKind::LBrace)?;

        let mut global = Vec::new();
        let mut routes = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.check(TokenKind::Global) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBracket)?;

                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    global.push(self.expect_identifier()?);
                    if !self.check(TokenKind::RBracket) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }

                self.expect(TokenKind::RBracket)?;
            } else if self.check(TokenKind::Routes) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBrace)?;

                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    // Pattern: "/path/*"
                    let pattern = self.expect_string()?;
                    self.expect(TokenKind::Colon)?;

                    // Guard name or "none"
                    let guard = if self.check(TokenKind::None) {
                        self.advance();
                        None
                    } else {
                        Some(self.expect_identifier()?)
                    };

                    routes.push(RouteGuard { pattern, guard });
                }

                self.expect(TokenKind::RBrace)?;
            } else {
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;

        Ok(Declaration::GuardSetup(GuardSetupDef { global, routes }))
    }

    // ========================================================================
    // Schema Definition
    // ========================================================================

    fn schema_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::Schema)?;
        self.expect(TokenKind::LBrace)?;

        let mut tables = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            tables.push(self.parse_table_def()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(Declaration::Schema(SchemaDef { tables }))
    }

    fn parse_table_def(&mut self) -> Result<TableDef, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(TokenKind::LBrace)?;

        let mut columns = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            columns.push(self.parse_column_def()?);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(TableDef { name, columns, relations: vec![] })
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParseError> {
        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;

        // Parse column type
        let type_token = self.expect_identifier()?;
        let column_type = match type_token.as_str() {
            "string" | "String" | "text" | "Text" => ColumnType::String,
            "number" | "Number" | "int" | "Int" | "integer" | "Integer" => ColumnType::Number,
            "boolean" | "Boolean" | "bool" | "Bool" => ColumnType::Boolean,
            "datetime" | "Datetime" | "timestamp" | "Timestamp" => ColumnType::Datetime,
            "json" | "Json" | "JSON" => ColumnType::Json,
            "blob" | "Blob" | "binary" | "Binary" => ColumnType::Blob,
            _ => {
                return Err(ParseError::UnexpectedToken {
                    expected: "column type (string, number, boolean, datetime, json, blob)".to_string(),
                    found: type_token,
                    line: self.peek().line,
                    column: self.peek().column,
                });
            }
        };

        // Check for nullable (?)
        let nullable = if self.check(TokenKind::Question) {
            self.advance();
            true
        } else {
            false
        };

        // Parse column constraints (@primary, @unique, etc.)
        let mut constraints = Vec::new();
        while self.check(TokenKind::At) {
            self.advance();
            let constraint_name = self.expect_identifier()?;
            let constraint = match constraint_name.as_str() {
                "primary" | "primaryKey" => ColumnConstraint::Primary,
                "unique" => ColumnConstraint::Unique,
                "autoincrement" | "autoIncrement" => ColumnConstraint::AutoIncrement,
                "references" => {
                    // @references(table.column)
                    self.expect(TokenKind::LParen)?;
                    let table = self.expect_identifier()?;
                    self.expect(TokenKind::Dot)?;
                    let column = self.expect_identifier()?;
                    self.expect(TokenKind::RParen)?;
                    ColumnConstraint::References { table, column }
                }
                "default" => {
                    // @default(value)
                    self.expect(TokenKind::LParen)?;
                    let value = self.expression()?;
                    self.expect(TokenKind::RParen)?;
                    ColumnConstraint::Default { value }
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        expected: "column constraint (primary, unique, references, default, autoincrement)".to_string(),
                        found: constraint_name,
                        line: self.peek().line,
                        column: self.peek().column,
                    });
                }
            };
            constraints.push(constraint);
        }

        Ok(ColumnDef {
            name,
            column_type,
            nullable,
            constraints,
        })
    }

    // ========================================================================
    // Animation Definition (>>)
    // ========================================================================

    fn animation_def(&mut self, name: String) -> Result<AnimationDef, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut duration = String::from("300ms");
        let mut easing = None;
        let mut fill = None;
        let mut from_props: Option<Vec<AnimationProperty>> = None;
        let mut to_props: Option<Vec<AnimationProperty>> = None;
        let mut keyframes: Vec<Keyframe> = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let key = self.expect_property_key()?;

            // Check for keyframe percentages (0%, 50%, 100%)
            if key.ends_with('%') || self.check(TokenKind::Percent) {
                // Parse keyframe: `50%: { ... }` or `50 %: { ... }`
                let percent_str = if key.ends_with('%') {
                    key.trim_end_matches('%').to_string()
                } else {
                    self.expect(TokenKind::Percent)?;
                    key
                };

                let percent: u8 = percent_str.parse().map_err(|_| ParseError::UnexpectedToken {
                    expected: "percentage (0-100)".to_string(),
                    found: percent_str.clone(),
                    line: self.peek().line,
                    column: self.peek().column,
                })?;

                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBrace)?;

                let mut kf_props = Vec::new();
                let mut kf_easing = None;

                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    let prop_key = self.expect_property_key()?;
                    self.expect(TokenKind::Colon)?;

                    if prop_key == "easing" {
                        kf_easing = Some(self.expect_identifier_or_string()?);
                    } else {
                        kf_props.push(AnimationProperty {
                            property: prop_key,
                            value: self.expression()?,
                        });
                    }

                    let _ = self.match_token(TokenKind::Comma);
                }

                self.expect(TokenKind::RBrace)?;

                keyframes.push(Keyframe {
                    percent,
                    properties: kf_props,
                    easing: kf_easing,
                });
            } else {
                self.expect(TokenKind::Colon)?;

                match key.as_str() {
                    "duration" => {
                        duration = self.parse_duration()?;
                    }
                    "easing" => {
                        easing = Some(self.expect_identifier_or_string()?);
                    }
                    "fill" => {
                        fill = Some(self.expect_identifier_or_string()?);
                    }
                    "from" => {
                        from_props = Some(self.parse_animation_properties()?);
                    }
                    "to" => {
                        to_props = Some(self.parse_animation_properties()?);
                    }
                    _ => {
                        // Skip unknown properties
                        let _ = self.expression()?;
                    }
                }
            }

            let _ = self.match_token(TokenKind::Comma);
        }

        self.expect(TokenKind::RBrace)?;

        // Determine animation type
        let animation_type = if !keyframes.is_empty() {
            AnimationType::Keyframes { keyframes }
        } else if let (Some(from), Some(to)) = (from_props, to_props) {
            AnimationType::FromTo { from, to }
        } else {
            // Default to empty from/to
            AnimationType::FromTo {
                from: Vec::new(),
                to: Vec::new(),
            }
        };

        Ok(AnimationDef {
            name,
            duration,
            easing,
            animation_type,
            fill,
        })
    }

    /// Parse duration: `300ms`, `1s`, `1.5s`, or a number
    fn parse_duration(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::Number => {
                self.advance();
                let mut duration = token.lexeme;

                // Check for unit suffix: ms or s
                if self.check(TokenKind::Identifier) {
                    let unit = self.peek().lexeme.clone();
                    if unit == "ms" || unit == "s" {
                        self.advance();
                        duration.push_str(&unit);
                    }
                }

                Ok(duration)
            }
            TokenKind::String => {
                self.advance();
                Ok(token.lexeme.trim_matches('"').to_string())
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "duration (e.g., 300ms, 1s)".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
    }

    /// Parse animation properties: `{ opacity: 0, transform: translateY(-10px) }`
    fn parse_animation_properties(&mut self) -> Result<Vec<AnimationProperty>, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut properties = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let prop_key = self.expect_property_key()?;
            self.expect(TokenKind::Colon)?;
            let value = self.expression()?;

            properties.push(AnimationProperty {
                property: prop_key,
                value,
            });

            let _ = self.match_token(TokenKind::Comma);
        }

        self.expect(TokenKind::RBrace)?;

        Ok(properties)
    }

    /// Expect an identifier or a string (for values like "ease-out")
    fn expect_identifier_or_string(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::String => {
                self.advance();
                Ok(token.lexeme.trim_matches('"').to_string())
            }
            _ if Self::is_valid_identifier_token(token.kind) => {
                self.advance();
                // Handle hyphenated identifiers: ease-out, ease-in-out
                let mut result = token.lexeme;
                while self.check(TokenKind::Minus) {
                    self.advance();
                    let next = self.expect_identifier()?;
                    result.push('-');
                    result.push_str(&next);
                }
                Ok(result)
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "identifier or string".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            }),
        }
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
        if name.chars().next().is_some_and(|c| c.is_uppercase()) {
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

    /// Accept identifiers, keywords, numbers, or strings as property keys (JSON-like)
    fn expect_property_key(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();

        // Allow numbers as keys (e.g., 50: "value")
        if token.kind == TokenKind::Number {
            self.advance();
            return Ok(token.lexeme);
        }

        // Allow strings as keys (e.g., "key": "value")
        if token.kind == TokenKind::String {
            self.advance();
            // Remove quotes from the lexeme
            let key = token.lexeme[1..token.lexeme.len() - 1].to_string();
            return Ok(key);
        }

        // Allow identifiers and keywords as property keys
        let is_valid_key = Self::is_valid_identifier_token(token.kind);

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

    /// Check if a token can be used as an identifier (identifier or keyword)
    fn is_valid_identifier_token(kind: TokenKind) -> bool {
        // Reject only structural tokens that would break parsing
        !matches!(
            kind,
            TokenKind::Colon      // key-value separator
                | TokenKind::Comma    // property separator
                | TokenKind::RBrace   // end of object
                | TokenKind::RBracket // end of array
                | TokenKind::RParen   // end of group
                | TokenKind::Eof
        )
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        if Self::is_valid_identifier_token(token.kind) {
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
        self.expect_identifier()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Result<Program, ParseError> {
        let mut lexer = Lexer::new(source).unwrap();
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

    #[test]
    fn test_recursion_depth_limit_nested_parens() {
        // Generate deeply nested parentheses that exceed MAX_RECURSION_DEPTH
        let depth = super::MAX_RECURSION_DEPTH + 10;
        let mut source = String::from("MyComponent -> { value: ");
        source.push_str(&"(".repeat(depth));
        source.push('1');
        source.push_str(&")".repeat(depth));
        source.push_str(" }");

        let result = parse(&source);
        assert!(result.is_err());
        if let Err(ParseError::MaxRecursionDepthExceeded { .. }) = result {
            // Expected error
        } else {
            panic!("Expected MaxRecursionDepthExceeded error, got {:?}", result);
        }
    }

    #[test]
    fn test_recursion_depth_limit_nested_arrays() {
        // Generate deeply nested arrays that exceed MAX_RECURSION_DEPTH
        let depth = super::MAX_RECURSION_DEPTH + 10;
        let mut source = String::from("MyComponent -> { value: ");
        source.push_str(&"[".repeat(depth));
        source.push('1');
        source.push_str(&"]".repeat(depth));
        source.push_str(" }");

        let result = parse(&source);
        assert!(result.is_err());
        if let Err(ParseError::MaxRecursionDepthExceeded { .. }) = result {
            // Expected error
        } else {
            panic!("Expected MaxRecursionDepthExceeded error, got {:?}", result);
        }
    }

    #[test]
    fn test_recursion_depth_limit_nested_objects() {
        // Generate deeply nested objects that exceed MAX_RECURSION_DEPTH
        let depth = super::MAX_RECURSION_DEPTH + 10;
        let mut source = String::from("MyComponent -> { value: ");
        for i in 0..depth {
            source.push_str(&format!("{{ a{}: ", i));
        }
        source.push('1');
        for _ in 0..depth {
            source.push_str(" }");
        }
        source.push_str(" }");

        let result = parse(&source);
        assert!(result.is_err());
        if let Err(ParseError::MaxRecursionDepthExceeded { .. }) = result {
            // Expected error
        } else {
            panic!("Expected MaxRecursionDepthExceeded error, got {:?}", result);
        }
    }

    #[test]
    fn test_moderate_nesting_allowed() {
        // Test that moderate nesting (below limit) is still allowed
        let depth = 50; // Well below MAX_RECURSION_DEPTH
        let mut source = String::from("MyComponent -> { value: ");
        source.push_str(&"(".repeat(depth));
        source.push('1');
        source.push_str(&")".repeat(depth));
        source.push_str(" }");

        let result = parse(&source);
        assert!(result.is_ok(), "Moderate nesting should be allowed");
    }

    #[test]
    fn test_parse_from_to_animation() {
        let source = r#"
            Fade >> {
                duration: 300ms
                easing: ease-out
                from: { opacity: 0 }
                to: { opacity: 1 }
            }
        "#;

        let program = parse(source).unwrap();
        assert_eq!(program.declarations.len(), 1);

        if let Declaration::Animation(anim) = &program.declarations[0] {
            assert_eq!(anim.name, "Fade");
            assert_eq!(anim.duration, "300ms");
            assert_eq!(anim.easing, Some("ease-out".to_string()));
            if let AnimationType::FromTo { from, to } = &anim.animation_type {
                assert_eq!(from.len(), 1);
                assert_eq!(from[0].property, "opacity");
                assert_eq!(to.len(), 1);
                assert_eq!(to[0].property, "opacity");
            } else {
                panic!("Expected FromTo animation type");
            }
        } else {
            panic!("Expected animation declaration");
        }
    }

    #[test]
    fn test_parse_keyframe_animation() {
        let source = r#"
            Bounce >> {
                duration: 500ms
                0%: { transform: "translateY(0)", easing: ease-out }
                50%: { transform: "translateY(-20px)", easing: ease-in }
                100%: { transform: "translateY(0)" }
            }
        "#;

        let program = parse(source).unwrap();
        assert_eq!(program.declarations.len(), 1);

        if let Declaration::Animation(anim) = &program.declarations[0] {
            assert_eq!(anim.name, "Bounce");
            assert_eq!(anim.duration, "500ms");
            if let AnimationType::Keyframes { keyframes } = &anim.animation_type {
                assert_eq!(keyframes.len(), 3);
                assert_eq!(keyframes[0].percent, 0);
                assert_eq!(keyframes[0].easing, Some("ease-out".to_string()));
                assert_eq!(keyframes[1].percent, 50);
                assert_eq!(keyframes[1].easing, Some("ease-in".to_string()));
                assert_eq!(keyframes[2].percent, 100);
                assert_eq!(keyframes[2].easing, None);
            } else {
                panic!("Expected Keyframes animation type");
            }
        } else {
            panic!("Expected animation declaration");
        }
    }

    #[test]
    fn test_parse_animation_with_seconds() {
        let source = r#"
            SlowFade >> {
                duration: 2s
                from: { opacity: 0 }
                to: { opacity: 1 }
            }
        "#;

        let program = parse(source).unwrap();
        if let Declaration::Animation(anim) = &program.declarations[0] {
            assert_eq!(anim.duration, "2s");
        } else {
            panic!("Expected animation declaration");
        }
    }
}
