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
    Lambda { param_count: usize, body_index: usize },
    NewInstance { class_name: InternedString, arg_count: usize },
    This,
    Super { method: InternedString },
    Spread,
    Match { arm_count: usize },
    Require { path: InternedString },
    Export { name: InternedString },
    Set,
    Class { name: InternedString, method_count: usize },
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

        self.locals = old_locals;
        self.slot_count = old_slot_count;
        self.max_slots = old_max_slots;

        let constants = std::mem::replace(&mut self.constants, old_constants);

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
                        *offset = else_start - else_jump - 1;
                    }
                    for stmt in else_stmts {
                        self.compile_stmt(instructions, stmt);
                    }
                    let end = instructions.len();
                    if let Instruction::Jump { offset } = &mut instructions[end_jump] {
                        *offset = end - end_jump - 1;
                    }
                } else {
                    let end = instructions.len();
                    if let Instruction::JumpIfFalse { offset } = &mut instructions[else_jump] {
                        *offset = end - else_jump - 1;
                    }
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::While { condition, body, line } => {
                let loop_start = instructions.len();
                self.compile_expr(instructions, condition);
                let exit_jump = instructions.len();
                instructions.push(Instruction::JumpIfFalse { offset: 0 });
                for stmt in &body {
                    self.compile_stmt(instructions, stmt.clone());
                }
                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start });
                let loop_end = instructions.len();
                if let Instruction::JumpIfFalse { offset } = &mut instructions[exit_jump] {
                    *offset = loop_end - exit_jump - 1;
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::For { initializer, condition, increment, body, line } => {
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
                if let Some(inc) = increment {
                    self.compile_expr(instructions, *inc);
                    instructions.push(Instruction::ReturnNil);
                }
                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start });
                let loop_end = instructions.len();
                if let Instruction::JumpIfFalse { offset } = &mut instructions[exit_jump] {
                    *offset = loop_end - exit_jump - 1;
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::ForIn { variable: _, iterable, body, line } => {
                self.compile_expr(instructions, iterable);
                let loop_start = instructions.len();
                let _iter_slot = self.allocate_slot();
                instructions.push(Instruction::StoreLocal { slot: 0 });
                for stmt in &body {
                    self.compile_stmt(instructions, stmt.clone());
                }
                instructions.push(Instruction::JumpBack { offset: instructions.len() - loop_start });
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Function { name, params, body, line } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _): &(InternedString, Option<crate::ast::Expr>)| p.id()).collect();
                let _func = self.compile_function(name.id(), param_ids.clone(), body);
                let const_index = self.add_constant(Value::Function {
                    name: name.id(),
                    params: param_ids,
                    default_params: vec![],
                    body: vec![],
                    closure: Rc::new(RefCell::new(Environment::new())),
                });
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
                if let Some(first) = values.into_iter().next() {
                    self.compile_expr(instructions, first);
                    instructions.push(Instruction::Return);
                } else {
                    instructions.push(Instruction::ReturnNil);
                }
            }
            Stmt::Block(stmts) => {
                for stmt in stmts {
                    self.compile_stmt(instructions, stmt);
                }
            }
            Stmt::Destructure { names, initializer, line } => {
                self.compile_expr(instructions, initializer);
                if let Some(first) = names.into_iter().next() {
                    let slot = self.allocate_slot();
                    self.locals.insert(first.id(), slot);
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
            Stmt::Throw { .. } => {}
            Stmt::Try { body, catch_var: _, catch_body, line } => {
                for stmt in body {
                    self.compile_stmt(instructions, stmt);
                }
                for stmt in catch_body {
                    self.compile_stmt(instructions, stmt);
                }
                instructions.push(Instruction::Line { line_num: line });
            }
            Stmt::Set { .. } => {}
            Stmt::Break => { instructions.push(Instruction::Break); }
            Stmt::Continue => { instructions.push(Instruction::Continue); }
            Stmt::Class { name, superclass, methods, line } => {
                let name_id = name.id();
                for (_method_name, method_expr) in &methods {
                    self.compile_expr(instructions, method_expr.clone());
                }
                instructions.push(Instruction::Class { name: InternedString(name_id), method_count: methods.len() });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
                let _ = superclass;
            }
            Stmt::Export { name, value, line } => {
                self.compile_expr(instructions, value);
                let name_id = name.id();
                instructions.push(Instruction::Export { name: InternedString(name_id) });
                let slot = self.allocate_slot();
                self.locals.insert(name.id(), slot);
                instructions.push(Instruction::StoreLocal { slot });
                instructions.push(Instruction::Line { line_num: line });
            }
            _ => {}
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
                for arg in &arguments {
                    self.compile_expr(instructions, arg.clone());
                }
                self.compile_expr(instructions, *callee);
                instructions.push(Instruction::Call { arg_count: arguments.len() });
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
            }
            Expr::Length { expr, line: _ } => {
                self.compile_expr(instructions, *expr);
                instructions.push(Instruction::LoadConstant { index: self.add_constant(Value::String("#".to_string())) });
            }
            Expr::Throw { expr, line: _ } => {
                self.compile_expr(instructions, *expr);
            }
            Expr::FunctionLiteral { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let _func = self.compile_function(0, param_ids, body);
            }
            Expr::Lambda { params, body, line: _ } => {
                let param_ids: Vec<usize> = params.iter().map(|(p, _)| p.id()).collect();
                let _func = self.compile_function(0, param_ids, vec![Stmt::Expression(*body)]);
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
                for (_method_name, method_expr) in &methods {
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
                for item in items {
                    self.compile_expr(instructions, item);
                }
            }
            // Async/await/yield - not yet supported in bytecode mode
            Expr::AsyncFunctionLiteral { .. } |
            Expr::GeneratorLiteral { .. } |
            Expr::Await { .. } |
            Expr::Yield { .. } => {
                // Push nil for now
                instructions.push(Instruction::LoadNil);
            }
        }
    }

    fn add_constant(&mut self, value: Value) -> usize {
        let index = self.constants.len();
        self.constants.push(value);
        index
    }
}

pub struct VirtualMachine {
    stack: Vec<Value>,
    globals: IndexMap<usize, Value>,
    call_frames: Vec<CallFrame>,
    interner: crate::string_intern::StringInterner,
}

struct CallFrame {
    function: BytecodeFunction,
    ip: usize,
    frame_start: usize,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            globals: IndexMap::new(),
            call_frames: Vec::new(),
            interner: crate::string_intern::StringInterner::new(),  // ← Добавили
        }
    }

    pub fn execute(&mut self, program: &BytecodeProgram) -> Result<(), String> {
        self.interner = program.string_interner.clone();
        
        let main = program.main.clone();
        self.call_frames.push(CallFrame {
            function: main,
            ip: 0,
            frame_start: 0,
        });
        loop {
            let instruction = if let Some(frame) = self.call_frames.last_mut() {
                if frame.ip >= frame.function.instructions.len() {
                    self.call_frames.pop();
                    if self.call_frames.is_empty() {
                        break;
                    }
                    continue;
                }
                let instruction = frame.function.instructions[frame.ip].clone();
                frame.ip += 1;
                instruction
            } else {
                break;
            };
            self.execute_instruction(instruction)?;
        }
        Ok(())
    }

    fn execute_instruction(&mut self, instruction: Instruction) -> Result<(), String> {
        match instruction {
            Instruction::LoadConstant { index } => {
                // Load constant from function's constant pool
                if let Some(frame) = self.call_frames.last() {
                    if index < frame.function.constants.len() {
                        self.stack.push(frame.function.constants[index].clone());
                    }
                }
            }
            Instruction::LoadNil => {
                self.stack.push(Value::Nil);
            }
            Instruction::LoadTrue => {
                self.stack.push(Value::Bool(true));
            }
            Instruction::LoadFalse => {
                self.stack.push(Value::Bool(false));
            }
            Instruction::LoadLocal { slot } => {
                if let Some(frame) = self.call_frames.last() {
                    let local_idx = frame.frame_start + slot;
                    if local_idx < self.stack.len() {
                        self.stack.push(self.stack[local_idx].clone());
                    }
                }
            }
            Instruction::StoreLocal { slot } => {
                if let Some(frame) = self.call_frames.last() {
                    let local_idx = frame.frame_start + slot;
                    while self.stack.len() <= local_idx {
                        self.stack.push(Value::Nil);
                    }
                    if let Some(value) = self.stack.pop() {
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
            Instruction::Negate => {
                if let Some(value) = self.stack.pop() {
                    match value {
                        Value::Number(n) => self.stack.push(Value::Number(-n)),
                        _ => return Err("Cannot negate non-number".to_string()),
                    }
                }
            }
            Instruction::Modulo => self.binary_op(|a, b| match (a, b) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l % r)),
                _ => Err("Invalid operands for %".to_string()),
            })?,
            Instruction::Jump { offset } => {
                if let Some(frame) = self.call_frames.last_mut() {
                    frame.ip = offset;
                }
            }
            Instruction::JumpIfFalse { offset } => {
                if let Some(value) = self.stack.last() {
                    if !value.is_truthy() {
                        if let Some(frame) = self.call_frames.last_mut() {
                            frame.ip = offset;
                        }
                    }
                }
            }
            Instruction::JumpIfTrue { offset } => {
                if let Some(value) = self.stack.last() {
                    if value.is_truthy() {
                        if let Some(frame) = self.call_frames.last_mut() {
                            frame.ip = offset;
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
                // Pop arguments and function from stack
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - arg_count);
                if let Some(callee) = self.stack.pop() {
                    let result = self.call_value(callee, args)?;
                    self.stack.push(result);
                }
            }
            Instruction::Return => {
                if let Some(_value) = self.stack.pop() {
                    self.call_frames.pop();
                }
            }
            Instruction::ReturnNil => {
                self.stack.push(Value::Nil);
                self.call_frames.pop();
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
            Instruction::Lambda { param_count: _, body_index } => {
                // Load closure from constants
                if let Some(frame) = self.call_frames.last() {
                    if body_index < frame.function.constants.len() {
                        self.stack.push(frame.function.constants[body_index].clone());
                    }
                }
            }
            Instruction::NewInstance { class_name: _, arg_count } => {
                // Pop arguments and class, create instance
                let args: Vec<Value> = self.stack.split_off(self.stack.len() - arg_count);
                if let Some(class) = self.stack.pop() {
                    let instance = self.instantiate_class(class, args)?;
                    self.stack.push(instance);
                }
            }
            Instruction::This => {
                // Push 'this' reference (current instance)
                // For now, push nil as placeholder
                self.stack.push(Value::Nil);
            }
            Instruction::Super { method: _ } => {
                // Super method lookup - not fully implemented
                self.stack.push(Value::Nil);
            }
            Instruction::Spread => {
                // Spread operator - expand array into individual values
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
                // Pattern matching - simplified implementation
                let match_value = self.stack.pop().unwrap_or(Value::Nil);
                // Skip arm_count instructions if no match
                // Full implementation would check patterns
                self.stack.push(match_value);
            }
            Instruction::Require { path } => {
                // Module loading - simplified
                self.stack.push(Value::String(format!("<module {:?}>", path)));
            }
            Instruction::Export { name } => {
                // Export value to module exports
                if let Some(value) = self.stack.pop() {
                    self.globals.insert(name.id(), value.clone());
                    self.stack.push(value);
                }
            }
            Instruction::Set => {
                // Set literal - already handled by NewTable
            }
            Instruction::Class { name, method_count } => {
                // Create class from methods on stack
                let mut methods = IndexMap::new(); // <-- Заменили здесь
                for i in 0..method_count {
                    if let Some(method) = self.stack.pop() {
                        methods.insert(i, method);
                    }
                }
                let class = Value::Table(Rc::new(RefCell::new(methods)));
                self.globals.insert(name.id(), class.clone());
                self.stack.push(class);
            }
            Instruction::Break | Instruction::Continue | Instruction::Line { .. } => {
                // Control flow and debug instructions - handled by interpreter
            }
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

    fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, String> {
        match callee {
            Value::NativeFunction(func) => func(&args, &mut self.interner),  // ← Добавили второй аргумент
            Value::Function { name: _, params: _, default_params: _, body: _, closure: _ } => {
                Ok(Value::Nil)
            }
            _ => Err(format!("Cannot call non-function value: {callee:?}")),
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

    fn instantiate_class(&self, class: Value, _args: Vec<Value>) -> Result<Value, String> {
        match class {
            Value::Table(t) => {
                // Create new instance with same methods
                let methods = t.borrow().clone();
                Ok(Value::Table(Rc::new(RefCell::new(methods))))
            }
            _ => Err("Cannot instantiate non-class".to_string()),
        }
    }
}
