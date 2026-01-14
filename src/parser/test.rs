//! Test parsing - handles Test definitions and test statements

use crate::ast::{Declaration, TestAssertion, TestDef, TestHookDef, TestStatement, TestTarget};
use crate::lexer::TokenKind;
use crate::parser::{ParseError, Parser};

impl Parser {
    pub(super) fn test_definition(&mut self, skip: bool) -> Result<Declaration, ParseError> {
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

    pub(super) fn before_each_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::BeforeEach)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::BeforeEach(TestHookDef { statements }))
    }

    pub(super) fn after_each_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::AfterEach)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::AfterEach(TestHookDef { statements }))
    }

    pub(super) fn before_once_definition(&mut self) -> Result<Declaration, ParseError> {
        self.expect(TokenKind::BeforeOnce)?;
        self.expect(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.test_statement()?);
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Declaration::BeforeOnce(TestHookDef { statements }))
    }

    pub(super) fn after_once_definition(&mut self) -> Result<Declaration, ParseError> {
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

    pub(super) fn expect_number(&mut self) -> Result<f64, ParseError> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Number {
            self.advance();
            token
                .lexeme
                .parse::<f64>()
                .map_err(|_| ParseError::UnexpectedToken {
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
}
