use crate::ast::{Expr, Stmt, Program};
use crate::lexer::TokenKind;
use std::collections::{HashMap, HashSet};

pub struct Optimizer {
    constants: HashMap<usize, Expr>,
    assigned: HashSet<usize>,
    stats: OptimizerStats,
}

#[derive(Debug, Default)]
pub struct OptimizerStats {
    pub constants_folded: usize,
    pub dead_code_removed: usize,
    pub strength_reduced: usize,
    pub variables_propagated: usize,
}

impl Optimizer {
    pub fn new() -> Self {
        Optimizer {
            constants: HashMap::new(),
            assigned: HashSet::new(),
            stats: OptimizerStats::default(),
        }
    }

    pub fn optimize(&mut self, program: Program) -> Program {
        let mut optimized_stmts = Vec::new();
        for stmt in program.statements {
            if let Some(opt_stmt) = self.optimize_stmt(stmt) {
                optimized_stmts.push(opt_stmt);
            }
        }
        Program { statements: optimized_stmts }
    }

    pub fn stats(&self) -> &OptimizerStats {
        &self.stats
    }

    fn optimize_stmt(&mut self, stmt: Stmt) -> Option<Stmt> {
        match stmt {
            Stmt::Declaration { name, initializer, line, is_const } => {
                let init = initializer.map(|i| self.optimize_expr(i));
                if let Some(ref init_expr) = init {
                    if self.is_constant_expr(init_expr) {
                        self.constants.insert(name.id(), init_expr.clone());
                    }
                }
                self.assigned.insert(name.id());
                Some(Stmt::Declaration { name, initializer: init, line, is_const })
            }
            Stmt::Assignment { name, value, line, is_const_assign } => {
                let opt_value = self.optimize_expr(value);
                self.constants.remove(&name.id());
                if self.is_constant_expr(&opt_value) {
                    self.constants.insert(name.id(), opt_value.clone());
                }
                self.assigned.insert(name.id());
                Some(Stmt::Assignment { name, value: opt_value, line, is_const_assign })
            }
            Stmt::Expression(expr) => {
                Some(Stmt::Expression(self.optimize_expr(expr)))
            }
            Stmt::If { condition, then_branch, else_branch, line } => {
                let opt_cond = self.optimize_expr(condition);
                match opt_cond {
                    Expr::LiteralTrue => {
                        self.stats.dead_code_removed += 1;
                        let optimized: Vec<Stmt> = then_branch.into_iter()
                            .filter_map(|s| self.optimize_stmt(s))
                            .collect();
                        if optimized.len() == 1 {
                            Some(optimized.into_iter().next().unwrap())
                        } else {
                            Some(Stmt::Block(optimized))
                        }
                    }
                    Expr::LiteralFalse => {
                        self.stats.dead_code_removed += 1;
                        match else_branch {
                            Some(branch) => {
                                let optimized: Vec<Stmt> = branch.into_iter()
                                    .filter_map(|s| self.optimize_stmt(s))
                                    .collect();
                                if optimized.len() == 1 {
                                    Some(optimized.into_iter().next().unwrap())
                                } else {
                                    Some(Stmt::Block(optimized))
                                }
                            }
                            None => None,
                        }
                    }
                    _ => {
                        Some(Stmt::If {
                            condition: opt_cond,
                            then_branch: then_branch.into_iter()
                                .filter_map(|s| self.optimize_stmt(s))
                                .collect(),
                            else_branch: else_branch.map(|b| b.into_iter()
                                .filter_map(|s| self.optimize_stmt(s))
                                .collect()),
                            line,
                        })
                    }
                }
            }
            Stmt::While { condition, body, line } => {
                let opt_cond = self.optimize_expr(condition);
                if let Expr::LiteralFalse = opt_cond {
                    self.stats.dead_code_removed += 1;
                    return None;
                }
                Some(Stmt::While {
                    condition: opt_cond,
                    body: body.into_iter()
                        .filter_map(|s| self.optimize_stmt(s))
                        .collect(),
                    line,
                })
            }
            Stmt::For { initializer, condition, increment, body, line } => {
                Some(Stmt::For {
                    initializer: initializer.map(|i| Box::new(self.optimize_expr(*i))),
                    condition: Box::new(self.optimize_expr(*condition)),
                    increment: increment.map(|i| Box::new(self.optimize_expr(*i))),
                    body: body.into_iter()
                        .filter_map(|s| self.optimize_stmt(s))
                        .collect(),
                    line,
                })
            }
            Stmt::ForIn { variable, iterable, body, line } => {
                Some(Stmt::ForIn {
                    variable,
                    iterable: self.optimize_expr(iterable),
                    body: body.into_iter()
                        .filter_map(|s| self.optimize_stmt(s))
                        .collect(),
                    line,
                })
            }
            Stmt::Function { name, params, body, line } => {
                let saved_constants = self.constants.clone();
                let saved_assigned = self.assigned.clone();
                let optimized_body: Vec<Stmt> = body.into_iter()
                    .filter_map(|s| self.optimize_stmt(s))
                    .collect();
                self.constants = saved_constants;
                self.assigned = saved_assigned;
                Some(Stmt::Function { name, params, body: optimized_body, line })
            }
            Stmt::Return(expr) => {
                Some(Stmt::Return(expr.map(|e| self.optimize_expr(e))))
            }
            Stmt::ReturnMulti(values) => {
                Some(Stmt::ReturnMulti(values.into_iter().map(|e| self.optimize_expr(e)).collect()))
            }
            Stmt::Block(stmts) => {
                let optimized: Vec<Stmt> = stmts.into_iter()
                    .filter_map(|s| self.optimize_stmt(s))
                    .collect();
                if optimized.is_empty() {
                    None
                } else if optimized.len() == 1 {
                    Some(optimized.into_iter().next().unwrap())
                } else {
                    Some(Stmt::Block(optimized))
                }
            }
            Stmt::Destructure { names, initializer, line } => {
                Some(Stmt::Destructure {
                    names,
                    initializer: self.optimize_expr(initializer),
                    line,
                })
            }
            Stmt::Throw { value, line } => {
                Some(Stmt::Throw {
                    value: self.optimize_expr(value),
                    line,
                })
            }
            Stmt::Try { body, catch_var, catch_body, line } => {
                Some(Stmt::Try {
                    body: body.into_iter()
                        .filter_map(|s| self.optimize_stmt(s))
                        .collect(),
                    catch_var,
                    catch_body: catch_body.into_iter()
                        .filter_map(|s| self.optimize_stmt(s))
                        .collect(),
                    line,
                })
            }
            Stmt::Set { object, name, value, line } => {
                Some(Stmt::Set {
                    object: Box::new(self.optimize_expr(*object)),
                    name,
                    value: self.optimize_expr(value),
                    line,
                })
            }

            Stmt::Import { .. } | Stmt::Break | Stmt::Continue => Some(stmt),
            _ => Some(stmt),
        }
    }

    fn optimize_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Number(_) | Expr::String(_) | Expr::LiteralTrue |
            Expr::LiteralFalse | Expr::LiteralNil => expr,

            Expr::Identifier(name) => {
                if let Some(const_expr) = self.constants.get(&name.id()) {
                    self.stats.variables_propagated += 1;
                    return const_expr.clone();
                }
                Expr::Identifier(name)
            }

            Expr::Binary { left, op, right, line } => {
                let left_opt = self.optimize_expr(*left);
                let right_opt = self.optimize_expr(*right);

                if let (Expr::Number(l), Expr::Number(r)) = (&left_opt, &right_opt) {
                    let result = match op {
                        TokenKind::Plus => Some(l + r),
                        TokenKind::Minus => Some(l - r),
                        TokenKind::Star => Some(l * r),
                        TokenKind::Slash if *r != 0.0 => Some(l / r),
                        TokenKind::Percent if *r != 0.0 => Some(l % r),
                        TokenKind::EqualEqual => { self.stats.constants_folded += 1; return Expr::LiteralTrue; }
                        TokenKind::NotEqual => { self.stats.constants_folded += 1; return Expr::LiteralFalse; }
                        TokenKind::Less => { self.stats.constants_folded += 1; return if l < r { Expr::LiteralTrue } else { Expr::LiteralFalse }; }
                        TokenKind::Greater => { self.stats.constants_folded += 1; return if l > r { Expr::LiteralTrue } else { Expr::LiteralFalse }; }
                        TokenKind::LessEqual => { self.stats.constants_folded += 1; return if l <= r { Expr::LiteralTrue } else { Expr::LiteralFalse }; }
                        TokenKind::GreaterEqual => { self.stats.constants_folded += 1; return if l >= r { Expr::LiteralTrue } else { Expr::LiteralFalse }; }
                        _ => None,
                    };
                    if let Some(val) = result {
                        self.stats.constants_folded += 1;
                        return Expr::Number(val);
                    }
                }

                if let (Expr::String(l), Expr::String(r)) = (&left_opt, &right_opt) {
                    if op == TokenKind::Plus {
                        self.stats.constants_folded += 1;
                        return Expr::String(format!("{l}{r}"));
                    }
                }

                let result = self.strength_reduce(&left_opt, &op, &right_opt, line);
                if let Some(reduced) = result {
                    self.stats.strength_reduced += 1;
                    return reduced;
                }

                Expr::Binary { left: Box::new(left_opt), op, right: Box::new(right_opt), line }
            }

            Expr::Unary { op, expr, line } => {
                let expr_opt = self.optimize_expr(*expr);
                if let Expr::Number(n) = &expr_opt {
                    if op == TokenKind::Minus {
                        self.stats.constants_folded += 1;
                        return Expr::Number(-n);
                    }
                }
                if let Expr::LiteralTrue = &expr_opt {
                    if op == TokenKind::Not {
                        self.stats.constants_folded += 1;
                        return Expr::LiteralFalse;
                    }
                }
                if let Expr::LiteralFalse = &expr_opt {
                    if op == TokenKind::Not {
                        self.stats.constants_folded += 1;
                        return Expr::LiteralTrue;
                    }
                }
                Expr::Unary { op, expr: Box::new(expr_opt), line }
            }

            Expr::NullCoalesce { left, right, line } => {
                let left_opt = self.optimize_expr(*left);
                match &left_opt {
                    Expr::LiteralNil => {
                        self.stats.dead_code_removed += 1;
                        self.optimize_expr(*right)
                    }
                    Expr::Number(_) | Expr::String(_) | Expr::LiteralTrue | Expr::LiteralFalse => {
                        self.stats.dead_code_removed += 1;
                        left_opt
                    }
                    _ => {
                        let right_opt = self.optimize_expr(*right);
                        Expr::NullCoalesce { left: Box::new(left_opt), right: Box::new(right_opt), line }
                    }
                }
            }

            Expr::Call { callee, arguments, line } => {
                Expr::Call {
                    callee,
                    arguments: arguments.into_iter()
                        .map(|a| self.optimize_expr(a))
                        .collect(),
                    line,
                }
            }

            Expr::Table { entries, line } => {
                Expr::Table {
                    entries: entries.into_iter()
                        .map(|(k, v)| (k, self.optimize_expr(v)))
                        .collect(),
                    line,
                }
            }

            Expr::Array { items, line } => {
                Expr::Array {
                    items: items.into_iter()
                        .map(|i| self.optimize_expr(i))
                        .collect(),
                    line,
                }
            }

            Expr::Index { object, index, line } => {
                Expr::Index {
                    object: Box::new(self.optimize_expr(*object)),
                    index: Box::new(self.optimize_expr(*index)),
                    line,
                }
            }

            Expr::Get { object, name, line } => {
                Expr::Get {
                    object: Box::new(self.optimize_expr(*object)),
                    name,
                    line,
                }
            }

            Expr::Length { expr, line } => {
                let inner = self.optimize_expr(*expr);
                if let Expr::Array { items, .. } = &inner {
                    self.stats.constants_folded += 1;
                    return Expr::Number(items.len() as f64);
                }
                if let Expr::String(s) = &inner {
                    self.stats.constants_folded += 1;
                    return Expr::Number(s.len() as f64);
                }
                Expr::Length { expr: Box::new(inner), line }
            }

            Expr::Range { start, end, line } => {
                Expr::Range {
                    start: Box::new(self.optimize_expr(*start)),
                    end: Box::new(self.optimize_expr(*end)),
                    line,
                }
            }

            _ => expr,
        }
    }

    fn strength_reduce(&self, left: &Expr, op: &TokenKind, right: &Expr, line: usize) -> Option<Expr> {
        match op {
            TokenKind::Star => {
                if let Expr::Number(2.0) = right {
                    return Some(Expr::Binary {
                        left: Box::new(left.clone()),
                        op: TokenKind::Plus,
                        right: Box::new(left.clone()),
                        line,
                    });
                }
                if let Expr::Number(2.0) = left {
                    return Some(Expr::Binary {
                        left: Box::new(right.clone()),
                        op: TokenKind::Plus,
                        right: Box::new(right.clone()),
                        line,
                    });
                }
                if let Expr::Number(1.0) = right {
                    return Some(left.clone());
                }
                if let Expr::Number(1.0) = left {
                    return Some(right.clone());
                }
                if let Expr::Number(0.0) = right {
                    return Some(Expr::Number(0.0));
                }
                if let Expr::Number(0.0) = left {
                    return Some(Expr::Number(0.0));
                }
                None
            }
            TokenKind::Plus => {
                if let Expr::Number(0.0) = right {
                    return Some(left.clone());
                }
                if let Expr::Number(0.0) = left {
                    return Some(right.clone());
                }
                None
            }
            TokenKind::Minus => {
                if let Expr::Number(0.0) = right {
                    return Some(left.clone());
                }
                None
            }
            TokenKind::Slash => {
                if let Expr::Number(1.0) = right {
                    return Some(left.clone());
                }
                None
            }
            _ => None,
        }
    }

    fn is_constant_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Number(_) | Expr::String(_) | Expr::LiteralTrue |
            Expr::LiteralFalse | Expr::LiteralNil => true,
            Expr::Unary { expr, .. } => self.is_constant_expr(expr),
            Expr::Binary { left, right, .. } => {
                self.is_constant_expr(left) && self.is_constant_expr(right)
            }
            _ => false,
        }
    }
}