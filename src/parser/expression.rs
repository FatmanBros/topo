//! Expression parsing
//!
//! Parses expressions from token stream into AST nodes.

use crate::ast::*;
use crate::lexer::TokenKind;
use super::{Parser, ParseError};

/// Dedent a multiline string by removing common leading whitespace.
/// This is used for template literals (backtick strings) to allow
/// nicely indented code in source files.
fn dedent(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();

    // Skip first line if empty (common pattern: `\n  content`)
    let start = if lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        1
    } else {
        0
    };

    // Skip last line if empty
    let end = if lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.len().saturating_sub(1)
    } else {
        lines.len()
    };

    if start >= end {
        return s.to_string();
    }

    let content_lines = &lines[start..end];

    // Find minimum indentation (ignoring empty lines)
    let min_indent = content_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    // Remove the common indentation from each line
    content_lines
        .iter()
        .map(|line| {
            if line.len() >= min_indent {
                &line[min_indent..]
            } else {
                line.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expression, ParseError> {
        self.ternary()
    }

    fn ternary(&mut self) -> Result<Expression, ParseError> {
        let condition = self.pipe_expression()?;

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

    /// Parse pipe expressions: `value | pipeName` or `value | pipeName(arg1, arg2)`
    /// Pipes can be chained: `value | pipe1 | pipe2(arg)`
    fn pipe_expression(&mut self) -> Result<Expression, ParseError> {
        let mut left = self.or_expression()?;

        while self.check(TokenKind::Pipe) {
            self.advance();
            // Parse pipe name (identifier)
            let pipe_name = self.expect_identifier()?;

            // Parse optional arguments: pipeName(arg1, arg2)
            let args = if self.check(TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    args.push(self.expression()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                args
            } else {
                Vec::new()
            };

            left = Expression::Pipe {
                value: Box::new(left),
                pipe_name,
                args,
            };
        }

        Ok(left)
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
            // Check recursion for chained unary operators (!!!!!x)
            self.enter_recursion()?;
            let operand = self.unary();
            self.exit_recursion();
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(operand?),
            });
        }

        if self.check(TokenKind::Minus) {
            self.advance();
            // Check recursion for chained unary operators (-----x)
            self.enter_recursion()?;
            let operand = self.unary();
            self.exit_recursion();
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Neg,
                operand: Box::new(operand?),
            });
        }

        if self.check(TokenKind::Await) {
            self.advance();
            // Check recursion for chained await
            self.enter_recursion()?;
            let expr = self.unary();
            self.exit_recursion();
            return Ok(Expression::Await {
                expr: Box::new(expr?),
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
            } else if self.check(TokenKind::LBracket) {
                // Index access: obj[key]
                self.advance();
                let index = self.expression()?;
                self.expect(TokenKind::RBracket)?;
                expr = Expression::IndexAccess {
                    object: Box::new(expr),
                    index: Box::new(index),
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

    pub(super) fn primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::String => {
                self.advance();
                // Remove quotes from the lexeme
                let value = token.lexeme[1..token.lexeme.len() - 1].to_string();
                // Apply dedent for template literals (backtick strings)
                let value = if token.lexeme.starts_with('`') {
                    dedent(&value)
                } else {
                    value
                };
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
            TokenKind::Sql => {
                // sql`SELECT * FROM users WHERE id = ${id}`
                self.advance();
                self.sql_template()
            }
            TokenKind::LBracket => {
                // Array literal - check recursion for nested arrays
                // Check BEFORE advancing to avoid stack buildup
                self.enter_recursion()?;
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
                self.exit_recursion();
                Ok(Expression::Array { elements })
            }
            TokenKind::LBrace => {
                // Object literal - check recursion for nested objects
                // Check BEFORE advancing to avoid stack buildup
                self.enter_recursion()?;
                self.advance();
                let mut members = Vec::new();
                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    // Check for spread operator: ...expr
                    if self.check(TokenKind::DotDotDot) {
                        self.advance();
                        let expr = self.expression()?;
                        members.push(ObjectMember::Spread { expr });
                    } else {
                        members.push(ObjectMember::Property(self.property()?));
                    }
                    // Handle optional comma between members
                    if !self.check(TokenKind::RBrace) {
                        let _ = self.match_token(TokenKind::Comma);
                    }
                }
                self.expect(TokenKind::RBrace)?;
                self.exit_recursion();
                Ok(Expression::Object { members })
            }
            TokenKind::LParen => {
                // Parenthesized expression - check recursion
                // Check BEFORE advancing to avoid stack buildup
                self.enter_recursion()?;
                self.advance();
                let expr = self.expression()?;
                self.expect(TokenKind::RParen)?;
                self.exit_recursion();
                Ok(expr)
            }
            TokenKind::Dot => {
                // Route reference: .home, .docs.installation
                self.advance();
                let mut path = vec![self.expect_identifier()?];
                while self.check(TokenKind::Dot) {
                    self.advance();
                    path.push(self.expect_identifier()?);
                }
                Ok(Expression::RouteRef { path })
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

    /// Parse SQL template literal: sql`SELECT * FROM users WHERE id = ${id}`
    fn sql_template(&mut self) -> Result<Expression, ParseError> {
        // Expect a template string token (backtick string)
        let token = self.peek().clone();
        if token.kind != TokenKind::String {
            return Err(ParseError::UnexpectedToken {
                expected: "template string".to_string(),
                found: token.lexeme,
                line: token.line,
                column: token.column,
            });
        }
        self.advance();

        // Remove backticks from the lexeme
        let template = token.lexeme.trim_matches('`').to_string();

        // Parse template for ${...} interpolations
        let mut parts: Vec<String> = Vec::new();
        let mut expressions: Vec<Expression> = Vec::new();
        let mut current_part = String::new();
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' && chars.peek() == Some(&'{') {
                // Start of interpolation
                chars.next(); // consume '{'
                parts.push(current_part);
                current_part = String::new();

                // Collect expression content until matching '}'
                let mut expr_content = String::new();
                let mut brace_depth = 1;
                while let Some(ec) = chars.next() {
                    if ec == '{' {
                        brace_depth += 1;
                        expr_content.push(ec);
                    } else if ec == '}' {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            break;
                        }
                        expr_content.push(ec);
                    } else {
                        expr_content.push(ec);
                    }
                }

                // Parse the expression content
                // For simplicity, treat single identifiers directly
                let expr = Expression::Identifier { name: expr_content.trim().to_string() };
                expressions.push(expr);
            } else {
                current_part.push(c);
            }
        }

        // Add remaining part
        parts.push(current_part);

        Ok(Expression::SqlTemplate { parts, expressions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedent_basic() {
        let input = r#"
            line1
            line2
        "#;
        let result = dedent(input);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_dedent_nested() {
        let input = r#"
            outer
              inner
            outer2
        "#;
        let result = dedent(input);
        assert_eq!(result, "outer\n  inner\nouter2");
    }

    #[test]
    fn test_dedent_preserves_relative_indent() {
        let input = r#"
            Component -> {
              type: div
              children: [
                Child
              ]
            }
        "#;
        let result = dedent(input);
        assert!(result.starts_with("Component -> {"));
        assert!(result.contains("  type: div"));
    }

    #[test]
    fn test_dedent_single_line() {
        let input = "single line";
        let result = dedent(input);
        assert_eq!(result, "single line");
    }
}
