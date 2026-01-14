//! Routes parsing - handles Routes definitions

use crate::ast::{Declaration, ResolverRef, RouteConfig, RouteEntry, RoutesDef, RoutesGuardsConfig};
use crate::lexer::TokenKind;
use crate::parser::{ParseError, Parser};

impl Parser {
    pub(super) fn routes_definition(&mut self) -> Result<Declaration, ParseError> {
        // Expect Routes keyword
        self.expect(TokenKind::Routes)?;

        // Check for optional name: Routes DocsRoutes { } or Routes { }
        let name = if self.check(TokenKind::Identifier) {
            self.expect_identifier()?
        } else {
            "Routes".to_string()
        };

        self.routes_def_with_name(name)
    }

    fn routes_def_with_name(&mut self, name: String) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::LBrace)?;

        let mut routes = Vec::new();
        let mut guards_config = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            // Check for Guards block
            if self.check(TokenKind::Guards) {
                self.advance();
                self.expect(TokenKind::LBrace)?;
                guards_config = Some(self.parse_routes_guards_config()?);
                self.expect(TokenKind::RBrace)?;
                continue;
            }

            // Parse route entry: name: "/path" or name(params): "/path"
            let route_name = self.expect_identifier()?;

            // Parse optional parameters: name(id) or name(projectId, taskId)
            let params = if self.check(TokenKind::LParen) {
                self.advance();
                let mut params = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    params.push(self.expect_identifier()?);
                    if !self.check(TokenKind::RParen) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RParen)?;
                params
            } else {
                Vec::new()
            };

            self.expect(TokenKind::Colon)?;

            // Parse route config: "/path", {"/path", [guards]}, or "/path" -> SubRoute
            let config = self.parse_route_config()?;

            routes.push(RouteEntry {
                name: route_name,
                params,
                config,
            });
        }

        self.expect(TokenKind::RBrace)?;

        Ok(Declaration::Routes(RoutesDef {
            name,
            routes,
            guards: guards_config,
        }))
    }

    fn parse_route_config(&mut self) -> Result<RouteConfig, ParseError> {
        // Simple path: "/path", "/path" -> SubRoute, "/path", [guards], or "/path", {resolvers}
        let path = self.expect_string()?;

        // Check for subroute: -> SubRoute
        if self.check(TokenKind::Arrow) {
            self.advance();
            let route_ref = self.expect_identifier()?;
            return Ok(RouteConfig::SubRoute { path, route_ref });
        }

        // Check for guards and/or resolvers: , [guards], {resolvers}
        if self.check(TokenKind::Comma) {
            self.advance();

            let mut guards = Vec::new();
            let mut can_deactivate = Vec::new();
            let mut resolvers = Vec::new();

            // Parse [guards] if present
            // Guards with ! prefix are canDeactivate guards
            if self.check(TokenKind::LBracket) {
                self.advance();
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    // Check for ! prefix (canDeactivate guard)
                    if self.check(TokenKind::Bang) {
                        self.advance();
                        can_deactivate.push(self.expect_identifier()?);
                    } else {
                        guards.push(self.expect_identifier()?);
                    }
                    if !self.check(TokenKind::RBracket) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RBracket)?;

                // Check for {resolvers} after guards
                if self.check(TokenKind::Comma) {
                    self.advance();
                    if self.check(TokenKind::LBrace) {
                        resolvers = self.parse_resolver_refs()?;
                    }
                }
            }
            // Parse {resolvers} if present (without guards)
            else if self.check(TokenKind::LBrace) {
                resolvers = self.parse_resolver_refs()?;
            }

            // Return appropriate variant based on what was parsed
            let has_guards = !guards.is_empty() || !can_deactivate.is_empty();
            return match (has_guards, resolvers.is_empty()) {
                (true, true) => Ok(RouteConfig::PathWithGuards {
                    path,
                    guards,
                    can_deactivate,
                }),
                (true, false) => Ok(RouteConfig::PathWithGuardsAndResolvers {
                    path,
                    guards,
                    can_deactivate,
                    resolvers,
                }),
                (false, true) => Ok(RouteConfig::Path { path }),
                (false, false) => Ok(RouteConfig::PathWithResolvers { path, resolvers }),
            };
        }

        Ok(RouteConfig::Path { path })
    }

    /// Parse resolver references: {Resolver1, Resolver2(arg)}
    fn parse_resolver_refs(&mut self) -> Result<Vec<ResolverRef>, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut resolvers = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let name = self.expect_identifier()?;

            // Check for arguments: Resolver(arg1, arg2)
            let args = if self.check(TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    args.push(self.expect_identifier()?);
                    if !self.check(TokenKind::RParen) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RParen)?;
                args
            } else {
                Vec::new()
            };

            resolvers.push(ResolverRef { name, args });

            if !self.check(TokenKind::RBrace) {
                let _ = self.match_token(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(resolvers)
    }

    fn parse_routes_guards_config(&mut self) -> Result<RoutesGuardsConfig, ParseError> {
        let mut global = Vec::new();
        let mut skip = Vec::new();

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
            } else if self.check(TokenKind::Skip) {
                self.advance();
                self.expect(TokenKind::Colon)?;
                self.expect(TokenKind::LBracket)?;

                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    skip.push(self.expect_identifier()?);
                    if !self.check(TokenKind::RBracket) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RBracket)?;
            } else {
                // Unknown token, skip it
                self.advance();
            }
        }

        Ok(RoutesGuardsConfig { global, skip })
    }
}
