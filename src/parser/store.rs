//! Store parsing - handles Store definitions including State, Actions, Reducers, Effects

use crate::ast::{
    ActionDef, ActionsBlock, CommandsBlock, EffectHandler, EffectsBlock, Param, ReducerHandler,
    ReducersBlock, SelectorDef, SelectorsBlock, StateBlock, Statement, StoreDef,
};
use crate::lexer::TokenKind;
use crate::parser::{ParseError, Parser};

impl Parser {
    pub(super) fn store_def(&mut self, name: Option<String>) -> Result<StoreDef, ParseError> {
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
            // Handle optional comma between fields (JSON-like syntax)
            if !self.check(TokenKind::RBrace) {
                let _ = self.match_token(TokenKind::Comma);
            }
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
            // Handle optional comma between properties (JSON-like syntax)
            if !self.check(TokenKind::RBrace) {
                let _ = self.match_token(TokenKind::Comma);
            }
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

    pub(super) fn statements(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
        }

        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Statement, ParseError> {
        use crate::ast::Expression;

        if self.check(TokenKind::Try) {
            self.advance();
            return self.try_catch_statement();
        }

        if self.check(TokenKind::Dispatch) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            let first_ident = self.expect_identifier()?;

            // Check for Store.Action pattern
            let (store, action) = if self.check(TokenKind::Dot) {
                self.advance();
                let action_name = self.expect_identifier()?;
                (Some(first_ident), action_name)
            } else {
                (None, first_ident)
            };

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
            return Ok(Statement::Dispatch { store, action, args });
        }

        // Navigate: navigate: "/path" or navigate: expression
        if self.check(TokenKind::Navigate) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            let path = self.expression()?;
            return Ok(Statement::Navigate { path });
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
}
