//! Bytecode Compiler and Virtual Machine
use crate::ast::{Expr, Stmt, Program};
use crate::lexer::TokenKind;
use crate::value::Value;
use crate::environment::Environment;
use crate::string_intern::{StringInterner, InternedString};
use std::rc::Rc;
use std::cell::RefCell;
use indexmap::IndexMap;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum OpCode {
    LoadConstant = 0,
    LoadNil = 1,
    LoadTrue = 2,
    LoadFalse = 3,
    LoadLocal = 4,
    StoreLocal = 5,
    LoadGlobal = 6,
    StoreGlobal = 7,
    Add = 8,
    Subtract = 9,
    Multiply = 10,
    Divide = 11,
    Modulo = 12,
    Negate = 13,
    Equal = 14,
    NotEqual = 15,
    Less = 16,
    Greater = 17,
    LessEqual = 18,
    GreaterEqual = 19,
    And = 20,
    Or = 21,
    Not = 22,
    Jump = 23,
    JumpIfFalse = 24,
    JumpIfTrue = 25,
    JumpBack = 26,
    Call = 27,
    Return = 28,
    ReturnNil = 29,
    NewArray = 30,
    NewTable = 31,
    ArrayPush = 32,
    TableSet = 33,
    IndexGet = 34,
    IndexSet = 35,
    PropertyGet = 36,
    PropertySet = 37,
    Break = 38,
    Continue = 39,
    Line = 40,
    NullCoalesce = 41,
    Lambda = 42,
    NewInstance = 43,
    This = 44,
    Super = 45,
    Spread = 46,
    Match = 47,
    Require = 48,
    Export = 49,
    Set = 50,
    Class = 51,
    NewRange = 52,
    ForInInit = 53,
    ForInNext = 54,
    Length = 55,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    LoadConstant { index: usize },
    LoadNil,
    LoadTrue,
    LoadFalse,
    LoadLocal { slot: usize },
    StoreLocal { slot: usize },
    LoadGlobal { name: InternedString },
    StoreGlobal { name: InternedString },
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Negate,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    And,
    Or,
    Not,
    Jump { offset: usize },
    JumpIfFalse { offset: usize },
    JumpIfTrue { offset: usize },
    JumpBack { offset: usize },
    Call { arg_count: usize },
    Return,
    ReturnNil,
    NewArray { capacity: usize },
    NewTable,
    ArrayPush,
    TableSet { key: InternedString },
    IndexGet,
    IndexSet,
    PropertyGet { name: InternedString },
    PropertySet { name: InternedString },
    Break,
    Continue,
    Line { line_num: usize },
    NullCoalesce,
    Lambda { param_count: usize, func_index: usize },
    NewInstance { class_name: InternedString, arg_count: usize },
    This,
    Super { method: InternedString },
    Spread,
    Match { arm_count: usize },
    Require { path: InternedString },
    Export { name: InternedString },
    Set,
    Class { name: InternedString, method_count: usize },
    NewRange,
    ForInInit,
    ForInNext { var_slot: usize, end_offset: usize },
    Length,
}

#[derive(Clone, Debug)]
pub struct BytecodeFunction {
    pub name: usize,
    pub params: Vec<usize>,
    pub arity: usize,
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub max_slots: usize,
}

#[derive(Clone, Debug)]
pub struct BytecodeProgram {
    pub functions: Vec<BytecodeFunction>,
    pub main: BytecodeFunction,
    pub string_interner: StringInterner,
}

pub struct Compiler {
    functions: Vec<BytecodeFunction>,
    constants: Vec<Value>,
    string_interner: StringInterner,
    locals: IndexMap<usize, usize>,
    slot_count: usize,
    max_slots: usize,
    break_patches: Vec<Vec<usize>>,
    continue_patches: Vec<Vec<usize>>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            constants: Vec::new(),
            string_interner: StringInterner::with_capacity(256),
            locals: IndexMap::new(),
            slot_count: 0,
            max_slots: 0,
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        }
    }

    pub fn compile(mut self, program: Program) -> BytecodeProgram {
        let main_name = 0;
        let main = self.compile_function(main_name, vec![], program.statements);
        BytecodeProgram {
            functions: self.functions,
            main,
            string_interner: self.string_interner,
        }
    }

    fn compile_function(&mut self, name: usize, params: Vec<usize>, statements: Vec<Stmt>) -> BytecodeFunction {
        let old_locals = self.locals.clone();
        let old_slot_count = self.slot_count;
        let old_max_slots = self.max_slots;
        let old_constants = std::mem::take(&mut self.constants);
        let old_break = std::mem::take(&mut self.break_patches);
        let old_continue = std::mem::take(&mut self.continue_patches);

        self.locals.clear();
        self.slot_count = 0;
        self.max_slots = 0;

        for param in &params {
            self.locals.insert(*param, self.slot_count);
            self.slot_count += 1;
        }

        let mut instructions = Vec::new();
        for stmt in statements {
            self.compile_stmt(&mut instructions, stmt);
        }
        instructions.push(Instruction::ReturnNil);

        let arity = params.len();
        let max_slots = self.max_slots;
        let constants = std::mem::replace(&mut self.constants, old_constants);

        self.locals = old_locals;
        self.slot_count = old_slot_count;
        self.max_slots = old_max_slots;
        self.break_patches = old_break;
        self.continue_patches = old_continue;

        BytecodeFunction {
            name,
            params,
            arity,
            instructions,
            constants,
            max_slots,
        }
    }

    fn allocate_slot(&mut self) -> usize {
        let slot = self.slot_count;
        self.slot_count += 1;
        if self.slot_count > self.max_slots {
            self.max_slots = self.slot_count;
        }
        slot
    }

    fn patch_breaks(&mut self, instructions: &mut Vec<Instruction>, target: usize) {
        if let Some(patches) = self.break_patches.pop() {
            for idx in patches {
                if let Instruction::Jump { offset } = &mut instructions[idx] {
                    *offset = target.saturating_sub(idx + 1);
                }
            }
        }
    }

    fn patch_continues(&mut self, instructions: &mut Vec<Instruction>, target: usize) {
        if let Some(patches) = self.continue_patches.pop() {
            for idx in patches {
                if let Instruction::Jump { offset } = &mut instructions[idx] {
                    *offset = target.saturating_sub(idx + 1);
                }
            }
        }
    }

    fn compile_stmt(&mut self, instructions: &mut Vec<Instruction>, stmt: Stmt) {
        match stmt {
            Stmt::Declaration { name, initializer, line, is_const: _ } => {
                if let Some(init) = initializer {
                    self.compile_expr(instructions, init);
                } else {
                    instructions.push(Instruction::LoadNil);
                }
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Assignment { name, value, line, .. } => {
                self.compile_expr(instructions, value);
                if let Some(&slot) = self.locals.get(&name.id()) {
                    instructions.push(Instruction::StoreLocal { slot });
                } else {
                    instructions.push(Instruction::StoreGlobal { name });
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Expression(expr) => {
                self.compile_expr(instructions, expr);
                instructions.push(Instruction::ReturnNil);
            }
            Stmt::If { condition, then_branch, else_branch, line } => {
                self.compile_expr(instructions, condition);
                let else_jump = instructions.len();
                instructions.push(Instruction::JumpIfFalse { offset: 0 });
                for stmt in then_branch {
                    self.compile_stmt(instructions, stmt);
                }
                if let Some(else_stmts) = else_branch {
                    let end_jump = instructions.len();
                    instructions.push(Instruction::Jump { offset: 0 });
                    let else_start = instructions.len();
                    if let Instruction::JumpIfFalse { offset } = &mut instructions[else_jump] {
                        *offset = else_start.saturating_sub(else_jump + 1);
                    }
                    for stmt in else_stmts {
                        self.compile_stmt(instructions, stmt);
                    }
                    let end = instructions.len();
                    if let Instruction::Jump { offset } = &mut instructions[end_jump] {
                        *offset = end.saturating_sub(end_jump + 1);
                    }
                } else {
                    let end = instructions.len();
                    if let Instruction::JumpIfFalse { offset } = &mut instructions[else_jump] {
                        *offset = end.saturating_sub(else_jump + 1);
                    }
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::While { condition, body, line } => {
                self.break_patches.push(Vec::new());
                self.continue_patches.push(Vec::new());

                let loop_start = instructions.len();
                self.compile_expr(instructions, condition);
                let exit_jump = instructions.len();
                instructions.push(Instruction::JumpIfFalse { offset: 0 });

                for stmt in &body {
                    self.compile_stmt(instructions, stmt.clone());
                }
                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start + 1 });

                let loop_end = instructions.len();
                if let Instruction::JumpIfFalse { offset } = &mut instructions[exit_jump] {
                    *offset = loop_end.saturating_sub(exit_jump + 1);
                }

                self.patch_breaks(instructions, loop_end);
                self.patch_continues(instructions, loop_start);
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::For { initializer, condition, increment, body, line } => {
                self.break_patches.push(Vec::new());
                self.continue_patches.push(Vec::new());

                if let Some(init) = initializer {
                    self.compile_expr(instructions, *init);
                    instructions.push(Instruction::ReturnNil);
                }

                let loop_start = instructions.len();
                self.compile_expr(instructions, *condition);
                let exit_jump = instructions.len();
                instructions.push(Instruction::JumpIfFalse { offset: 0 });

                for stmt in &body {
                    self.compile_stmt(instructions, stmt.clone());
                }

                let continue_target = instructions.len();

                if let Some(inc) = increment {
                    self.compile_expr(instructions, *inc);
                    instructions.push(Instruction::ReturnNil);
                }

                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start + 1 });

                let loop_end = instructions.len();
                if let Instruction::JumpIfFalse { offset } = &mut instructions[exit_jump] {
                    *offset = loop_end.saturating_sub(exit_jump + 1);
                }

                self.patch_breaks(instructions, loop_end);
                self.patch_continues(instructions, continue_target);
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::ForIn { variable, iterable, body, line } => {
                self.break_patches.push(Vec::new());
                self.continue_patches.push(Vec::new());

                self.compile_expr(instructions, iterable);
                instructions.push(Instruction::ForInInit);

                let var_slot = self.allocate_slot();
                self.locals.insert(variable.id(), var_slot);

                let loop_start = instructions.len();
                let forin_next_idx = instructions.len();
                instructions.push(Instruction::ForInNext { var_slot, end_offset: 0 });

                for stmt in &body {
                    self.compile_stmt(instructions, stmt.clone());
                }
                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start + 1 });

                let loop_end = instructions.len();
                if let Instruction::ForInNext { end_offset, .. } = &mut instructions[forin_next_idx] {
                    *end_offset = loop_end.saturating_sub(forin_next_idx + 1);
                }

                self.patch_breaks(instructions, loop_end);
                self.patch_continues(instructions, loop_start);
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Function { name, params, body, line } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _): &(InternedString, Option<crate::ast::Expr>)| p.id()).collect();
                let default_params: Vec<Option<crate::ast::Expr>> = params.iter().map(|(_, d): &(InternedString, Option<crate::ast::Expr>)| d.clone()).collect();

                let func = self.compile_function(name.id(), param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);

                let func_value = Value::Function {
                    name: func_index,
                    params: param_ids,
                    default_params,
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::AsyncFunction { name, params, body, line } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _): &(InternedString, Option<crate::ast::Expr>)| p.id()).collect();
                let func = self.compile_function(name.id(), param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);

                let func_value = Value::AsyncFunction {
                    name: func_index,
                    params: param_ids,
                    default_params: params.iter().map(|(_, d): &(InternedString, Option<crate::ast::Expr>)| d.clone()).collect(),
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Generator { name, params, body, line } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _): &(InternedString, Option<crate::ast::Expr>)| p.id()).collect();
                let func = self.compile_function(name.id(), param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);

                let func_value = Value::Generator {
                    name: func_index,
                    params: param_ids,
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                    state: Rc::new(RefCell::new(crate::value::GeneratorState::default())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(instructions, e);
                    instructions.push(Instruction::Return);
                } else {
                    instructions.push(Instruction::ReturnNil);
                }
            }
            Stmt::ReturnMulti(values) => {
                instructions.push(Instruction::NewArray { capacity: values.len() });
                for v in values {
                    self.compile_expr(instructions, v);
                    instructions.push(Instruction::ArrayPush);
                }
                instructions.push(Instruction::Return);
            }
            Stmt::Block(stmts) => {
                for stmt in stmts {
                    self.compile_stmt(instructions, stmt);
                }
            }
            Stmt::Destructure { names, initializer, line } => {
                self.compile_expr(instructions, initializer);
                for (i, name) in names.iter().enumerate() {
                    let slot = self.allocate_slot();
                    self.locals.insert(name.id(), slot);
                    instructions.push(Instruction::LoadLocal { slot: 0 });
                    let idx_const = self.add_constant(Value::Number(i as f64));
                    instructions.push(Instruction::LoadConstant { index: idx_const });
                    instructions.push(Instruction::IndexGet);
                    instructions.push(Instruction::StoreLocal { slot });
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Import { path, alias, line } => {
                let interned = self.string_interner.intern(&path);
                instructions.push(Instruction::LoadGlobal { name: InternedString(interned) });
                if let Some(alias_name) = alias {
                    let slot = self.allocate_slot();
                    self.locals.insert(alias_name.id(), slot);
                    instructions.push(Instruction::StoreLocal { slot });
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Throw { value, line } => {
                self.compile_expr(instructions, value);
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Try { body, catch_var: _, catch_body, line } => {
                for stmt in body {
                    self.compile_stmt(instructions, stmt);
                }
                let end_jump = instructions.len();
                instructions.push(Instruction::Jump { offset: 0 });
                let catch_start = instructions.len();
                if let Instruction::Jump { offset } = &mut instructions[end_jump] {
                    *offset = catch_start.saturating_sub(end_jump + 1);
                }
                for stmt in catch_body {
                    self.compile_stmt(instructions, stmt);
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Break => {
                let idx = instructions.len();
                instructions.push(Instruction::Jump { offset: 0 });
                if let Some(patches) = self.break_patches.last_mut() {
                    patches.push(idx);
                }
            }
            Stmt::Continue => {
                let idx = instructions.len();
                instructions.push(Instruction::Jump { offset: 0 });
                if let Some(patches) = self.continue_patches.last_mut() {
                    patches.push(idx);
                }
            }
            Stmt::Class { name, superclass: _, methods, line } => {
                for (method_name, method_expr) in &methods {
                    let name_const = self.add_constant(Value::Number(method_name.id() as f64));
                    instructions.push(Instruction::LoadConstant { index: name_const });
                    self.compile_expr(instructions, method_expr.clone());
                }
                instructions.push(Instruction::Class { name, method_count: methods.len() });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Export { name, value, line } => {
                self.compile_expr(instructions, value);
                instructions.push(Instruction::Export { name });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Set { object, name, value, line } => {
                self.compile_expr(instructions, *object);
                self.compile_expr(instructions, value);
                instructions.push(Instruction::PropertySet { name });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::NullCoalesceAssign { name, value, line } => {
                if let Some(&slot) = self.locals.get(&name.id()) {
                    instructions.push(Instruction::LoadLocal { slot });
                    instructions.push(Instruction::LoadNil);
                    instructions.push(Instruction::NotEqual);
                    let skip_jump = instructions.len();
                    instructions.push(Instruction::JumpIfTrue { offset: 0 });
                    self.compile_expr(instructions, value);
                    instructions.push(Instruction::StoreLocal { slot });
                    let end = instructions.len();
                    if let Instruction::JumpIfTrue { offset } = &mut instructions[skip_jump] {
                        *offset = end.saturating_sub(skip_jump + 1);
                    }
                }
                instructions.push(Instruction::Line { line_num: line });
            }
        }
    }

    fn compile_expr(&mut self, instructions: &mut Vec<Instruction>, expr: Expr) {
        match expr {
            Expr::Number(n) => {
                let index = self.add_constant(Value::Number(n));
                instructions.push(Instruction::LoadConstant { index });
            }
            Expr::String(s) => {
                let index = self.add_constant(Value::String(s));
                instructions.push(Instruction::LoadConstant { index });
            }
            Expr::LiteralTrue => { instructions.push(Instruction::LoadTrue); }
            Expr::LiteralFalse => { instructions.push(Instruction::LoadFalse); }
            Expr::LiteralNil => { instructions.push(Instruction::LoadNil); }
            Expr::Identifier(name) => {
                if let Some(&slot) = self.locals.get(&name.id()) {
                    instructions.push(Instruction::LoadLocal { slot });
                } else {
                    instructions.push(Instruction::LoadGlobal { name });
                }
            }
            Expr::Binary { left, op, right, line: _ } => {
                self.compile_expr(instructions, *left);
                self.compile_expr(instructions, *right);
                match op {
                    TokenKind::Plus => instructions.push(Instruction::Add),
                    TokenKind::Minus => instructions.push(Instruction::Subtract),
                    TokenKind::Star => instructions.push(Instruction::Multiply),
                    TokenKind::Slash => instructions.push(Instruction::Divide),
                    TokenKind::Percent => instructions.push(Instruction::Modulo),
                    TokenKind::EqualEqual => instructions.push(Instruction::Equal),
                    TokenKind::NotEqual => instructions.push(Instruction::NotEqual),
                    TokenKind::Less => instructions.push(Instruction::Less),
                    TokenKind::Greater => instructions.push(Instruction::Greater),
                    TokenKind::LessEqual => instructions.push(Instruction::LessEqual),
                    TokenKind::GreaterEqual => instructions.push(Instruction::GreaterEqual),
                    TokenKind::And => instructions.push(Instruction::And),
                    TokenKind::Or => instructions.push(Instruction::Or),
                    _ => {}
                }
            }
            Expr::Unary { op, expr, line: _ } => {
                self.compile_expr(instructions, *expr);
                match op {
                    TokenKind::Minus => instructions.push(Instruction::Negate),
                    TokenKind::Not => instructions.push(Instruction::Not),
                    _ => {}
                }
            }
            Expr::Call { callee, arguments, line: _ } => {
                if let Expr::Get { object, name, .. } = &*callee {
                    self.compile_expr(instructions, *object.clone());
                    instructions.push(Instruction::This);
                    for arg in &arguments {
                        self.compile_expr(instructions, arg.clone());
                    }
                    self.compile_expr(instructions, *callee);
                    instructions.push(Instruction::Call { arg_count: arguments.len() + 1 });
                } else {
                    for arg in &arguments {
                        self.compile_expr(instructions, arg.clone());
                    }
                    self.compile_expr(instructions, *callee);
                    instructions.push(Instruction::Call { arg_count: arguments.len() });
                }
            }
            Expr::Table { entries, line: _ } => {
                instructions.push(Instruction::NewTable);
                for (key, value) in entries {
                    self.compile_expr(instructions, value);
                    instructions.push(Instruction::TableSet { key });
                }
            }
            Expr::Array { items, line: _ } => {
                instructions.push(Instruction::NewArray { capacity: items.len() });
                for item in items {
                    self.compile_expr(instructions, item);
                    instructions.push(Instruction::ArrayPush);
                }
            }
            Expr::Index { object, index, line: _ } => {
                self.compile_expr(instructions, *object);
                self.compile_expr(instructions, *index);
                instructions.push(Instruction::IndexGet);
            }
            Expr::SetIndex { object, index, value, line: _ } => {
                self.compile_expr(instructions, *object);
                self.compile_expr(instructions, *index);
                self.compile_expr(instructions, *value);
                instructions.push(Instruction::IndexSet);
            }
            Expr::Get { object, name, line: _ } => {
                self.compile_expr(instructions, *object);
                instructions.push(Instruction::PropertyGet { name });
            }
            Expr::SetProperty { object, name, value, line: _ } => {
                self.compile_expr(instructions, *object);
                self.compile_expr(instructions, *value);
                instructions.push(Instruction::PropertySet { name });
            }
            Expr::SafeGet { object, name, line: _ } => {
                self.compile_expr(instructions, *object);
                instructions.push(Instruction::PropertyGet { name });
            }
            Expr::Range { start, end, line: _ } => {
                self.compile_expr(instructions, *start);
                self.compile_expr(instructions, *end);
                instructions.push(Instruction::NewRange);
            }
            Expr::Length { expr, line: _ } => {
                self.compile_expr(instructions, *expr);
                instructions.push(Instruction::Length);
            }
            Expr::Throw { expr, line: _ } => {
                self.compile_expr(instructions, *expr);
            }
            Expr::FunctionLiteral { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let default_params: Vec<Option<crate::ast::Expr>> = params.iter().map(|(_, d)| d.clone()).collect();
                let func = self.compile_function(0, param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);
                let func_value = Value::Function {
                    name: func_index,
                    params: param_ids,
                    default_params,
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
            }
            Expr::Lambda { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let func = self.compile_function(0, param_ids.clone(), vec![Stmt::Return(Some(*body))]);
                let func_index = self.functions.len();
                self.functions.push(func);
                let func_value = Value::Function {
                    name: func_index,
                    params: param_ids,
                    default_params: vec![],
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
            }
            Expr::Spread { expr, line: _ } => {
                self.compile_expr(instructions, *expr);
                instructions.push(Instruction::Spread);
            }
            Expr::NullCoalesce { left, right, line: _ } => {
                self.compile_expr(instructions, *left);
                self.compile_expr(instructions, *right);
                instructions.push(Instruction::NullCoalesce);
            }
            Expr::Match { value, arms, line: _ } => {
                self.compile_expr(instructions, *value);
                instructions.push(Instruction::Match { arm_count: arms.len() });
            }
            Expr::Require { path, line: _ } => {
                let interned = self.string_interner.intern(&path);
                instructions.push(Instruction::Require { path: InternedString(interned) });
            }
            Expr::Export { name, value, line: _ } => {
                self.compile_expr(instructions, *value);
                instructions.push(Instruction::Export { name });
            }
            Expr::Class { name, superclass: _, methods, line: _ } => {
                for (method_name, method_expr) in &methods {
                    let name_const = self.add_constant(Value::Number(method_name.id() as f64));
                    instructions.push(Instruction::LoadConstant { index: name_const });
                    self.compile_expr(instructions, method_expr.clone());
                }
                instructions.push(Instruction::Class { name, method_count: methods.len() });
            }
            Expr::NewInstance { class_name, arguments, line: _ } => {
                for arg in &arguments {
                    self.compile_expr(instructions, arg.clone());
                }
                instructions.push(Instruction::NewInstance { class_name, arg_count: arguments.len() });
            }
            Expr::FString { parts, line: _ } => {
                let mut result = String::new();
                for (s, expr_opt) in parts {
                    result.push_str(&s);
                    if let Some(expr) = expr_opt {
                        self.compile_expr(instructions, *expr);
                    }
                }
                let index = self.add_constant(Value::String(result));
                instructions.push(Instruction::LoadConstant { index });
            }
            Expr::This { line: _ } => {
                instructions.push(Instruction::This);
            }
            Expr::Super { method, line: _ } => {
                instructions.push(Instruction::Super { method });
            }
            Expr::Set { items, line: _ } => {
                instructions.push(Instruction::NewTable);
                for item in items {
                    self.compile_expr(instructions, item);
                    instructions.push(Instruction::ArrayPush);
                }
                instructions.push(Instruction::Set);
            }
            Expr::AsyncFunctionLiteral { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let func = self.compile_function(0, param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);
                let func_value = Value::AsyncFunction {
                    name: func_index,
                    params: param_ids,
                    default_params: params.iter().map(|(_, d)| d.clone()).collect(),
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
            }
            Expr::GeneratorLiteral { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let func = self.compile_function(0, param_ids.clone(), body);
                let func_index = self.functions.len();
                self.functions.push(func);
                let func_value = Value::Generator {
                    name: func_index,
                    params: param_ids,
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                    state: Rc::new(RefCell::new(crate::value::GeneratorState::default())),
                };
                let const_index = self.add_constant(func_value);
                instructions.push(Instruction::LoadConstant { index: const_index });
            }
            Expr::Await { future, line: _ } => {
                self.compile_expr(instructions, *future);
            }
            Expr::Yield { value, line: _ } => {
                if let Some(v) = value {
                    self.compile_expr(instructions, *v);
                } else {
                    instructions.push(Instruction::LoadNil);
                }
                instructions.push(Instruction::Return);
            }
        }
    }

    fn add_constant(&mut self, value: Value) -> usize {
        let index = self.constants.len();
        self.constants.push(value);
        index
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

enum IteratorState {
    Array { values: Vec<Value>, index: usize },
    Range { current: i64, end: i64 },
    Table { values: Vec<Value>, index: usize },
}

struct CallFrame {
    function: BytecodeFunction,
    ip: usize,
    frame_start: usize,
}

pub struct VirtualMachine {
    stack: Vec<Value>,
    globals: IndexMap<usize, Value>,
    call_frames: Vec<CallFrame>,
    interner: crate::string_intern::StringInterner,
    functions: Vec<BytecodeFunction>,
    this_stack: Vec<Value>,
    iterators: Vec<IteratorState>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            globals: IndexMap::new(),
            call_frames: Vec::new(),
            interner: crate::string_intern::StringInterner::new(),
            functions: Vec::new(),
            this_stack: Vec::new(),
            iterators: Vec::new(),
        }
    }

    pub fn execute(&mut self, program: &BytecodeProgram) -> Result<(), String> {
        self.interner = program.string_interner.clone();
        self.functions = program.functions.clone();

        let main = program.main.clone();
        self.call_frames.push(CallFrame {
            function: main,
            ip: 0,
            frame_start: 0,
        });

        loop {
            if self.call_frames.is_empty() {
                break;
            }

            let instruction = {
                let frame = self.call_frames.last_mut().unwrap();
                if frame.ip >= frame.function.instructions.len() {
                    self.call_frames.pop();
                    if self.call_frames.is_empty() {
                        break;
                    }
                    continue;
                }
                let instr = frame.function.instructions[frame.ip].clone();
                frame.ip += 1;
                instr
            };

            self.execute_instruction(instruction)?;
        }

        Ok(())
    }

    fn execute_instruction(&mut self, instruction: Instruction) -> Result<(), String> {
        match instruction {
            Instruction::LoadConstant { index } => {
                if let Some(frame) = self.call_frames.last() {
                    if index < frame.function.constants.len() {
                        self.stack.push(frame.function.constants[index].clone());
                        return Ok(());
                    }
                }
                self.stack.push(Value::Nil);
            }
            Instruction::LoadNil => { self.stack.push(Value::Nil); }
            Instruction::LoadTrue => { self.stack.push(Value::Bool(true)); }
            Instruction::LoadFalse => { self.stack.push(Value::Bool(false)); }
            Instruction::LoadLocal { slot } => {
                if let Some(frame) = self.call_frames.last() {
                    let local_idx = frame.frame_start + slot;
                    if local_idx < self.stack.len() {
                        self.stack.push(self.stack[local_idx].clone());
                    } else {
                        self.stack.push(Value::Nil);
                    }
                }
            }
            Instruction::StoreLocal { slot } => {
                if let Some(frame) = self.call_frames.last() {
                    let local_idx = frame.frame_start + slot;
                    if let Some(value) = self.stack.pop() {
                        while self.stack.len() <= local_idx {
                            self.stack.push(Value::Nil);
                        }
                        self.stack[local_idx] = value;
                    }
                }
            }
            Instruction::LoadGlobal { name } => {
                if let Some(value) = self.globals.get(&name.id()).cloned() {
                    self.stack.push(value);
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Instruction::StoreGlobal { name } => {
                if let Some(value) = self.stack.pop() {
                    self.globals.insert(name.id(), value);
                }
            }
            Instruction::Add => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l + r)),
                (Value::String(l), Value::String(r)) => Ok(Value::String(format!("{l}{r}"))),
                _ => Err("Invalid operands for +".to_string()),
            })?,
            Instruction::Subtract => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l - r)),
                _ => Err("Invalid operands for -".to_string()),
            })?,
            Instruction::Multiply => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l * r)),
                _ => Err("Invalid operands for *".to_string()),
            })?,
            Instruction::Divide => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => {
                    if r == 0.0 { Err("Division by zero".to_string()) } else { Ok(Value::Number(l / r)) }
                }
                _ => Err("Invalid operands for /".to_string()),
            })?,
            Instruction::Modulo => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => {
                    if r == 0.0 { Err("Modulo by zero".to_string()) } else { Ok(Value::Number(l % r)) }
                }
                _ => Err("Invalid operands for %".to_string()),
            })?,
            Instruction::Negate => {
                if let Some(value) = self.stack.pop() {
                    match value {
                        Value::Number(n) => self.stack.push(Value::Number(-n)),
                        _ => return Err("Cannot negate non-number".to_string()),
                    }
                }
            }
            Instruction::Equal => self.compare_op(|a, b| a == b)?,
            Instruction::NotEqual => self.compare_op(|a, b| a != b)?,
            Instruction::Less => self.compare_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => l < r,
                (Value::String(l), Value::String(r)) => l < r,
                _ => false,
            })?,
            Instruction::Greater => self.compare_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => l > r,
                (Value::String(l), Value::String(r)) => l > r,
                _ => false,
            })?,
            Instruction::LessEqual => self.compare_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => l <= r,
                (Value::String(l), Value::String(r)) => l <= r,
                _ => false,
            })?,
            Instruction::GreaterEqual => self.compare_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => l >= r,
                (Value::String(l), Value::String(r)) => l >= r,
                _ => false,
            })?,
            Instruction::And => {
                if let Some(left) = self.stack.pop() {
                    let result = if left.is_truthy() {
                        self.stack.pop().unwrap_or(Value::Nil)
                    } else {
                        left
                    };
                    self.stack.push(result);
                }
            }
            Instruction::Or => {
                if let Some(left) = self.stack.pop() {
                    let result = if left.is_truthy() {
                        left
                    } else {
                        self.stack.pop().unwrap_or(Value::Nil)
                    };
                    self.stack.push(result);
                }
            }
            Instruction::Not => {
                if let Some(value) = self.stack.pop() {
                    self.stack.push(Value::Bool(!value.is_truthy()));
                }
            }
            Instruction::Jump { offset } => {
                if let Some(frame) = self.call_frames.last_mut() {
                    frame.ip += offset;
                }
            }
            Instruction::JumpIfFalse { offset } => {
                if let Some(value) = self.stack.last() {
                    if !value.is_truthy() {
                        if let Some(frame) = self.call_frames.last_mut() {
                            frame.ip += offset;
                        }
                    }
                }
            }
            Instruction::JumpIfTrue { offset } => {
                if let Some(value) = self.stack.last() {
                    if value.is_truthy() {
                        if let Some(frame) = self.call_frames.last_mut() {
                            frame.ip += offset;
                        }
                    }
                }
            }
            Instruction::JumpBack { offset } => {
                if let Some(frame) = self.call_frames.last_mut() {
                    if frame.ip >= offset {
                        frame.ip -= offset;
                    }
                }
            }
            Instruction::Call { arg_count } => {
                if let Some(callee) = self.stack.pop() {
                    let args: Vec<Value> = self.stack.split_off(self.stack.len() - arg_count);
                    if let Some(result) = self.call_value(callee, args)? {
                        self.stack.push(result);
                    }
                }
            }
            Instruction::Return => {
                let return_value = self.stack.pop().unwrap_or(Value::Nil);
                if let Some(frame) = self.call_frames.pop() {
                    self.stack.truncate(frame.frame_start);
                    self.stack.push(return_value);
                }
            }
            Instruction::ReturnNil => {
                if self.call_frames.len() > 1 {
                    if let Some(frame) = self.call_frames.pop() {
                        self.stack.truncate(frame.frame_start);
                        self.stack.push(Value::Nil);
                    }
                }
            }
            Instruction::NewArray { capacity: _ } => {
                self.stack.push(Value::new_array());
            }
            Instruction::NewTable => {
                self.stack.push(Value::new_table());
            }
            Instruction::ArrayPush => {
                if let (Some(value), Some(array)) = (self.stack.pop(), self.stack.pop()) {
                    if let Value::Array(arr) = array {
                        arr.borrow_mut().push(value);
                        self.stack.push(Value::Array(arr));
                    }
                }
            }
            Instruction::TableSet { key } => {
                if let (Some(value), Some(table)) = (self.stack.pop(), self.stack.pop()) {
                    if let Value::Table(t) = table {
                        t.borrow_mut().insert(key.id(), value);
                        self.stack.push(Value::Table(t));
                    }
                }
            }
            Instruction::IndexGet => {
                if let (Some(index), Some(object)) = (self.stack.pop(), self.stack.pop()) {
                    let result = self.get_index(object, index)?;
                    self.stack.push(result);
                }
            }
            Instruction::IndexSet => {
                if let (Some(value), Some(index), Some(object)) = (self.stack.pop(), self.stack.pop(), self.stack.pop()) {
                    self.set_index(object, index, value)?;
                }
            }
            Instruction::PropertyGet { name } => {
                if let Some(object) = self.stack.pop() {
                    let result = self.get_property(object, name)?;
                    self.stack.push(result);
                }
            }
            Instruction::PropertySet { name } => {
                if let (Some(value), Some(object)) = (self.stack.pop(), self.stack.pop()) {
                    self.set_property(object, name, value)?;
                }
            }
            Instruction::NullCoalesce => {
                if let (Some(right), Some(left)) = (self.stack.pop(), self.stack.pop()) {
                    let result = if left != Value::Nil { left } else { right };
                    self.stack.push(result);
                }
            }
            Instruction::Lambda { param_count: _, func_index } => {
                if func_index < self.functions.len() {
                    let func = self.functions[func_index].clone();
                    let func_value = Value::Function {
                        name: func_index,
                        params: func.params.clone(),
                        default_params: vec![],
                        body: vec![],
                        closure: Rc::new(RefCell::new(Environment::new())),
                    };
                    self.stack.push(func_value);
                }
            }
            Instruction::NewInstance { class_name: _, arg_count } => {
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - arg_count);
                if let Some(class) = self.stack.pop() {
                    let instance = self.instantiate_class(class, args)?;
                    self.stack.push(instance);
                }
            }
            Instruction::This => {
                let this = self.this_stack.last().cloned().unwrap_or(Value::Nil);
                self.stack.push(this);
            }
            Instruction::Super { method } => {
                let this = self.this_stack.last().cloned().unwrap_or(Value::Nil);
                let mut found = false;
                if let Value::Table(t) = &this {
                    let super_key = self.interner.get_id("__superclass__").unwrap_or(usize::MAX);
                    if let Some(superclass) = t.borrow().get(&super_key).cloned() {
                        if let Value::Table(st) = &superclass {
                            if let Some(method_val) = st.borrow().get(&method.id()).cloned() {
                                self.stack.push(method_val);
                                found = true;
                            }
                        }
                    }
                }
                if !found {
                    self.stack.push(Value::Nil);
                }
            }
            Instruction::Spread => {
                if let Some(value) = self.stack.pop() {
                    if let Value::Array(arr) = value {
                        let items = arr.borrow().clone();
                        for item in items {
                            self.stack.push(item);
                        }
                    }
                }
            }
            Instruction::Match { arm_count: _ } => {
                let match_value = self.stack.pop().unwrap_or(Value::Nil);
                self.stack.push(match_value);
            }
            Instruction::Require { path } => {
                self.stack.push(Value::String(format!("<module {:?}>", path)));
            }
            Instruction::Export { name } => {
                if let Some(value) = self.stack.pop() {
                    self.globals.insert(name.id(), value.clone());
                    self.stack.push(value);
                }
            }
            Instruction::Set => {}
            Instruction::Class { name, method_count } => {
                let mut methods = IndexMap::new();
                for _ in 0..method_count {
                    if let Some(method) = self.stack.pop() {
                        if let Some(name_val) = self.stack.pop() {
                            if let Value::Number(name_id) = name_val {
                                methods.insert(name_id as usize, method);
                            }
                        }
                    }
                }
                let class = Value::Table(Rc::new(RefCell::new(methods)));
                self.globals.insert(name.id(), class.clone());
                self.stack.push(class);
            }
            Instruction::NewRange => {
                if let (Some(end_val), Some(start_val)) = (self.stack.pop(), self.stack.pop()) {
                    if let (Value::Number(start), Value::Number(end)) = (start_val, end_val) {
                        self.stack.push(Value::Range { start, end });
                    } else {
                        return Err("Range bounds must be numbers".to_string());
                    }
                }
            }
            Instruction::ForInInit => {
                let iterable = self.stack.pop().unwrap_or(Value::Nil);
                let iter_state = match iterable {
                    Value::Array(arr) => IteratorState::Array {
                        values: arr.borrow().to_vec(),
                        index: 0,
                    },
                    Value::Range { start, end } => IteratorState::Range {
                        current: start as i64,
                        end: end as i64,
                    },
                    Value::Table(t) => {
                        let values: Vec<Value> = t.borrow().values().cloned().collect();
                        IteratorState::Table { values, index: 0 }
                    }
                    _ => return Err("Can only iterate over arrays, ranges, or tables".to_string()),
                };
                self.iterators.push(iter_state);
            }
            Instruction::ForInNext { var_slot, end_offset } => {
                if let Some(iter) = self.iterators.last_mut() {
                    let next_value = match iter {
                        IteratorState::Array { values, index } => {
                            if *index < values.len() {
                                let v = values[*index].clone();
                                *index += 1;
                                Some(v)
                            } else { None }
                        }
                        IteratorState::Range { current, end } => {
                            if *current < *end {
                                let v = Value::Number(*current as f64);
                                *current += 1;
                                Some(v)
                            } else { None }
                        }
                        IteratorState::Table { values, index } => {
                            if *index < values.len() {
                                let v = values[*index].clone();
                                *index += 1;
                                Some(v)
                            } else { None }
                        }
                    };

                    if let Some(value) = next_value {
                        if let Some(frame) = self.call_frames.last() {
                            let local_idx = frame.frame_start + var_slot;
                            while self.stack.len() <= local_idx {
                                self.stack.push(Value::Nil);
                            }
                            self.stack[local_idx] = value;
                        }
                    } else {
                        self.iterators.pop();
                        if let Some(frame) = self.call_frames.last_mut() {
                            frame.ip += end_offset;
                        }
                    }
                }
            }
            Instruction::Length => {
                if let Some(value) = self.stack.pop() {
                    let len = match &value {
                        Value::Array(arr) => arr.borrow().len() as f64,
                        Value::String(s) => s.len() as f64,
                        Value::Table(t) => t.borrow().len() as f64,
                        _ => return Err("Cannot get length".to_string()),
                    };
                    self.stack.push(Value::Number(len));
                }
            }
            Instruction::Break | Instruction::Continue | Instruction::Line { .. } => {}
        }
        Ok(())
    }

    fn binary_op<F>(&mut self, op: F) -> Result<(), String>
    where
        F: Fn(Value, Value) -> Result<Value, String>,
    {
        if let (Some(right), Some(left)) = (self.stack.pop(), self.stack.pop()) {
            let result = op(left, right)?;
            self.stack.push(result);
        }
        Ok(())
    }

    fn compare_op<F>(&mut self, op: F) -> Result<(), String>
    where
        F: Fn(&Value, &Value) -> bool,
    {
        if let (Some(right), Some(left)) = (self.stack.pop(), self.stack.pop()) {
            let result = op(&left, &right);
            self.stack.push(Value::Bool(result));
        }
        Ok(())
    }

    fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Option<Value>, String> {
        match callee {
            Value::NativeFunction(func) => {
                let result = func(&args, &mut self.interner)?;
                Ok(Some(result))
            }
            Value::Function { name, params, default_params, .. } => {
                if name < self.functions.len() {
                    let func = self.functions[name].clone();
                    let frame_start = self.stack.len();
                    for (i, _) in params.iter().enumerate() {
                        let val = if i < args.len() {
                            args[i].clone()
                        } else if i < default_params.len() {
                            if let Some(ref default_expr) = default_params[i] {
                                self.eval_default_param(default_expr)?
                            } else {
                                Value::Nil
                            }
                        } else {
                            Value::Nil
                        };
                        self.stack.push(val);
                    }
                    for _ in params.len()..func.max_slots {
                        self.stack.push(Value::Nil);
                    }
                    self.call_frames.push(CallFrame {
                        function: func,
                        ip: 0,
                        frame_start,
                    });
                    Ok(None)
                } else {
                    Err(format!("Function index {name} out of bounds"))
                }
            }
            Value::AsyncFunction { name, params, .. } => {
                if name < self.functions.len() {
                    let func = self.functions[name].clone();
                    let frame_start = self.stack.len();
                    for (i, _) in params.iter().enumerate() {
                        let val = if i < args.len() { args[i].clone() } else { Value::Nil };
                        self.stack.push(val);
                    }
                    for _ in params.len()..func.max_slots {
                        self.stack.push(Value::Nil);
                    }
                    self.call_frames.push(CallFrame {
                        function: func,
                        ip: 0,
                        frame_start,
                    });
                    Ok(None)
                } else {
                    Err(format!("Function index {name} out of bounds"))
                }
            }
            Value::Generator { name, params, .. } => {
                if name < self.functions.len() {
                    let func = self.functions[name].clone();
                    let frame_start = self.stack.len();
                    for (i, _) in params.iter().enumerate() {
                        let val = if i < args.len() { args[i].clone() } else { Value::Nil };
                        self.stack.push(val);
                    }
                    for _ in params.len()..func.max_slots {
                        self.stack.push(Value::Nil);
                    }
                    self.call_frames.push(CallFrame {
                        function: func,
                        ip: 0,
                        frame_start,
                    });
                    Ok(None)
                } else {
                    Err(format!("Function index {name} out of bounds"))
                }
            }
            _ => Err(format!("Cannot call non-function value: {callee:?}")),
        }
    }

    fn eval_default_param(&mut self, expr: &crate::ast::Expr) -> Result<Value, String> {
        match expr {
            crate::ast::Expr::Number(n) => Ok(Value::Number(*n)),
            crate::ast::Expr::String(s) => Ok(Value::String(s.clone())),
            crate::ast::Expr::LiteralTrue => Ok(Value::Bool(true)),
            crate::ast::Expr::LiteralFalse => Ok(Value::Bool(false)),
            crate::ast::Expr::LiteralNil => Ok(Value::Nil),
            _ => Ok(Value::Nil),
        }
    }

    fn get_index(&self, obj: Value, idx: Value) -> Result<Value, String> {
        match (obj, idx) {
            (Value::Array(arr), Value::Number(i)) => {
                let idx = i as usize;
                let arr_ref = arr.borrow();
                if idx < arr_ref.len() {
                    Ok(arr_ref[idx].clone())
                } else {
                    Ok(Value::Nil)
                }
            }
            (Value::Table(t), Value::Number(i)) => {
                let t_ref = t.borrow();
                Ok(t_ref.get(&(i as usize)).cloned().unwrap_or(Value::Nil))
            }
            (Value::String(s), Value::Number(i)) => {
                let idx = i as usize;
                if idx < s.len() {
                    Ok(Value::String(s[idx..idx + 1].to_string()))
                } else {
                    Ok(Value::Nil)
                }
            }
            _ => Err("Invalid index operation".to_string()),
        }
    }

    fn set_index(&self, obj: Value, idx: Value, val: Value) -> Result<(), String> {
        match (obj, idx) {
            (Value::Array(arr), Value::Number(i)) => {
                let idx = i as usize;
                let mut arr_ref = arr.borrow_mut();
                if idx < arr_ref.len() {
                    arr_ref[idx] = val;
                    Ok(())
                } else {
                    Err(format!("Index {idx} out of bounds"))
                }
            }
            (Value::Table(t), Value::Number(i)) => {
                let mut t_ref = t.borrow_mut();
                t_ref.insert(i as usize, val);
                Ok(())
            }
            _ => Err("Invalid index assignment".to_string()),
        }
    }

    fn get_property(&self, obj: Value, name: InternedString) -> Result<Value, String> {
        match obj {
            Value::Table(t) => {
                let t_ref = t.borrow();
                Ok(t_ref.get(&name.id()).cloned().unwrap_or(Value::Nil))
            }
            _ => Err(format!("Cannot get property '{:?}' on non-table", name)),
        }
    }

    fn set_property(&self, obj: Value, name: InternedString, val: Value) -> Result<(), String> {
        match obj {
            Value::Table(t) => {
                let mut t_ref = t.borrow_mut();
                t_ref.insert(name.id(), val);
                Ok(())
            }
            _ => Err(format!("Cannot set property '{:?}' on non-table", name)),
        }
    }

    fn instantiate_class(&mut self, class: Value, args: Vec<Value>) -> Result<Value, String> {
        match class {
            Value::Table(t) => {
                let methods = t.borrow().clone();
                let instance = Value::Table(Rc::new(RefCell::new(methods)));

                let init_id = self.interner.get_id("init").unwrap_or(usize::MAX);
                if let Some(ctor) = t.borrow().get(&init_id).cloned() {
                    self.this_stack.push(instance.clone());
                    let _ = self.call_value(ctor, args)?;
                    self.this_stack.pop();
                }

                Ok(instance)
            }
            _ => Err("Cannot instantiate non-class".to_string()),
        }
    }
}

impl Default for VirtualMachine {
    fn default() -> Self {
        Self::new()
    }
}