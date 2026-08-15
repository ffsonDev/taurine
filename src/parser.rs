use crate::lexer::{Token, TokenKind};
use crate::ast::{Expr, Stmt, Program, Pattern, MatchArm};
use crate::string_intern::{StringInterner, InternedString};
use std::sync::OnceLock;

const MAX_PARSER_RECURSION_DEPTH: usize = 500;

static DUMMY_TOKEN: OnceLock<Token> = OnceLock::new();

fn dummy_token() -> &'static Token {
    DUMMY_TOKEN.get_or_init(|| Token {
        kind: TokenKind::Nil,
        lexeme: String::new(),
        line: 0,
    })
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    interner: Option<StringInterner>,
    recursion_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, current: 0, interner: None, recursion_depth: 0 }
    }

    pub fn with_interner(tokens: Vec<Token>, interner: StringInterner) -> Self {
        Parser { tokens, current: 0, interner: Some(interner), recursion_depth: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(Program { statements })
    }

    fn destructure_array(&mut self, line: usize) -> Result<Stmt, String> {
        self.consume(&TokenKind::LBracket, "Expected '[' in array destructuring")?;
        let mut names = Vec::new();
        if !self.check(&TokenKind::RBracket) {
            loop {
                let name = self.consume_identifier()?;
                names.push(name);
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RBracket, "Expected ']' in array destructuring")?;
        self.consume(&TokenKind::Equal, "Expected '=' in destructuring")?;
        let initializer = self.expression()?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Destructure { names, initializer, line })
    }

    fn destructure_table(&mut self, line: usize) -> Result<Stmt, String> {
        self.consume(&TokenKind::LBrace, "Expected '{' in table destructuring")?;
        let mut names = Vec::new();
        if !self.check(&TokenKind::RBrace) {
            loop {
                let name = self.consume_identifier()?;
                names.push(name);
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RBrace, "Expected '}' in table destructuring")?;
        self.consume(&TokenKind::Equal, "Expected '=' in destructuring")?;
        let initializer = self.expression()?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Destructure { names, initializer, line })
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        let line = self.peek().line;
        if self.match_token(&TokenKind::Import) {
            return self.import_statement(line);
        }
        if self.match_token(&TokenKind::Export) {
            return self.export_statement(line);
        }
        if self.match_token(&TokenKind::Class) {
            return self.class_declaration(line);
        }
        if self.match_token(&TokenKind::Async) {
            return self.async_declaration(line);
        }
        if self.match_token(&TokenKind::Const) {
            let name = self.consume_identifier()?;
            self.consume(&TokenKind::Equal, "Expected '=' after const declaration")?;
            let initializer = Some(self.expression()?);
            self.match_token(&TokenKind::Semicolon);
            return Ok(Stmt::Declaration { name, initializer, line, is_const: true });
        }
        if self.match_token(&TokenKind::Let) {
            if self.check(&TokenKind::LBracket) {
                return self.destructure_array(line);
            }
            if self.check(&TokenKind::LBrace) {
                return self.destructure_table(line);
            }
            let name = self.consume_identifier()?;
            let is_const = false;
            let initializer = if self.match_token(&TokenKind::Equal) {
                Some(self.expression()?)
            } else {
                None
            };
            self.match_token(&TokenKind::Semicolon);
            return Ok(Stmt::Declaration { name, initializer, line, is_const });
        }
        if self.match_token(&TokenKind::Function) {
            return self.function_declaration(line);
        }
        if self.match_token(&TokenKind::Try) {
            return self.try_statement(line);
        }
        self.statement()
    }

    fn import_statement(&mut self, line: usize) -> Result<Stmt, String> {
        if !self.match_token(&TokenKind::String) {
            return Err("Expected string path after 'import'".to_string());
        }
        let path = self.previous().lexeme.clone();
        let path = path[1..path.len()-1].to_string();
        let alias = if self.match_token(&TokenKind::As) {
            if !self.match_token(&TokenKind::Identifier) {
                return Err("Expected identifier after 'as'".to_string());
            }
            Some(self.intern(&self.previous().lexeme.clone()))
        } else { None };
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Import { path, alias, line })
    }

    fn export_statement(&mut self, line: usize) -> Result<Stmt, String> {
        let name = self.consume_identifier()?;
        self.consume(&TokenKind::Equal, "Expected '=' after export name")?;
        let value = self.expression()?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Export { name, value, line })
    }

    fn class_declaration(&mut self, line: usize) -> Result<Stmt, String> {
        let name = self.consume_identifier()?;
        let superclass = if self.match_token(&TokenKind::Extends) {
            Some(self.consume_identifier()?)
        } else { None };
        self.consume(&TokenKind::LBrace, "Expected '{' before class body")?;
        let mut methods = Vec::new();
        
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(&TokenKind::Function) {
                let method_name = self.consume_identifier()?;
                self.consume(&TokenKind::LParen, "Expected '(' after method name")?;
                let mut params = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        let param_name = self.consume_identifier()?;
                        let default = if self.match_token(&TokenKind::Equal) {
                            Some(self.expression()?)
                        } else { None };
                        params.push((param_name, default));
                        if !self.match_token(&TokenKind::Comma) { break; }
                    }
                }
                self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;
                self.consume(&TokenKind::LBrace, "Expected '{' before method body")?;
                let mut body = Vec::new();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    body.push(self.declaration()?);
                }
                self.consume(&TokenKind::RBrace, "Expected '}' after method body")?;
                let method_fn = Expr::FunctionLiteral { params, body, line };
                methods.push((method_name, method_fn));
            } else if self.match_token(&TokenKind::Identifier) {
                let field_name = self.intern(&self.previous().lexeme.clone());
                self.consume(&TokenKind::Equal, "Expected '=' after field name")?;
                let value = self.expression()?;
                // Делаем запятую или точку с запятой опциональными
                self.match_token(&TokenKind::Comma);
                self.match_token(&TokenKind::Semicolon);
                methods.push((field_name, value));
            } else {
                self.advance();
            }
        }
        
        self.consume(&TokenKind::RBrace, "Expected '}' after class body")?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Class { name, superclass, methods, line })
    }

    fn variable_declaration(&mut self, line: usize) -> Result<Stmt, String> {
        let is_const = self.previous().kind == TokenKind::Const;
        if self.match_token(&TokenKind::LBrace) {
            let mut names = Vec::new();
            loop {
                names.push(self.consume_identifier()?);
                if !self.match_token(&TokenKind::Comma) { break; }
            }
            self.consume(&TokenKind::RBrace, "Expected '}' after destructuring")?;
            self.consume(&TokenKind::Equal, "Expected '=' after destructuring")?;
            let initializer = self.expression()?;
            self.match_token(&TokenKind::Semicolon);
            return Ok(Stmt::Destructure { names, initializer, line });
        }
        let name = self.consume_identifier()?;
        let initializer = if self.match_token(&TokenKind::Equal) {
            Some(self.expression()?)
        } else { None };
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Declaration { name, initializer, line, is_const })
    }

    fn function_declaration(&mut self, line: usize) -> Result<Stmt, String> {
        let first_name = self.consume_identifier()?;

        // === Поддержка синтаксиса function Namespace.method() ===
        if self.match_token(&TokenKind::Dot) {
            let method_name = self.consume_identifier()?;
            self.consume(&TokenKind::LParen, "Expected '(' after method name")?;
            let mut params = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    let param_name = self.consume_identifier()?;
                    let default = if self.match_token(&TokenKind::Equal) {
                        Some(self.expression()?)
                    } else { None };
                    params.push((param_name, default));
                    if !self.match_token(&TokenKind::Comma) { break; }
                }
            }
            self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;
            self.consume(&TokenKind::LBrace, "Expected '{' before function body")?;
            let body = self.block()?;
            self.consume(&TokenKind::RBrace, "Expected '}' after function body")?;

            let func_expr = Expr::FunctionLiteral {
                params,
                body,
                line,
            };

            let target_obj = Expr::Identifier(first_name);
            return Ok(Stmt::Expression(Expr::SetProperty {
                object: Box::new(target_obj),
                name: method_name,
                value: Box::new(func_expr),
                line,
            }));
        }
        // === КОНЕЦ НОВОГО КОДА ===

        self.consume(&TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.consume_identifier()?;
                let default = if self.match_token(&TokenKind::Equal) {
                    Some(self.expression()?)
                } else { None };
                params.push((param_name, default));
                if !self.match_token(&TokenKind::Comma) { break; }
            }
        }
        self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;
        self.consume(&TokenKind::LBrace, "Expected '{' before function body")?;
        let body = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after function body")?;
        Ok(Stmt::Function { name: first_name, params, body, line })
    }

    fn async_declaration(&mut self, line: usize) -> Result<Stmt, String> {
        if !self.match_token(&TokenKind::Function) {
            return Err("Expected 'function' after 'async'".to_string());
        }
        let name = self.consume_identifier()?;
        self.consume(&TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.consume_identifier()?;
                let default = if self.match_token(&TokenKind::Equal) {
                    Some(self.expression()?)
                } else { None };
                params.push((param_name, default));
                if !self.match_token(&TokenKind::Comma) { break; }
            }
        }
        self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;
        self.consume(&TokenKind::LBrace, "Expected '{' before async function body")?;
        let body = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after async function body")?;
        Ok(Stmt::AsyncFunction { name, params, body, line })
    }

    fn try_statement(&mut self, line: usize) -> Result<Stmt, String> {
        self.consume(&TokenKind::LBrace, "Expected '{' after 'try'")?;
        let body = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after try block")?;
        let mut catch_var = None;
        let mut catch_body = Vec::new();
        if self.match_token(&TokenKind::Catch) {
            if self.match_token(&TokenKind::LParen) {
                if !self.match_token(&TokenKind::Identifier) {
                    return Err("Expected identifier in catch".to_string());
                }
                catch_var = Some(self.intern(&self.previous().lexeme.clone()));
                self.consume(&TokenKind::RParen, "Expected ')' after catch variable")?;
            }
            self.consume(&TokenKind::LBrace, "Expected '{' after catch")?;
            catch_body = self.block()?;
            self.consume(&TokenKind::RBrace, "Expected '}' after catch block")?;
        }
        Ok(Stmt::Try { body, catch_var, catch_body, line })
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        let line = self.peek().line;
        if self.match_token(&TokenKind::If) { return self.if_statement(line); }
        if self.match_token(&TokenKind::While) { return self.while_statement(line); }
        if self.match_token(&TokenKind::For) { return self.for_statement(line); }
        if self.match_token(&TokenKind::Return) { return self.return_statement(line); }
        if self.match_token(&TokenKind::Break) { return Ok(Stmt::Break); }
        if self.match_token(&TokenKind::Continue) { return Ok(Stmt::Continue); }
        if self.match_token(&TokenKind::Throw) { return self.throw_statement(line); }
        if self.check(&TokenKind::LBrace) { return self.block().map(Stmt::Block); }
        if self.check(&TokenKind::Identifier) && self.current + 1 < self.tokens.len() && self.tokens[self.current + 1].kind == TokenKind::Equal {
            return self.assignment_statement(line);
        }
        self.expression_statement()
    }
    
    fn expression_statement(&mut self) -> Result<Stmt, String> {
            let line = self.peek().line;
            let expr = self.expression()?;
            if self.match_token(&TokenKind::Equal) {
                let value = self.expression()?;
                self.match_token(&TokenKind::Semicolon);
                match expr {
                    Expr::Get { object, name, .. } => {
                        return Ok(Stmt::Set { object, name, value, line });
                    }
                    _ => return Err("Invalid assignment target".to_string()),
                }
            }
            self.match_token(&TokenKind::Semicolon);
            Ok(Stmt::Expression(expr))
        }

        fn return_statement(&mut self, _line: usize) -> Result<Stmt, String> {
        if self.match_token(&TokenKind::Semicolon) || self.check(&TokenKind::RBrace) || self.is_at_end() {
            return Ok(Stmt::Return(None));
        }
        let mut values = vec![self.expression()?];
        while self.match_token(&TokenKind::Comma) {
            values.push(self.expression()?);
        }
        self.match_token(&TokenKind::Semicolon);
        if values.len() == 1 {
            Ok(Stmt::Return(Some(values.remove(0))))
        } else {
            Ok(Stmt::ReturnMulti(values))
        }
    }

    fn for_statement(&mut self, line: usize) -> Result<Stmt, String> {
        if self.check(&TokenKind::Identifier) {
            let var_name = self.intern(&self.peek().lexeme.clone());
            let saved_current = self.current;
            self.advance();
            if self.match_token(&TokenKind::In) {
                let iterable = self.expression()?;
                self.consume(&TokenKind::LBrace, "Expected '{' after for-in")?;
                let body = self.block()?;
                self.consume(&TokenKind::RBrace, "Expected '}' after for-in body")?;
                return Ok(Stmt::ForIn { variable: var_name, iterable, body, line });
            } else {
                self.current = saved_current;
            }
        }
        let has_parens = self.match_token(&TokenKind::LParen);
        let initializer = if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) {
            self.advance();
            let name = self.consume_identifier()?;
            self.consume(&TokenKind::Equal, "Expected '=' in for initializer")?;
            let init_expr = self.expression()?;
            Some(Box::new(Expr::Binary { left: Box::new(Expr::Identifier(name)), op: TokenKind::Equal, right: Box::new(init_expr), line }))
        } else if !self.check(&TokenKind::Semicolon) && !self.check(&TokenKind::LBrace) && !self.check(&TokenKind::RParen) {
            Some(Box::new(self.expression()?))
        } else { None };
        let condition;
        let increment;
        if has_parens {
            self.consume(&TokenKind::Semicolon, "Expected ';' after for initializer")?;
            condition = if !self.check(&TokenKind::Semicolon) && !self.check(&TokenKind::RParen) {
                Box::new(self.expression()?)
            } else { Box::new(Expr::LiteralTrue) };
            self.consume(&TokenKind::Semicolon, "Expected ';' after for condition")?;
            increment = if !self.check(&TokenKind::RParen) {
                Some(Box::new(self.expression()?))
            } else { None };
            self.consume(&TokenKind::RParen, "Expected ')' after for increment")?;
        } else {
            condition = if initializer.is_some() {
                self.consume(&TokenKind::Comma, "Expected ',' after for initializer")?;
                Box::new(self.expression()?)
            } else { Box::new(Expr::LiteralTrue) };
            increment = None;
        }
        self.consume(&TokenKind::LBrace, "Expected '{' after for header")?;
        let body = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after for body")?;
        Ok(Stmt::For { initializer, condition, increment, body, line })
    }

    fn throw_statement(&mut self, line: usize) -> Result<Stmt, String> {
        let value = self.expression()?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Throw { value, line })
    }

    fn assignment_statement(&mut self, line: usize) -> Result<Stmt, String> {
        let name = self.consume_identifier()?;
        self.consume(&TokenKind::Equal, "Expected '=' after variable name")?;
        let value = self.expression()?;
        self.match_token(&TokenKind::Semicolon);
        Ok(Stmt::Assignment { name, value, line, is_const_assign: false })
    }

    fn if_statement(&mut self, line: usize) -> Result<Stmt, String> {
        let condition = self.expression()?;
        self.consume(&TokenKind::LBrace, "Expected '{' after if condition")?;
        let then_branch = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after if branch")?;
        let else_branch = if self.match_token(&TokenKind::Else) {
            self.consume(&TokenKind::LBrace, "Expected '{' after else")?;
            let branch = self.block()?;
            self.consume(&TokenKind::RBrace, "Expected '}' after else branch")?;
            Some(branch)
        } else { None };
        Ok(Stmt::If { condition, then_branch, else_branch, line })
    }

    fn while_statement(&mut self, line: usize) -> Result<Stmt, String> {
        let condition = self.expression()?;
        self.consume(&TokenKind::LBrace, "Expected '{' after while condition")?;
        let body = self.block()?;
        self.consume(&TokenKind::RBrace, "Expected '}' after while body")?;
        Ok(Stmt::While { condition, body, line })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(statements)
    }

    fn expression(&mut self) -> Result<Expr, String> {
        self.null_coalesce_expr()
    }

    fn null_coalesce_expr(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.or_expr()?;
        while self.match_token(&TokenKind::NullCoalesce) {
            let right = self.or_expr()?;
            left = Expr::NullCoalesce { left: Box::new(left), right: Box::new(right), line };
        }
        Ok(left)
    }

    fn or_expr(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.and_expr()?;
        while self.match_token(&TokenKind::Or) {
            let right = self.and_expr()?;
            left = Expr::Binary { left: Box::new(left), op: TokenKind::Or, right: Box::new(right), line };
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.equality()?;
        while self.match_token(&TokenKind::And) {
            let right = self.equality()?;
            left = Expr::Binary { left: Box::new(left), op: TokenKind::And, right: Box::new(right), line };
        }
        Ok(left)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.comparison()?;
        while self.match_token(&TokenKind::EqualEqual) || self.match_token(&TokenKind::NotEqual) {
            let op = self.previous().kind.clone();
            let right = self.comparison()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), line };
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.term()?;
        while self.match_token(&TokenKind::Less) || self.match_token(&TokenKind::Greater) || self.match_token(&TokenKind::LessEqual) || self.match_token(&TokenKind::GreaterEqual) {
            let op = self.previous().kind.clone();
            let right = self.term()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), line };
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.factor()?;
        while self.match_token(&TokenKind::Plus) || self.match_token(&TokenKind::Minus) {
            let op = self.previous().kind.clone();
            let right = self.factor()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), line };
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut left = self.unary()?;
        while self.match_token(&TokenKind::Star) || self.match_token(&TokenKind::Slash) || self.match_token(&TokenKind::Percent) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), line };
        }
        if self.match_token(&TokenKind::DotDot) {
            let right = self.unary()?;
            return Ok(Expr::Range { start: Box::new(left), end: Box::new(right), line });
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.recursion_depth >= MAX_PARSER_RECURSION_DEPTH {
            return Err(format!("Parser recursion depth limit exceeded (max: {})", MAX_PARSER_RECURSION_DEPTH));
        }
        let line = self.peek().line;
        if self.match_token(&TokenKind::Minus) || self.match_token(&TokenKind::Not) {
            let op = self.previous().kind.clone();
            self.recursion_depth += 1;
            let result = (|| -> Result<Expr, String> {
                let right = self.unary()?;
                Ok(Expr::Unary { op, expr: Box::new(right), line })
            })();
            self.recursion_depth -= 1;
            return result;
        }
        if self.match_token(&TokenKind::Hash) {
            self.recursion_depth += 1;
            let result = (|| -> Result<Expr, String> {
                let right = self.unary()?;
                Ok(Expr::Length { expr: Box::new(right), line })
            })();
            self.recursion_depth -= 1;
            return result;
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        let mut expr = self.primary()?;
        loop {
            if self.match_token(&TokenKind::LParen) {
                let arguments = self.arguments()?;
                expr = Expr::Call { callee: Box::new(expr), arguments, line };
            } else if self.match_token(&TokenKind::Dot) {
                if !self.match_token(&TokenKind::Identifier) {
                    return Err("Expected property name after '.'".to_string());
                }
                let name = self.intern(&self.previous().lexeme.clone());
                if self.match_token(&TokenKind::Equal) {
                    let value = self.expression()?;
                    expr = Expr::SetProperty { object: Box::new(expr), name, value: Box::new(value), line };
                } else {
                    expr = Expr::Get { object: Box::new(expr), name, line };
                }
            } else if self.match_token(&TokenKind::Colon) {
                if !self.match_token(&TokenKind::Identifier) {
                    return Err("Expected method name after ':'".to_string());
                }
                let method = self.intern(&self.previous().lexeme.clone());
                if self.match_token(&TokenKind::LParen) {
                    let mut arguments = self.arguments()?;
                    arguments.insert(0, expr.clone());
                    expr = Expr::Call { callee: Box::new(Expr::Get { object: Box::new(expr.clone()), name: method, line }), arguments, line };
                } else {
                    return Err("Expected '(' after method name".to_string());
                }
            } else if self.match_token(&TokenKind::QuestionDot) {
                if !self.match_token(&TokenKind::Identifier) {
                    return Err("Expected property name after '?.'".to_string());
                }
                let name = self.intern(&self.previous().lexeme.clone());
                expr = Expr::SafeGet { object: Box::new(expr), name, line };
            } else if self.match_token(&TokenKind::LBracket) {
                let index = self.expression()?;
                self.consume(&TokenKind::RBracket, "Expected ']' after index")?;
                if self.match_token(&TokenKind::Equal) {
                    let value = self.expression()?;
                    expr = Expr::SetIndex { object: Box::new(expr), index: Box::new(index), value: Box::new(value), line };
                } else {
                    expr = Expr::Index { object: Box::new(expr), index: Box::new(index), line };
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            self.advance();
            return Ok(args);
        }
        loop {
            let line = self.peek().line;
            if self.match_token(&TokenKind::DotDotDot) {
                let expr = self.expression()?;
                args.push(Expr::Spread { expr: Box::new(expr), line });
            } else {
                args.push(self.expression()?);
            }
            if !self.match_token(&TokenKind::Comma) { break; }
        }
        if !self.match_token(&TokenKind::RParen) {
            return Err("Expected ')' after arguments".to_string());
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let line = self.peek().line;
        if self.match_token(&TokenKind::True) { return Ok(Expr::LiteralTrue); }
        if self.match_token(&TokenKind::False) { return Ok(Expr::LiteralFalse); }
        if self.match_token(&TokenKind::Nil) { return Ok(Expr::LiteralNil); }
        if self.match_token(&TokenKind::Number) {
            let num: f64 = self.previous().lexeme.parse().map_err(|_| "Invalid number")?;
            return Ok(Expr::Number(num));
        }
        if self.match_token(&TokenKind::FString) { return self.fstring_literal(line); }
        if self.match_token(&TokenKind::String) {
            let s = self.previous().lexeme.clone();
            let raw = &s[1..s.len()-1];
            let mut result = String::new();
            let mut chars = raw.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(&next) = chars.peek() {
                        match next {
                            'n' => { chars.next(); result.push('\n'); }
                            't' => { chars.next(); result.push('\t'); }
                            'r' => { chars.next(); result.push('\r'); }
                            '"' => { chars.next(); result.push('"'); }
                            '\'' => { chars.next(); result.push('\''); }
                            '\\' => { chars.next(); result.push('\\'); }
                            _ => result.push(c),
                        }
                    } else {
                        result.push(c);
                    }
                } else {
                    result.push(c);
                }
            }
            return Ok(Expr::String(result));
        }
        if self.match_token(&TokenKind::Await) {
            while self.check(&TokenKind::Await) {
                self.advance();
            }
            let future = self.primary()?;
            return Ok(Expr::Await { future: Box::new(future), line });
        }
        if self.match_token(&TokenKind::Match) { return self.match_expression(line); }
        if self.match_token(&TokenKind::Require) {
            self.consume(&TokenKind::LParen, "Expected '(' after require")?;
            if !self.match_token(&TokenKind::String) {
                return Err("Expected string path after require".to_string());
            }
            let path = self.previous().lexeme.clone();
            let path = path[1..path.len()-1].to_string();
            self.consume(&TokenKind::RParen, "Expected ')' after require path")?;
            return Ok(Expr::Require { path, line });
        }
        if self.match_token(&TokenKind::LBrace) {
            if self.check_lambda() { return self.lambda_literal(line); }
            return self.table_literal(line);
        }
        if self.match_token(&TokenKind::LBracket) { return self.array_literal(line); }
        if self.check(&TokenKind::LParen) {
            if self.check_lambda() {
                self.advance();
                return self.lambda_literal(line);
            }
            self.advance();
            let expr = self.expression()?;
            self.consume(&TokenKind::RParen, "Expected ')' after expression")?;
            return Ok(expr);
        }
        if self.match_token(&TokenKind::New) {
            let class_name = self.consume_identifier()?;
            self.consume(&TokenKind::LParen, "Expected '(' after class name in new")?;
            let arguments = self.arguments()?;
            return Ok(Expr::NewInstance { class_name, arguments, line });
        }

        if self.match_token(&TokenKind::This) {
            return Ok(Expr::This { line });
        }

        if self.match_token(&TokenKind::Super) {
            self.consume(&TokenKind::Dot, "Expected '.' after super")?;
            let method = self.consume_identifier()?;
            return Ok(Expr::Super { method, line });
        }

        if self.match_token(&TokenKind::DotDotDot) {
            let expr = self.expression()?;
            return Ok(Expr::Spread { expr: Box::new(expr), line });
        }
        if self.match_token(&TokenKind::Identifier) {
            let param_lexeme = self.previous().lexeme.clone();
            if self.match_token(&TokenKind::FatArrow) {
                let param_name = self.intern(&param_lexeme);
                let body = self.expression()?;
                return Ok(Expr::Lambda { params: vec![(param_name, None)], body: Box::new(body), line });
            }
            let lexeme = self.previous().lexeme.clone();
            return Ok(Expr::Identifier(self.intern(&lexeme)));
        }
        Err(format!("Unexpected token: {:?} at line {}", self.peek().kind, self.peek().line))
    }

    fn check_lambda(&mut self) -> bool {
        let start = self.current;
        let mut result = false;
        if self.match_token(&TokenKind::LParen) {
            loop {
                if self.check(&TokenKind::RParen) { break; }
                if self.check(&TokenKind::Identifier) {
                    self.advance();
                    if self.match_token(&TokenKind::Equal) {
                        if !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Comma) { self.advance(); }
                    }
                } else { break; }
                if self.check(&TokenKind::Comma) { self.advance(); } else { break; }
            }
            if self.match_token(&TokenKind::RParen) { result = self.check(&TokenKind::FatArrow); }
        }
        self.current = start;
        result
    }

    fn lambda_literal(&mut self, line: usize) -> Result<Expr, String> {
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.consume_identifier()?;
                let default = if self.match_token(&TokenKind::Equal) {
                    Some(self.expression()?)
                } else { None };
                params.push((param_name, default));
                if !self.match_token(&TokenKind::Comma) { break; }
            }
        }
        self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;
        self.consume(&TokenKind::FatArrow, "Expected '=>' in lambda")?;
        if self.check(&TokenKind::LBrace) {
            let body_stmts = self.block()?;
            let body_expr = if body_stmts.is_empty() {
                Expr::LiteralNil
            } else if let Stmt::Expression(expr) = &body_stmts[0] {
                expr.clone()
            } else {
                return Err("Lambda body must be an expression".to_string());
            };
            return Ok(Expr::Lambda { params, body: Box::new(body_expr), line });
        }
        let body = self.expression()?;
        Ok(Expr::Lambda { params, body: Box::new(body), line })
    }

    fn match_expression(&mut self, line: usize) -> Result<Expr, String> {
        let value = self.expression()?;
        self.consume(&TokenKind::LBrace, "Expected '{' after match value")?;
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let pattern = self.pattern()?;
            let guard = if self.match_token(&TokenKind::If) {
                Some(self.expression()?)
            } else { None };
            self.consume(&TokenKind::FatArrow, "Expected '=>' in match arm")?;
            let body = self.expression()?;
            arms.push(MatchArm { pattern, guard, body });
            self.match_token(&TokenKind::Comma);
        }
        self.consume(&TokenKind::RBrace, "Expected '}' after match arms")?;
        Ok(Expr::Match { value: Box::new(value), arms, line })
    }

    fn pattern(&mut self) -> Result<Pattern, String> {
        let line = self.peek().line;
        if self.match_token(&TokenKind::Underscore) { return Ok(Pattern::Wildcard); }
        if self.match_token(&TokenKind::Identifier) {
            let name = self.intern(&self.previous().lexeme.clone());
            return Ok(Pattern::Identifier(name));
        }
        if self.match_token(&TokenKind::Number) {
            let num: f64 = self.previous().lexeme.parse().map_err(|_| "Invalid number")?;
            return Ok(Pattern::Literal(Expr::Number(num)));
        }
        if self.match_token(&TokenKind::FString) {
            let _ = self.fstring_literal(line);
            return Err("FString not supported in pattern".to_string());
        }
        if self.match_token(&TokenKind::String) {
            let s = self.previous().lexeme.clone();
            return Ok(Pattern::Literal(Expr::String(s[1..s.len()-1].to_string())));
        }
        Err(format!("Unexpected token in pattern: {:?}", self.peek().kind))
    }

    fn table_literal(&mut self, line: usize) -> Result<Expr, String> {
        let mut entries = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if !self.match_token(&TokenKind::Identifier) {
                return Err("Expected identifier in table".to_string());
            }
            let key = self.intern(&self.previous().lexeme.clone());
            if self.match_token(&TokenKind::Equal) || self.match_token(&TokenKind::Colon) {
                let value = self.expression()?;
                entries.push((key, value));
            } else {
                let value = self.expression()?;
                entries.push((key, value));
            }
            self.match_token(&TokenKind::Comma);
        }
        self.consume(&TokenKind::RBrace, "Expected '}' after table entries")?;
        Ok(Expr::Table { entries, line })
    }

    fn array_literal(&mut self, line: usize) -> Result<Expr, String> {
        let mut items = Vec::new();
        while !self.check(&TokenKind::RBracket) && !self.is_at_end() {
            items.push(self.expression()?);
            if !self.match_token(&TokenKind::Comma) { break; }
        }
        self.consume(&TokenKind::RBracket, "Expected ']' after array items")?;
        Ok(Expr::Array { items, line })
    }

    fn intern(&mut self, s: &str) -> InternedString {
        if let Some(ref mut interner) = self.interner {
            InternedString::new(interner.intern(s))
        } else {
            InternedString::new(0)
        }
    }

    fn consume_identifier(&mut self) -> Result<InternedString, String> {
        if !self.match_token(&TokenKind::Identifier) {
            return Err("Expected identifier".to_string());
        }
        let lexeme = self.previous().lexeme.clone();
        Ok(self.intern(&lexeme))
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) { self.advance(); true } else { false }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() { return false; }
        self.peek().kind == *kind
    }

    fn advance(&mut self) { if !self.is_at_end() { self.current += 1; } }

    fn is_at_end(&self) -> bool { self.current >= self.tokens.len() }

    fn peek(&self) -> &Token {
        match self.tokens.get(self.current) {
            Some(t) => t,
            None => dummy_token(),
        }
    }

    fn previous(&self) -> &Token {
        if self.current > 0 {
            match self.tokens.get(self.current - 1) {
                Some(t) => t,
                None => dummy_token(),
            }
        } else {
            dummy_token()
        }
    }

    fn consume(&mut self, kind: &TokenKind, message: &str) -> Result<(), String> {
        if self.match_token(kind) { return Ok(()); }
        Err(format!("{} at line {}", message, self.peek().line))
    }

    pub fn take_interner(&mut self) -> Option<StringInterner> {
        self.interner.take()
    }
}

impl Parser {
    fn fstring_literal(&mut self, line: usize) -> Result<Expr, String> {
        let lexeme = self.previous().lexeme.clone();
        let content = lexeme[2..lexeme.len()-1].to_string();
        let mut parts = Vec::new();
        let mut current_str = String::new();
        let mut chars = content.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                if !current_str.is_empty() {
                    parts.push((current_str.clone(), None));
                    current_str.clear();
                }
                let mut expr_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '}' {
                        chars.next();
                        break;
                    }
                    expr_str.push(chars.next().unwrap());
                }
                let mut interner = self.interner.take().unwrap_or_default();
                let tokens = crate::lexer::tokenize_with_interner(&expr_str, &mut interner);
                let mut expr_parser = Parser::with_interner(tokens, interner);
                let expr = expr_parser.expression()?;
                self.interner = expr_parser.interner;
                parts.push((String::new(), Some(Box::new(expr))));
            } else if c == '\\' {
                if let Some(&next) = chars.peek() {
                    match next {
                        'n' => { chars.next(); current_str.push('\n'); }
                        't' => { chars.next(); current_str.push('\t'); }
                        'r' => { chars.next(); current_str.push('\r'); }
                        '"' => { chars.next(); current_str.push('"'); }
                        '\'' => { chars.next(); current_str.push('\''); }
                        '\\' => { chars.next(); current_str.push('\\'); }
                        _ => current_str.push(c),
                    }
                } else {
                    current_str.push(c);
                }
            } else {
                current_str.push(c);
            }
        }
        if !current_str.is_empty() {
            parts.push((current_str, None));
        }
        Ok(Expr::FString { parts, line })
    }
}