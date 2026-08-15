// src/formatter.rs — Code formatter for Taurine

use crate::ast::*;
use crate::lexer::TokenKind;
use crate::string_intern::InternedString;

pub struct Formatter {
    indent: usize,
    output: String,
}

impl Formatter {
    pub fn new() -> Self {
        Formatter {
            indent: 0,
            output: String::new(),
        }
    }

    pub fn format(&mut self, program: &Program) -> String {
        self.output.clear();
        self.indent = 0;
        
        for (i, stmt) in program.statements.iter().enumerate() {
            if i > 0 {
                self.output.push('\n');
            }
            self.format_stmt(stmt);
        }
        
        self.output.push('\n');
        self.output.clone()
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }


    fn writeln(&mut self, s: &str) {
        self.output.push_str(&self.indent_str());
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn format_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Declaration { name, initializer, line: _, is_const } => {
                let keyword = if *is_const { "const" } else { "let" };
                if let Some(init) = initializer {
                    self.writeln(&format!("{} {} = {};", keyword, name, self.expr_to_string(init)));
                } else {
                    self.writeln(&format!("{} {};", keyword, name));
                }
            }
            Stmt::Assignment { name, value, line: _, .. } => {
                self.writeln(&format!("{} = {};", name, self.expr_to_string(value)));
            }
            Stmt::Expression(expr) => {
                self.writeln(&format!("{};", self.expr_to_string(expr)));
            }
            Stmt::If { condition, then_branch, else_branch, line: _ } => {
                self.writeln(&format!("if {} {{", self.expr_to_string(condition)));
                self.indent += 1;
                for stmt in then_branch {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                if let Some(else_b) = else_branch {
                    self.writeln("} else {");
                    self.indent += 1;
                    for stmt in else_b {
                        self.format_stmt(stmt);
                    }
                    self.indent -= 1;
                }
                self.writeln("}");
            }
            Stmt::While { condition, body, line: _ } => {
                self.writeln(&format!("while {} {{", self.expr_to_string(condition)));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::For { initializer, condition, increment, body, line: _ } => {
                let init_str = initializer.as_ref().map(|i| self.expr_to_string(i)).unwrap_or_default();
                let cond_str = self.expr_to_string(condition);
                let inc_str = increment.as_ref().map(|i| self.expr_to_string(i)).unwrap_or_default();
                self.writeln(&format!("for ({init_str}; {cond_str}; {inc_str}) {{"));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::ForIn { variable, iterable, body, line: _ } => {
                self.writeln(&format!("for {} in {} {{", variable, self.expr_to_string(iterable)));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::Function { name, params, body, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, d): &(InternedString, Option<Expr>)| {
                        if let Some(default) = d {
                            format!("{} = {}", p, self.expr_to_string(default))
                        } else {
                            p.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.writeln(&format!("function {}({}) {{", name, params_str));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.writeln(&format!("return {};", self.expr_to_string(e)));
                } else {
                    self.writeln("return;");
                }
            }
            Stmt::ReturnMulti(values) => {
                let values_str = values.iter()
                    .map(|v| self.expr_to_string(v))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.writeln(&format!("return {values_str};"));
            }
            Stmt::Block(stmts) => {
                self.writeln("{");
                self.indent += 1;
                for stmt in stmts {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::Import { path, alias, line: _ } => {
                if let Some(a) = alias {
                    self.writeln(&format!("import \"{}\" as {};", path, a));
                } else {
                    self.writeln(&format!("import \"{}\";", path));
                }
            }
            Stmt::Try { body, catch_var, catch_body, line: _ } => {
                self.writeln("try {");
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                if let Some(var) = catch_var {
                    self.writeln(&format!("}} catch ({var}) {{"));
                } else {
                    self.writeln("} catch {");
                }
                self.indent += 1;
                for stmt in catch_body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::Break => self.writeln("break;"),
            Stmt::Continue => self.writeln("continue;"),
            Stmt::Destructure { names, initializer, line: _ } => {
                let names_str = names.iter().map(|n: &InternedString| n.to_string()).collect::<Vec<_>>().join(", ");
                self.writeln(&format!("let {{{names_str}}} = {};", self.expr_to_string(initializer)));
            }
            Stmt::AsyncFunction { name, params, body, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, d)| {
                        if let Some(default) = d {
                            format!("{} = {}", p, self.expr_to_string(default))
                        } else {
                            p.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.writeln(&format!("async function {}({}) {{", name, params_str));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            Stmt::Generator { name, params, body, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, d)| {
                        if let Some(default) = d {
                            format!("{} = {}", p, self.expr_to_string(default))
                        } else {
                            p.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.writeln(&format!("generator {}({}) {{", name, params_str));
                self.indent += 1;
                for stmt in body {
                    self.format_stmt(stmt);
                }
                self.indent -= 1;
                self.writeln("}");
            }
            _ => {}
        }
    }

    fn expr_to_string(&self, expr: &Expr) -> String {
        match expr {
            Expr::Number(n) => format!("{n}"),
            Expr::String(s) => format!("\"{s}\""),
            Expr::Identifier(name) => name.to_string(),
            Expr::LiteralTrue => "true".to_string(),
            Expr::LiteralFalse => "false".to_string(),
            Expr::LiteralNil => "nil".to_string(),
            Expr::Binary { left, op, right, line: _ } => {
                let op_str = match op {
                    TokenKind::Plus => "+",
                    TokenKind::Minus => "-",
                    TokenKind::Star => "*",
                    TokenKind::Slash => "/",
                    TokenKind::EqualEqual => "==",
                    TokenKind::NotEqual => "!=",
                    TokenKind::Less => "<",
                    TokenKind::Greater => ">",
                    TokenKind::LessEqual => "<=",
                    TokenKind::GreaterEqual => ">=",
                    TokenKind::And => "and",
                    TokenKind::Or => "or",
                    _ => "?",
                };
                format!("{} {op_str} {}", self.expr_to_string(left), self.expr_to_string(right))
            }
            Expr::Unary { op, expr, line: _ } => {
                let op_str = match op {
                    TokenKind::Minus => "-",
                    TokenKind::Not => "not ",
                    _ => "?",
                };
                format!("{op_str}{}", self.expr_to_string(expr))
            }
            Expr::Call { callee, arguments, line: _ } => {
                let args = arguments.iter()
                    .map(|a| self.expr_to_string(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({args})", self.expr_to_string(callee))
            }
            Expr::Table { entries, line: _ } => {
                let items = entries.iter()
                    .map(|(k, v)| format!("{k}: {}", self.expr_to_string(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {items} }}")
            }
            Expr::Array { items, line: _ } => {
                let items_str = items.iter()
                    .map(|i| self.expr_to_string(i))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{items_str}]")
            }
            Expr::Index { object, index, line: _ } => {
                format!("{}[{}]", self.expr_to_string(object), self.expr_to_string(index))
            }
            Expr::Length { expr, line: _ } => {
                format!("#{}", self.expr_to_string(expr))
            }
            Expr::Get { object, name, line: _ } => {
                format!("{}.{}", self.expr_to_string(object), name)
            }
            Expr::SafeGet { object, name, line: _ } => {
                format!("{}?.{}", self.expr_to_string(object), name)
            }
            Expr::SetIndex { object, index, value, line: _ } => {
                format!("{}[{}] = {}", self.expr_to_string(object), self.expr_to_string(index), self.expr_to_string(value))
            }
            Expr::Range { start, end, line: _ } => {
                format!("{}..{}", self.expr_to_string(start), self.expr_to_string(end))
            }
            Expr::FString { parts, line: _ } => {
                let mut result = String::from("f\"");
                for (s, expr_opt) in parts {
                    result.push_str(s);
                    if let Some(expr) = expr_opt {
                        result.push('{');
                        result.push_str(&self.expr_to_string(expr));
                        result.push('}');
                    }
                }
                result.push('"');
                result
            }
            Expr::FunctionLiteral { params, body: _, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, _): &(InternedString, Option<Expr>)| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("function({params_str}) {{ ... }}")
            }
            Expr::Set { items, line: _ } => {
                let inner: Vec<String> = items.iter().map(|i| format!("{:?}", i)).collect();
                format!("{{{}}}", inner.join(", "))
            }
            Expr::This { line: _ } => {
                "this".to_string()
            }
            Expr::Super { method, line: _ } => {
                format!("super.{}", method.id())
            }
            Expr::Throw { expr, line: _ } => {
                format!("throw {}", self.expr_to_string(expr))
            }
            Expr::Await { future, line: _ } => {
                format!("await {}", self.expr_to_string(future))
            }
            Expr::Yield { value, line: _ } => {
                if let Some(v) = value {
                    format!("yield {}", self.expr_to_string(v))
                } else {
                    "yield".to_string()
                }
            }
            Expr::AsyncFunctionLiteral { params, body: _, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, _)| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("async function({params_str}) {{ ... }}")
            }
            Expr::GeneratorLiteral { params, body: _, line: _ } => {
                let params_str = params.iter()
                    .map(|(p, _)| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("generator({params_str}) {{ ... }}")
            }
            _ => String::from("<expr>"),
        }
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}
