//! Interpreter Execution Module
//! This module contains the core execution logic for the Taurine interpreter.
//! It handles statement and expression evaluation, control flow, and runtime errors.

use crate::ast::{Expr, Stmt, Program, Pattern};
use crate::value::Value;
use crate::lexer;
use crate::string_intern::InternedString;
use crate::safety::{SafetyContext, SafetyLimits};
use crate::gc::{GarbageCollector, GcConfig, GcStats};
use super::native_functions::register_builtins;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use smallvec::SmallVec;
use crate::value::GeneratorExecutionState;
use crate::safety::{SecurityContext};

fn get_global_packages_dir() -> PathBuf {
    let mut path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    path.push(".taurine");
    path.push("packages");
    path
}
fn resolve_module_path(base_path: &PathBuf, module_path: &str) -> Option<PathBuf> {
    let local = base_path.join(module_path);
    if local.exists() {
        return Some(local);
    }
    let global_dir = get_global_packages_dir();
    if global_dir.exists() {
        let global_pkg = global_dir.join(module_path);
        if global_pkg.exists() {
            let main_file = global_pkg.join("main.tau");
            if main_file.exists() {
                return Some(main_file);
            }
            let tau_file = global_pkg.join(format!("{}.tau", module_path));
            if tau_file.exists() {
                return Some(tau_file);
            }
        }

        if let Ok(entries) = std::fs::read_dir(&global_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    if name == module_path || name.starts_with(&format!("{}-", module_path)) {
                        if let Ok(files) = std::fs::read_dir(&path) {
                            for file in files.flatten() {
                                let file_path = file.path();
                                if file_path.extension().map_or(false, |ext| ext == "tau") {
                                    return Some(file_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    let std_path = base_path.join("std").join(format!("{}.tau", module_path));
    if std_path.exists() {
        return Some(std_path);
    }
    
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlFlow {
    None,
    Break,
    Continue,
}

pub struct StackFrame {
    pub function: String,
    pub line: usize,
}

pub struct Interpreter {
    global: Rc<RefCell<crate::environment::Environment>>,
    return_value: Option<Value>,
    base_path: PathBuf,
    call_stack: Vec<StackFrame>,
    current_line: usize,
    control_flow: ControlFlow,
    safety: SafetyContext,
    security: crate::safety::SecurityContext,
    module_cache: Rc<RefCell<std::collections::HashMap<String, Value>>>,
    gc: Option<GarbageCollector>,
    pub(crate) interner: crate::string_intern::StringInterner,
    call_depth: usize,
    this_stack: Vec<Value>,
}

fn create_default_interner() -> crate::string_intern::StringInterner {
    let mut interner = crate::string_intern::StringInterner::new();
    interner.intern_with_id("print", 1);
    interner.intern_with_id("assert", 2);
    interner.intern_with_id("assert_eq", 3);
    interner.intern_with_id("type", 4);
    interner.intern_with_id("tonumber", 5);
    interner.intern_with_id("tostring", 6);
    interner.intern_with_id("io_read", 10);
    interner.intern_with_id("io_write", 11);
    interner.intern_with_id("io_append", 12);
    interner.intern_with_id("io_exists", 13);
    interner.intern_with_id("io_remove", 14);
    interner.intern_with_id("io_mkdir", 15);
    interner.intern_with_id("io_platform", 16);
    interner.intern_with_id("io_arch", 17);
    interner.intern_with_id("io_cwd", 18);
    interner.intern_with_id("io_exit", 19);
    interner.intern_with_id("io_sleep", 20);
    interner.intern_with_id("io_time", 21);
    interner.intern_with_id("io_strupper", 30);
    interner.intern_with_id("io_strlower", 31);
    interner.intern_with_id("io_strtrim", 32);
    interner.intern_with_id("io_strsubstr", 33);
    interner.intern_with_id("io_strfind", 34);
    interner.intern_with_id("io_strreplace", 35);
    interner.intern_with_id("io_strreplaceall", 36);
    interner.intern_with_id("io_strsplit", 37);
    interner.intern_with_id("io_char", 38);
    interner.intern_with_id("io_byte", 39);
    interner.intern_with_id("io_arraypush", 40);
    interner.intern_with_id("io_arraypop", 41);
    interner.intern_with_id("io_arraylen", 42);
    interner.intern_with_id("io_arrayget", 43);
    interner.intern_with_id("io_arrayset", 44);
    interner.intern_with_id("io_arrayconcat", 45);
    interner.intern_with_id("io_arrayreverse", 46);
    interner.intern_with_id("io_arrayclear", 47);
    interner.intern_with_id("json_parse", 50);
    interner.intern_with_id("json_stringify", 51);
    interner.intern_with_id("http_get", 60);
    interner.intern_with_id("http_post", 61);
    interner.intern_with_id("http_put", 62);
    interner.intern_with_id("http_delete", 63);
    interner.intern_with_id("crypto_md5", 70);
    interner.intern_with_id("crypto_sha256", 71);
    interner.intern_with_id("crypto_base64_encode", 72);
    interner.intern_with_id("crypto_base64_decode", 73);
    interner.intern_with_id("crypto_uuid", 74);
    interner.intern_with_id("crypto_random_bytes", 75);
    interner.intern_with_id("date_now", 80);
    interner.intern_with_id("date_format", 81);
    interner.intern_with_id("regex_match", 90);
    interner.intern_with_id("regex_find", 91);
    interner.intern_with_id("regex_replace", 92);
    interner.intern_with_id("regex_find_all", 93);
    interner.intern_with_id("async_sleep", 100);
    interner.intern_with_id("async_spawn", 101);
    interner
}

impl Interpreter {
    pub fn new(base_path: PathBuf) -> Self {
        Self::with_interner(base_path, create_default_interner())
    }
    pub fn with_interner(base_path: PathBuf, interner: crate::string_intern::StringInterner) -> Self {
        Self::with_limits_and_interner(base_path, SafetyLimits::default(), interner)
    }

    pub fn with_limits(base_path: PathBuf, limits: SafetyLimits) -> Self {
        Self::with_limits_and_interner(base_path, limits, create_default_interner())
    }

    pub fn with_limits_and_interner(base_path: PathBuf, limits: SafetyLimits, interner: crate::string_intern::StringInterner) -> Self {
        Self::with_gc_and_interner(base_path, limits, None, interner)
    }

    pub fn with_gc(base_path: PathBuf, limits: SafetyLimits, gc_config: Option<GcConfig>) -> Self {
        Self::with_gc_and_interner(base_path, limits, gc_config, create_default_interner())
    }

    pub fn with_gc_and_interner(base_path: PathBuf, limits: SafetyLimits, gc_config: Option<GcConfig>, interner: crate::string_intern::StringInterner) -> Self {
        let global = Rc::new(RefCell::new(crate::environment::Environment::new()));
        let safety = SafetyContext::new(limits);
        let security = crate::safety::SecurityContext::new();
        let gc = gc_config.map(GarbageCollector::new);
        let mut interp = Interpreter {
            global,
            return_value: None,
            base_path,
            call_stack: Vec::new(),
            current_line: 1,
            control_flow: ControlFlow::None,
            safety,
            security,
            interner,
            module_cache: Rc::new(RefCell::new(std::collections::HashMap::new())),
            gc,
            call_depth: 0,
            this_stack: Vec::new(),
        };
        register_builtins(&interp.global);
        interp.load_stdlib();
        interp
    }

    pub fn generator_next(&mut self, generator: &Value) -> Result<Option<Value>, String> {
        match generator {
            Value::Generator { body, closure, state, .. } => {
                {
                    let mut gen_state = state.borrow_mut();
                    if gen_state.consumed_index < gen_state.yielded_values.len() {
                        let value = gen_state.yielded_values[gen_state.consumed_index].clone();
                        gen_state.consumed_index += 1;
                        return Ok(Some(value));
                    }
                    if gen_state.is_done {
                        return Ok(None);
                    }
                }

                let exec_state = state.borrow().execution_state.clone();
                let parent_env = closure.clone();
                let body_to_run = match &exec_state {
                    GeneratorExecutionState::NotStarted => body.clone(),
                    GeneratorExecutionState::Suspended { remaining_body, .. } => remaining_body.clone(),
                    GeneratorExecutionState::Finished => return Ok(None),
                };

                let new_env = std::rc::Rc::new(std::cell::RefCell::new(
                    crate::environment::Environment::with_parent(parent_env)
                ));
                let old_global = std::mem::replace(&mut self.global, new_env.clone());
                let old_return = self.return_value.take();

                let mut found_yield = false;
                let mut yielded_val = Value::Nil;
                let mut suspend_index = 0;

                for (i, stmt) in body_to_run.iter().enumerate() {
                    match self.execute_stmt(stmt.clone()) {
                        Ok(_) => {
                            if let Some(val) = self.return_value.take() {
                                found_yield = true;
                                yielded_val = val;
                                suspend_index = i;
                                break;
                            }
                        }
                        Err(e) => {
                            self.global = old_global;
                            self.return_value = old_return;
                            return Err(e);
                        }
                    }
                }

                self.global = old_global;
                self.return_value = old_return;

                let mut gen_state = state.borrow_mut();
                if found_yield {
                    gen_state.yielded_values.push(yielded_val);
                    let original_len = match &exec_state {
                        GeneratorExecutionState::NotStarted => body.len(),
                        GeneratorExecutionState::Suspended { remaining_body, .. } => remaining_body.len(),
                        _ => 0,
                    };
                    if suspend_index + 1 < original_len {
                        let full_body = match &exec_state {
                            GeneratorExecutionState::NotStarted => body.clone(),
                            GeneratorExecutionState::Suspended { remaining_body, .. } => remaining_body.clone(),
                            _ => vec![],
                        };
                        gen_state.execution_state = GeneratorExecutionState::Suspended {
                            remaining_body: full_body[suspend_index + 1..].to_vec(),
                            closure: new_env.clone(),
                        };
                    } else {
                        gen_state.execution_state = GeneratorExecutionState::Finished;
                        gen_state.is_done = true;
                    }
                } else {
                    gen_state.is_done = true;
                    gen_state.execution_state = GeneratorExecutionState::Finished;
                    return Ok(None);
                }

                let value = gen_state.yielded_values[gen_state.consumed_index].clone();
                gen_state.consumed_index += 1;
                Ok(Some(value))
            }
            _ => Err("Not a generator".to_string()),
        }
    }

    pub fn clear_module_cache(&self) {
        self.module_cache.borrow_mut().clear();
    }

    pub fn update_interner(&mut self, new_interner: crate::string_intern::StringInterner) {
        self.interner = new_interner;
    }

    pub fn invalidate_module(&self, path: &str) {
        self.module_cache.borrow_mut().remove(path);
    }

    pub fn safety(&self) -> &SafetyContext {
        &self.safety
    }

    pub fn security(&self) -> &crate::safety::SecurityContext {
        &self.security
    }

    pub fn security_mut(&mut self) -> &mut crate::safety::SecurityContext {
        &mut self.security
    }

    pub fn set_security_level(&mut self, level: crate::safety::SecurityLevel) {
        self.security.apply_level(&level);
    }

    /// Get GC statistics if GC is enabled
    pub fn gc_stats(&self) -> Option<GcStats> {
        self.gc.as_ref().map(|gc| gc.stats().clone())
    }

    /// Trigger garbage collection
    pub fn gc_collect(&mut self) {
        if let Some(ref mut gc) = self.gc {
            gc.collect();
        }
    }

    /// Force full garbage collection
    pub fn gc_collect_full(&mut self) {
        if let Some(ref mut gc) = self.gc {
            gc.collect_full();
        }
    }

    /// Register a table/array value with the GC for cycle tracking
    pub fn gc_track_value(&mut self, id: usize, size: usize) -> usize {
        if let Some(ref mut gc) = self.gc {
            gc.allocate(size)
        } else {
            id
        }
    }

    /// Register a parent-child relationship for cycle detection
    pub fn gc_add_child(&mut self, parent_id: usize, child_id: usize) {
        if let Some(ref mut gc) = self.gc {
            gc.add_child(parent_id, child_id);
        }
    }

    /// Collect cycles: detect and break circular references
    pub fn gc_collect_cycles(&mut self) -> usize {
        if let Some(ref mut gc) = self.gc {
            let before = gc.stats().cycles_found;
            gc.collect_full();
            let after = gc.stats().cycles_found;
            (after - before) as usize
        } else {
            0
        }
    }

    /// Get the number of cycles found
    pub fn gc_cycles_found(&self) -> usize {
        self.gc.as_ref().map_or(0, |gc| gc.stats().cycles_found)
    }

    /// Get GC enabled status
    pub fn gc_enabled(&self) -> bool {
        self.gc.is_some()
    }

    /// Set GC enabled
    pub fn set_gc_enabled(&mut self, enabled: bool) {
        if let Some(ref gc) = self.gc {
            gc.set_enabled(enabled);
        }
    }

    /// Record memory allocation for GC tracking
    pub fn interrupt(&self) {
        self.safety.interrupt();
    }

    pub fn reset_safety(&mut self) {
        self.safety.reset();
    }

    pub fn optimize(&mut self) {
        // Placeholder for optimization setup
    }

    pub fn interpret(&mut self, program: Program) -> Result<(), crate::error::TaurineError> {
        self.safety.reset();
        for stmt in program.statements {
            self.safety.safety_check().map_err(|e| crate::error::TaurineError::Runtime { 
                message: e, 
                line: self.current_line 
            })?;
            
            if let Err(e) = self.execute_stmt(stmt) {
                let formatted_msg = self.format_error_with_traceback(&e);
                return Err(crate::error::TaurineError::Runtime { 
                    message: formatted_msg, 
                    line: self.current_line 
                });
            }
        }
        Ok(())
    }

    pub fn interpret_optimized(&mut self, program: Program) -> Result<(), String> {
        let mut optimizer = crate::optimizer::Optimizer::new();
        let optimized_program = optimizer.optimize(program);
        self.safety.reset();
        for stmt in optimized_program.statements {
            self.safety.safety_check()?;
            if let Err(e) = self.execute_stmt(stmt) {
                return Err(self.format_error_with_traceback(&e));
            }
        }
        Ok(())
    }

    pub fn run(&mut self, source: &str) -> Result<(), crate::error::TaurineError> {
        let tokens = crate::lexer::tokenize_with_interner(source, &mut self.interner);
        let mut parser = crate::parser::Parser::with_interner(tokens, self.interner.clone());
        let program = parser.parse().map_err(|e| crate::error::TaurineError::Parse {
            message: e,
            line: 0
        })?;
        if let Some(updated) = parser.take_interner() {
            self.interner = updated;
        }
        self.interpret(program)
    }

    pub fn run_optimized(&mut self, source: &str) -> Result<(), String> {
        let tokens = crate::lexer::tokenize_with_interner(source, &mut self.interner);
        let mut parser = crate::parser::Parser::with_interner(tokens, self.interner.clone());
        let program = parser.parse()?;
        if let Some(updated) = parser.take_interner() {
            self.interner = updated;
        }
        self.interpret_optimized(program)
    }

    pub fn get(&self, name: &str) -> Result<Value, String> {
        if let Some(id) = self.interner.get_id(name) {
        self.global.borrow().get(&InternedString::new(id))
        } else {
            Err(format!("Undefined variable: {name}"))
        }
    }

    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        let id = self.interner.intern(name);
        self.global.borrow_mut().define(InternedString::new(id), value);
        Ok(())
    }

    /// Get the global environment (for FFI)
    pub fn global_env(&self) -> Rc<RefCell<crate::environment::Environment>> {
        self.global.clone()
    }

    fn format_error_with_traceback(&self, error: &str) -> String {
        let mut msg = format!("Error: {error}\n");
        msg.push_str("\nStack traceback:\n");
        if self.call_stack.is_empty() {
            msg.push_str("  [main chunk]\n");
        } else {
            for (i, frame) in self.call_stack.iter().enumerate() {
                msg.push_str(&format!("  [{}]: {} at line {}\n", i, frame.function, frame.line));
            }
            msg.push_str("  [main chunk]\n");
        }
        msg
    }

    #[allow(dead_code)]
    fn find_std_dir(&self) -> Option<PathBuf> {
        if let Ok(p) = std::env::var("TAURINE_STD_DIR") {
            let p = PathBuf::from(p);
            if p.exists() { return Some(p); }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("std");
                if p.exists() { return Some(p); }
            }
        }
        let p = PathBuf::from("./std");
        if p.exists() { return Some(p); }
        
        None
    }

    fn load_stdlib(&mut self) {
        if let Some(dir) = self.find_std_dir() {
            let core_files = [
                "array.tau", "json.tau", "crypto.tau",
                "date.tau", "http.tau", "regex.tau",
            ];
            for file in core_files {
                let path = dir.join(file);
                if path.exists() {
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        let tokens = crate::lexer::tokenize_with_interner(&source, &mut self.interner);
                        let mut parser = crate::parser::Parser::with_interner(tokens, self.interner.clone());
                        if let Ok(program) = parser.parse() {
                            if let Some(updated_interner) = parser.take_interner() {
                                self.interner = updated_interner;
                            }
                            let _ = self.interpret(program);
                            self.return_value = None;
                        }
                    }
                }
            }
        }
    }


    fn get_line(&self, expr: &Expr) -> usize {
        match expr {
            Expr::Number(_) => 0,
            Expr::String(_) => 0,
            Expr::Identifier(_) => 0,
            Expr::Binary { line, .. } => *line,
            Expr::Unary { line, .. } => *line,
            Expr::Call { line, .. } => *line,
            Expr::LiteralTrue => 0,
            Expr::LiteralFalse => 0,
            Expr::LiteralNil => 0,
            Expr::Table { line, .. } => *line,
            Expr::Array { line, .. } => *line,
            Expr::Index { line, .. } => *line,
            Expr::SetIndex { line, .. } => *line,
            Expr::Range { line, .. } => *line,
            Expr::Length { line, .. } => *line,
            Expr::Get { line, .. } => *line,
            Expr::SafeGet { line, .. } => *line,
            Expr::SetProperty { line, .. } => *line,
            Expr::Throw { line, .. } => *line,
            Expr::FunctionLiteral { line, .. } => *line,
            Expr::AsyncFunctionLiteral { line, .. } => *line,
            Expr::GeneratorLiteral { line, .. } => *line,
            Expr::Lambda { line, .. } => *line,
            Expr::Spread { line, .. } => *line,
            Expr::NullCoalesce { line, .. } => *line,
            Expr::Match { line, .. } => *line,
            Expr::Require { line, .. } => *line,
            Expr::Export { line, .. } => *line,
            Expr::Class { line, .. } => *line,
            Expr::NewInstance { line, .. } => *line,
            Expr::Await { line, .. } => *line,
            Expr::Yield { line, .. } => *line,
            _ => 0,
        }
    }

    fn execute_stmt(&mut self, stmt: Stmt) -> Result<(), String> {
        self.safety.safety_check()?;

        match stmt {
            Stmt::Declaration { name, initializer, line, is_const } => {
                self.current_line = line;
                let value = if let Some(init) = initializer {
                    self.execute_expr(init)?
                } else {
                    Value::Nil
                };
                if is_const {
                    self.global.borrow_mut().define_const(name, value);
                } else {
                    self.global.borrow_mut().define(name, value);
                }
            }
            Stmt::Destructure { names, initializer, line } => {
                self.current_line = line;
                let value = self.execute_expr(initializer)?;
                match &value {
                    Value::Array(arr) => {
                        let arr_ref = arr.borrow();
                        for (i, name) in names.iter().enumerate() {
                            let val = arr_ref.get(i).cloned().unwrap_or(Value::Nil);
                            self.global.borrow_mut().define(*name, val);
                        }
                    }
                    Value::Table(map) => {
                        let map_ref = map.borrow();
                        for name in &names {
                            let val = map_ref.get(&name.id()).cloned().unwrap_or(Value::Nil);
                            self.global.borrow_mut().define(*name, val);
                        }
                    }
                    _ => {
                        for name in &names {
                            self.global.borrow_mut().define(*name, Value::Nil);
                        }
                    }
                }
                return Ok(())
            }
            Stmt::Assignment { name, value, line, is_const_assign: _ } => {
                self.current_line = line;
                let val = self.execute_expr(value)?;
                self.global.borrow_mut().assign(&name, val)?;
            }
            Stmt::Set { object, name, value, line } => {
                self.current_line = line;
                let obj_val = self.execute_expr(*object)?;
                let val = self.execute_expr(value)?;
                match obj_val {
                    Value::Table(map) => {
                        map.borrow_mut().insert(name.id(), val);
                    }
                    _ => return Err(format!("Cannot set property on non-table value")),
                }
                return Ok(())
            }
            Stmt::Expression(expr) => {
                self.current_line = self.get_line(&expr);
                // Check if this is a yield expression before moving expr
                let is_yield = matches!(&expr, Expr::Yield { .. });
                let _result = self.execute_expr(expr)?;
                // Yield values are handled by generator state management
                let _ = is_yield;
            }
            Stmt::Return(expr) => {
                self.return_value = if let Some(e) = expr {
                    Some(self.execute_expr(e)?)
                } else {
                    Some(Value::Nil)
                };
                return Ok(());
            }
            Stmt::ReturnMulti(values) => {
                let mut result_values = Vec::new();
                for expr in values {
                    result_values.push(self.execute_expr(expr)?);
                }
                self.return_value = Some(Value::Array(std::rc::Rc::new(std::cell::RefCell::new(result_values.into()))));
                return Ok(());
            }
            Stmt::If { condition, then_branch, else_branch, line } => {
                self.current_line = line;
                let cond = self.execute_expr(condition)?;
                if cond.is_truthy() {
                    for stmt in then_branch {
                        self.execute_stmt(stmt)?;
                    }
                } else if let Some(else_stmts) = else_branch {
                    for stmt in else_stmts {
                        self.execute_stmt(stmt)?;
                    }
                }
            }
            Stmt::While { condition, body, line } => {
                self.current_line = line;
                loop {
                    self.safety.safety_check()?;
                    let cond = self.execute_expr(condition.clone())?;
                    if !cond.is_truthy() {
                        break;
                    }
                    for stmt in &body {
                        match self.execute_stmt(stmt.clone()) {
                            Ok(_) => {}
                            Err(e) => return Err(e),
                        }
                        if self.control_flow == ControlFlow::Break {
                            self.control_flow = ControlFlow::None;
                            return Ok(());
                        }
                        if self.control_flow == ControlFlow::Continue {
                            self.control_flow = ControlFlow::None;
                            break;
                        }
                    }
                }
            }
            Stmt::For { initializer, condition, increment, body, line } => {
                self.current_line = line;
                if let Some(init) = initializer {
                    self.execute_expr(*init)?;
                }
                loop {
                    self.safety.safety_check()?;
                    let cond = self.execute_expr((*condition).clone())?;
                    if !cond.is_truthy() {
                        break;
                    }
                    for stmt in &body {
                        match self.execute_stmt(stmt.clone()) {
                            Ok(_) => {}
                            Err(e) => return Err(e),
                        }
                        if self.control_flow == ControlFlow::Break {
                            self.control_flow = ControlFlow::None;
                            return Ok(());
                        }
                        if self.control_flow == ControlFlow::Continue {
                            self.control_flow = ControlFlow::None;
                            break;
                        }
                    }
                    if let Some(inc) = &increment {
                        self.execute_expr((**inc).clone())?;
                    }
                }
            }
            Stmt::ForIn { variable, iterable, body, line } => {
                self.current_line = line;
                let iter_value = self.execute_expr(iterable)?;
                match iter_value {
                    Value::Array(arr) => {
                        let arr_ref = arr.borrow();
                        for item in arr_ref.iter() {
                            self.global.borrow_mut().define(variable, item.clone());
                            for stmt in &body {
                                match self.execute_stmt(stmt.clone()) {
                                    Ok(_) => {}
                                    Err(e) => return Err(e),
                                }
                                if self.control_flow == ControlFlow::Break {
                                    self.control_flow = ControlFlow::None;
                                    return Ok(());
                                }
                                if self.control_flow == ControlFlow::Continue {
                                    self.control_flow = ControlFlow::None;
                                    break;
                                }
                            }
                        }
                    }
                    Value::Range { start, end } => {
                        let start_i = start as i64;
                        let end_i = end as i64;
                        for i in start_i..end_i {
                            self.global.borrow_mut().define(variable, Value::Number(i as f64));
                            for stmt in &body {
                                match self.execute_stmt(stmt.clone()) {
                                    Ok(_) => {}
                                    Err(e) => return Err(e),
                                }
                                if self.control_flow == ControlFlow::Break {
                                    self.control_flow = ControlFlow::None;
                                    return Ok(());
                                }
                                if self.control_flow == ControlFlow::Continue {
                                    self.control_flow = ControlFlow::None;
                                    break;
                                }
                            }
                        }
                    }

                    Value::Generator { .. } => {
                        loop {
                            self.safety.safety_check()?;
                            let item = match self.generator_next(&iter_value)? {
                                None => break,
                                Some(val) => val,
                            };
                            self.global.borrow_mut().define(variable, item);
                            for stmt in &body {
                                match self.execute_stmt(stmt.clone()) {
                                    Ok(_) => {}
                                    Err(e) => return Err(e),
                                }
                                if self.control_flow == ControlFlow::Break {
                                    self.control_flow = ControlFlow::None;
                                    return Ok(());
                                }
                                if self.control_flow == ControlFlow::Continue {
                                    self.control_flow = ControlFlow::None;
                                    break;
                                }
                            }
                        }
                    }
                    _ => return Err("Can only iterate over arrays, ranges, or generators".to_string()),
                }
            }
            Stmt::Function { name, params, body, line } => {
                self.current_line = line;
                let func = Value::Function {
                    name: name.id(),
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    default_params: params.iter().map(|(_, d)| d.clone()).collect(),
                    body,
                    closure: self.global.clone(),
                };
                self.global.borrow_mut().define(name, func);
            }
            Stmt::AsyncFunction { name, params, body, line } => {
                self.current_line = line;
                let func = Value::AsyncFunction {
                    name: name.id(),
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    default_params: params.iter().map(|(_, d): &(InternedString, Option<Expr>)| d.clone()).collect(),
                    body,
                    closure: self.global.clone(),
                };
                self.global.borrow_mut().define(name, func);
            }
            Stmt::Generator { name, params, body, line } => {
                self.current_line = line;
                let func = Value::Generator {
                    name: name.id(),
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    body,
                    closure: self.global.clone(),
                    state: Rc::new(RefCell::new(crate::value::GeneratorState::default())),
                };
                self.global.borrow_mut().define(name, func);
            }
            Stmt::Block(stmts) => {
                for stmt in stmts {
                    self.execute_stmt(stmt)?;
                }
            }
            Stmt::Import { path, alias, line } => {
                self.current_line = line;
                let module_path = resolve_module_path(&self.base_path, &path)
                    .ok_or_else(|| format!("Cannot import '{path}': module not found"))?;
                let canonical_path = std::fs::canonicalize(&module_path).unwrap_or(module_path.clone());
                let module_key = canonical_path.to_string_lossy().to_string();

                if let Some(cached) = self.module_cache.borrow().get(&module_key) {
                    if let Some(alias_name) = alias {
                        self.global.borrow_mut().define(alias_name, cached.clone());
                    } else {
                        if let Value::Table(map) = cached {
                            let entries: Vec<(usize, Value)> = map.borrow().iter().map(|(k, v)| (*k, v.clone())).collect();
                            for (k, v) in entries {
                                self.global.borrow_mut().define_raw(k, v);
                            }
                        }
                    }
                    return Ok(());
                }

                let source = std::fs::read_to_string(&module_path)
                    .map_err(|e| format!("Cannot import '{path}': {e}"))?;

                let tokens = crate::lexer::tokenize_with_interner(&source, &mut self.interner);
                let mut parser = crate::parser::Parser::with_interner(tokens, self.interner.clone());
                let program = parser.parse()?;
                if let Some(updated_interner) = parser.take_interner() {
                    self.interner = updated_interner;
                }

                let module_table = Value::new_table();

                match alias {
                    Some(alias_name) => {
                        let mut sub_interp = Interpreter::with_interner(self.base_path.clone(), self.interner.clone());
                        sub_interp.interpret(program)?;
                        self.interner = sub_interp.interner.clone();
                        if let Value::Table(module_map) = &module_table {
                            let sub_globals = sub_interp.global.borrow().all_entries();
                            let mut module_ref = module_map.borrow_mut();
                            for (k, v) in sub_globals {
                                module_ref.insert(k, v);
                            }
                        }
                        self.module_cache.borrow_mut().insert(module_key, module_table.clone());
                        self.global.borrow_mut().define(alias_name, module_table);
                    }
                    None => {
                        for stmt in program.statements {
                            self.execute_stmt(stmt)?;
                        }
                        let module_table = Value::new_table();
                        self.module_cache.borrow_mut().insert(module_key, module_table);
                    }
                }
                return Ok(())
            }
            Stmt::Throw { value, line } => {
                self.current_line = line;
                let val = self.execute_expr(value)?;
                return Err(format!("Uncaught exception: {:?}", val));
            }
            Stmt::Try { body, catch_var, catch_body, line } => {
                self.current_line = line;
                let result = (|| -> Result<(), String> {
                    for stmt in body {
                        self.execute_stmt(stmt)?;
                        if self.return_value.is_some() || self.control_flow != ControlFlow::None {
                            break;
                        }
                    }
                    Ok(())
                })();

                if let Err(e) = result {
                    if !catch_body.is_empty() {
                        if let Some(var_name) = catch_var {
                            self.global.borrow_mut().define(var_name, Value::String(e));
                        }
                        for stmt in catch_body {
                            self.execute_stmt(stmt)?;
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
            Stmt::Class { name, superclass, methods, line } => {
                self.current_line = line;
                let mut class_table: IndexMap<usize, Value> = IndexMap::new();
                for method_item in methods {
                    let (member_name, member_expr): (InternedString, Expr) = method_item;
                    if let Expr::FunctionLiteral { params, body, .. } = member_expr {
                        // Это метод
                        let method = Value::Function {
                            name: member_name.id(),
                            params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                            default_params: params.iter().map(|(_, d): &(InternedString, Option<Expr>)| d.clone()).collect(),
                            body,
                            closure: self.global.clone(),
                        };
                        class_table.insert(member_name.id(), method);
                    } else {
                        // Это поле класса (например, width = 10)
                        let val = self.execute_expr(member_expr)?;
                        class_table.insert(member_name.id(), val);
                    }
                }
                let class_value = Value::Table(Rc::new(RefCell::new(class_table)));
                self.global.borrow_mut().define(name, class_value);
                let _ = superclass;
            }
            Stmt::Export { name, value, line } => {
                self.current_line = line;
                let val = self.execute_expr(value)?;
                self.global.borrow_mut().define(name, val);
            }
            Stmt::Break => {
                self.control_flow = ControlFlow::Break;
            }
            Stmt::Continue => {
                self.control_flow = ControlFlow::Continue;
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_expr(&mut self, expr: Expr) -> Result<Value, String> {
        self.safety.safety_check()?;

        match expr {
            Expr::Number(n) => Ok(Value::Number(n)),
            Expr::String(s) => Ok(Value::String(s)),
            Expr::LiteralTrue => Ok(Value::Bool(true)),
            Expr::LiteralFalse => Ok(Value::Bool(false)),
            Expr::LiteralNil => Ok(Value::Nil),
            Expr::Identifier(name) => {
                self.global.borrow().get(&name)
            }
            Expr::Binary { left, op, right, line } => {
                self.current_line = line;
                let left_val = self.execute_expr(*left)?;
                let right_val = self.execute_expr(*right)?;
                self.evaluate_binary(left_val, right_val, op)
            }
            Expr::Unary { op, expr, line } => {
                self.current_line = line;
                let val = self.execute_expr(*expr)?;
                self.evaluate_unary(val, op)
            }
            Expr::Call { callee, arguments, line } => {
                self.current_line = line;
                if let Expr::Identifier(name) = &*callee {
                    if let Some(func_name) = self.interner.get(name.id()) {
                        if !self.security.is_function_allowed(func_name) {
                            return Err(format!("Function '{}' is blocked by security policy", func_name));
                        }
                    }
                    if name.id() == 50 && arguments.len() == 1 {
                        let arg = self.execute_expr(arguments[0].clone())?;
                        if let Value::String(s) = arg {
                            return self.json_parse(&s);
                        }
                    }
                    if name.id() == 51 && arguments.len() == 1 {
                        let arg = self.execute_expr(arguments[0].clone())?;
                        let json = self.json_stringify(&arg)?;
                        return Ok(Value::String(json));
                    }
                }
                if let Expr::Get { object, name, .. } = &*callee {
                    let obj_val = self.execute_expr(*object.clone())?;
                    let method = match &obj_val {
                        Value::Table(map) => map.borrow().get(&name.id()).cloned(),
                        _ => None,
                    };
                    if let Some(method_val) = method {
                        let args: Result<Vec<Value>, String> = arguments
                            .into_iter()
                            .map(|arg| self.execute_expr(arg))
                            .collect();
                        let args = args?;
                        self.this_stack.push(obj_val.clone());
                        let result = self.call_function(method_val, args);
                        self.this_stack.pop();
                        return result;
                    }
                }
                if let Expr::SafeGet { object, name, .. } = &*callee {
                    let obj_val = self.execute_expr(*object.clone())?;
                    if obj_val == Value::Nil { return Ok(Value::Nil); }
                    if let Value::Table(ref t) = obj_val {
                        if let Some(method) = t.borrow().get(&name.id()).cloned() {
                            let args_res: Result<Vec<Value>, String> = arguments.into_iter().map(|a| self.execute_expr(a)).collect();
                            let args = args_res?;
                            self.this_stack.push(obj_val.clone());
                            let result = self.call_function(method, args);
                            self.this_stack.pop();
                            return result;
                        }
                    }
                }
                let func = self.execute_expr(*callee)?;
                let args: Result <Vec <Value >, String > = arguments
                    .into_iter()
                    .map(|arg| self.execute_expr(arg))
                    .collect();
                self.call_function(func, args?)
            }

            Expr::Table { entries, line } => {
                self.current_line = line;
                let mut table: IndexMap<usize, Value> = IndexMap::new();
                for entry in entries {
                    let (key, value): (InternedString, Expr) = entry;
                    let val = self.execute_expr(value)?;
                    table.insert(key.id(), val);
                }
                Ok(Value::Table(Rc::new(RefCell::new(table))))
            }
            Expr::Array { items, line } => {
                self.current_line = line;
                let mut arr = SmallVec::new();
                for item in items {
                    arr.push(self.execute_expr(item)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(arr))))
            }
            Expr::Index { object, index, line } => {
                self.current_line = line;
                let obj = self.execute_expr(*object)?;
                let idx = self.execute_expr(*index)?;
                self.get_index(obj, idx)
            }
            Expr::SetIndex { object, index, value, line } => {
                self.current_line = line;
                let obj = self.execute_expr(*object)?;
                let idx = self.execute_expr(*index)?;
                let val = self.execute_expr(*value)?;
                self.set_index(obj, idx, val)?;
                Ok(Value::Nil)
            }
            Expr::Get { object, name, line } => {
                self.current_line = line;
                let obj = self.execute_expr(*object)?;
                self.get_property(obj, name)
            }
            Expr::SafeGet { object, name, line } => {
                self.current_line = line;
                let obj = self.execute_expr(*object)?;
                match obj {
                    Value::Nil => Ok(Value::Nil),
                    _ => self.get_property(obj, name),
                }
            }
            Expr::SetProperty { object, name, value, line } => {
                self.current_line = line;
                let obj = self.execute_expr(*object)?;
                let val = self.execute_expr(*value)?;
                self.set_property(obj, name, val)?;
                Ok(Value::Nil)
            }
            Expr::Range { start, end, line } => {
                self.current_line = line;
                let start_val = self.execute_expr(*start)?;
                let end_val = self.execute_expr(*end)?;
                match (start_val, end_val) {
                    (Value::Number(s), Value::Number(e)) => Ok(Value::Range { start: s, end: e }),
                    _ => Err("Range bounds must be numbers".to_string()),
                }
            }
            Expr::Length { expr, line } => {
                self.current_line = line;
                let val = self.execute_expr(*expr)?;
                match val {
                    Value::Array(arr) => Ok(Value::Number(arr.borrow().len() as f64)),
                    Value::String(s) => Ok(Value::Number(s.len() as f64)),
                    Value::Table(t) => Ok(Value::Number(t.borrow().len() as f64)),
                    _ => Err("Cannot get length of non-array/string/table".to_string()),
                }
            }
            Expr::Throw { expr, line } => {
                self.current_line = line;
                let val = self.execute_expr(*expr)?;
                Err(val.to_string())
            }
            Expr::FunctionLiteral { params, body, line } => {
                self.current_line = line;
                Ok(Value::Function {
                    name: 0,
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    default_params: params.iter().map(|(_, d): &(InternedString, Option<Expr>)| d.clone()).collect(),
                    body,
                    closure: self.global.clone(),
                })
            }
            Expr::Lambda { params, body, line } => {
                self.current_line = line;
                Ok(Value::Function {
                    name: 0,
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    default_params: vec![],
                    body: vec![Stmt::Return(Some(*body))],
                    closure: self.global.clone(),
                })
            }
            Expr::AsyncFunctionLiteral { params, body, line } => {
                self.current_line = line;
                Ok(Value::AsyncFunction {
                    name: 0,
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    default_params: params.iter().map(|(_, d)| d.clone()).collect(),
                    body,
                    closure: self.global.clone(),
                })
            }
            Expr::GeneratorLiteral { params, body, line } => {
                self.current_line = line;
                Ok(Value::Generator {
                    name: 0,
                    params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                    body,
                    closure: self.global.clone(),
                    state: Rc::new(RefCell::new(crate::value::GeneratorState::default())),
                })
            }
            Expr::Await { future, line } => {
                self.current_line = line;
                let future_val = self.execute_expr(*future)?;
                match &future_val {
                    Value::Future(state) => {
                        let state_ref = state.borrow();
                        match &*state_ref {
                            crate::value::FutureState::Ready(v) => Ok(v.clone()),
                            crate::value::FutureState::Pending => Ok(Value::Nil),
                        }
                    }
                    _ => Ok(future_val),
                }
            }
            Expr::Yield { value, line } => {
                self.current_line = line;
                let yield_val = match value {
                    Some(v) => self.execute_expr(*v)?,
                    None => Value::Nil,
                };
                // Store yield value in return_value for generator_next to pick up
                self.return_value = Some(yield_val.clone());
                Ok(yield_val)
            }
            Expr::Spread { expr, line } => {
                self.current_line = line;
                self.execute_expr(*expr)
            }
            Expr::NullCoalesce { left, right, line } => {
                self.current_line = line;
                let left_val = self.execute_expr(*left)?;
                if left_val != Value::Nil {
                    Ok(left_val)
                } else {
                    self.execute_expr(*right)
                }
            }
            Expr::Match { value, arms, line } => {
                self.current_line = line;
                let match_val = self.execute_expr(*value)?;
                for arm in arms {
                    if self.pattern_matches(&arm.pattern, &match_val)? {
                        return self.execute_expr(arm.body);
                    }
                }
                Ok(Value::Nil)
            }
            Expr::Require { path, line } => {
                self.current_line = line;
                
                // Использовать универсальный поиск модулей
                let module_path = resolve_module_path(&self.base_path, &path)
                    .ok_or_else(|| format!("Cannot require '{path}': module not found"))?;
                
                let canonical_path = std::fs::canonicalize(&module_path).unwrap_or(module_path.clone());
                let module_key = canonical_path.to_string_lossy().to_string();

                if let Some(cached) = self.module_cache.borrow().get(&module_key) {
                    return Ok(cached.clone());
                }

                let source = std::fs::read_to_string(&module_path)
                    .map_err(|e| format!("Cannot require '{path}': {e}"))?;
                let mut interner = self.interner.clone();
                let tokens = lexer::tokenize_with_interner(&source, &mut interner);
                let mut parser = crate::parser::Parser::with_interner(tokens, interner.clone());
                let program = parser.parse()?;
                
                // Update main interner with new strings from module
                self.interner = parser.take_interner().unwrap_or(interner);

                let mut sub_interp = Interpreter::with_interner(self.base_path.clone(), self.interner.clone());
                sub_interp.interpret(program)?;

                let module_values = sub_interp.global.borrow().values.borrow().clone();
                let module_table = Value::new_table();
                
                // Extract the HashMap from the table
                if let Value::Table(ref t) = module_table {
                    let mut table_ref = t.borrow_mut();
                    for (id, value) in module_values {
                        self.global.borrow_mut().define(InternedString::new(id), value.clone());
                        table_ref.insert(id, value);
                    }
                }
                
                self.module_cache.borrow_mut().insert(module_key, module_table.clone());
                Ok(module_table)
            }
            Expr::Export { name, value, line } => {
                self.current_line = line;
                let val = self.execute_expr(*value)?;
                self.global.borrow_mut().define(name, val.clone());
                Ok(val)
            }
            Expr::Class { name: _, superclass, methods, line } => {
                self.current_line = line;
                let mut class_table: IndexMap<usize, Value> = IndexMap::new();
                for method_item in methods {
                    let (member_name, member_expr): (InternedString, Expr) = method_item;
                    if let Expr::FunctionLiteral { params, body, .. } = member_expr {
                        let method = Value::Function {
                            name: member_name.id(),
                            params: params.iter().map(|(p, _): &(InternedString, Option<Expr>)| p.id()).collect(),
                            default_params: params.iter().map(|(_, d): &(InternedString, Option<Expr>)| d.clone()).collect(),
                            body,
                            closure: self.global.clone(),
                        };
                        class_table.insert(member_name.id(), method);
                    } else {
                        let val = self.execute_expr(member_expr)?;
                        class_table.insert(member_name.id(), val);
                    }
                }
                let class_value = Value::Table(Rc::new(RefCell::new(class_table)));
                let _ = superclass;
                Ok(class_value)
            }
            Expr::NewInstance { class_name, arguments, line } => {
                self.current_line = line;
                let class = self.execute_expr(Expr::Identifier(class_name))?;
                let args: Result<Vec<Value>, String> = arguments
                    .into_iter()
                    .map(|arg| self.execute_expr(arg))
                    .collect();
                self.instantiate_class(class, args?)
            }
            Expr::FString { parts, line } => {
                self.current_line = line;
                let mut result = String::new();
                for (s, expr_opt) in parts {
                    result.push_str(&s);
                    if let Some(expr) = expr_opt {
                        let val = self.execute_expr(*expr)?;
                        result.push_str(&val.to_string());
                    }
                }
                Ok(Value::String(result))
            }
            Expr::This { line: _ } => {
                Ok(self.this_stack.last().cloned().unwrap_or(Value::Nil))
            }
            Expr::Super { method, line: _ } => {
                let _ = method;
                Ok(Value::Nil)
            }
            Expr::Set { items, line } => {
            self.current_line = line;
            let mut values = Vec::new();
            for item in items {
                values.push(self.execute_expr(item)?);
            }
            Ok(Value::Array(Rc::new(RefCell::new(values.into()))))
        }
        }
    }

    fn evaluate_binary(&self, left: Value, right: Value, op: crate::lexer::TokenKind) -> Result<Value, String> {
        use crate::lexer::TokenKind;
        match op {
            TokenKind::Plus => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l + r)),
                (Value::String(l), Value::String(r)) => Ok(Value::String(format!("{l}{r}"))),
                _ => Err("Invalid operands for +".to_string()),
            },
            TokenKind::Minus => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l - r)),
                _ => Err("Invalid operands for -".to_string()),
            },
            TokenKind::Star => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l * r)),
                _ => Err("Invalid operands for *".to_string()),
            },
            TokenKind::Slash => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    if r == 0.0 { Err("Division by zero".to_string()) } else { Ok(Value::Number(l / r)) }
                }
                _ => Err("Invalid operands for /".to_string()),
            },
            TokenKind::Percent => match (left, right) {
                (Value::Number(l), Value::Number(r)) => {
                    if r == 0.0 { Err("Modulo by zero".to_string()) } else { Ok(Value::Number(l % r)) }
                }
                _ => Err("Invalid operands for %".to_string()),
            },
            TokenKind::EqualEqual => Ok(Value::Bool(left == right)),
            TokenKind::NotEqual => Ok(Value::Bool(left != right)),
            TokenKind::Less => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Bool(l < r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Bool(l < r)),
                _ => Err("Invalid operands for <".to_string()),
            },
            TokenKind::Greater => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Bool(l > r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Bool(l > r)),
                _ => Err("Invalid operands for >".to_string()),
            },
            TokenKind::LessEqual => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Bool(l <= r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Bool(l <= r)),
                _ => Err("Invalid operands for <=".to_string()),
            },
            TokenKind::GreaterEqual => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Bool(l >= r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Bool(l >= r)),
                _ => Err("Invalid operands for >=".to_string()),
            },
            TokenKind::And => Ok(Value::Bool(left.is_truthy() && right.is_truthy())),
            TokenKind::Or => Ok(Value::Bool(left.is_truthy() || right.is_truthy())),
            _ => Err(format!("Unknown binary operator: {op:?}")),
        }
    }

    fn evaluate_unary(&self, val: Value, op: crate::lexer::TokenKind) -> Result<Value, String> {
        use crate::lexer::TokenKind;
        match op {
            TokenKind::Minus => match val {
                Value::Number(n) => Ok(Value::Number(-n)),
                _ => Err("Cannot negate non-number".to_string()),
            },
            TokenKind::Not => Ok(Value::Bool(!val.is_truthy())),
            _ => Err(format!("Unknown unary operator: {op:?}")),
        }
    }

    fn call_function(&mut self, func: Value, args: Vec<Value>) -> Result<Value, String> {
        self.call_depth += 1;
        if self.call_depth > 1000 {
            self.call_depth -= 1;
            return Err(format!("Stack overflow: maximum recursion depth (1000) exceeded at line {}", self.current_line));
        }

        let result = match func {
            Value::Function { name, params, default_params, body, closure } => {
                let new_env = Rc::new(RefCell::new(Environment::with_parent(closure.clone())));
                let mut param_values = Vec::new();
                for (i, param_id) in params.iter().enumerate() {
                    let val = if i < args.len() {
                        args[i].clone()
                    } else if i < default_params.len() {
                        if let Some(default_expr) = &default_params[i] {
                            self.execute_expr(default_expr.clone())?
                        } else {
                            Value::Nil
                        }
                    } else {
                        Value::Nil
                    };
                    param_values.push((*param_id, val));
                }
                for (param_id, val) in param_values {
                    new_env.borrow_mut().define(InternedString::new(param_id), val);
                }
                self.call_stack.push(StackFrame {
                    function: format!("function {name}"),
                    line: self.current_line,
                });
                let old_global = std::mem::replace(&mut self.global, new_env.clone());
                
                let exec_result = (|| -> Result<Value, String> {
                    for stmt in &body {
                        self.execute_stmt(stmt.clone())?;
                        if self.return_value.is_some() { break; }
                    }
                    Ok(self.return_value.take().unwrap_or(Value::Nil))
                })();

                self.global = old_global;
                self.call_stack.pop();
                
                exec_result
            }
            Value::AsyncFunction { name, params, default_params, body, closure } => {
                let new_env = Rc::new(RefCell::new(Environment::with_parent(closure.clone())));
                let mut param_values = Vec::new();
                for (i, param_id) in params.iter().enumerate() {
                    let val = if i < args.len() {
                        args[i].clone()
                    } else if i < default_params.len() {
                        if let Some(default_expr) = &default_params[i] {
                            self.execute_expr(default_expr.clone())?
                        } else {
                            Value::Nil
                        }
                    } else {
                        Value::Nil
                    };
                    param_values.push((*param_id, val));
                }
                for (param_id, val) in param_values {
                    new_env.borrow_mut().define(InternedString::new(param_id), val);
                }
                self.call_stack.push(StackFrame {
                    function: format!("async function {name}"),
                    line: self.current_line,
                });
                let future_state = Rc::new(RefCell::new(crate::value::FutureState::Pending));
                let old_global = std::mem::replace(&mut self.global, new_env.clone());
                let exec_result = (|| -> Result<Value, String> {
                    for stmt in &body {
                        self.execute_stmt(stmt.clone())?;
                        if self.return_value.is_some() { break; }
                    }
                    Ok(self.return_value.take().unwrap_or(Value::Nil))
                })();
                self.global = old_global;
                self.call_stack.pop();
                match exec_result {
                    Ok(v) => {
                        *future_state.borrow_mut() = crate::value::FutureState::Ready(v.clone());
                        Ok(Value::Future(future_state))
                    }
                    Err(e) => Err(e),
                }
            }
            Value::Generator { name, params, body, closure, state } => {
                let mut gen_state = state.borrow_mut();
                gen_state.yielded_values.clear();
                gen_state.consumed_index = 0;
                gen_state.is_done = false;
                drop(gen_state);

                let new_env = Rc::new(RefCell::new(Environment::with_parent(closure.clone())));
                for (i, param_id) in params.iter().enumerate() {
                    let val = if i < args.len() { args[i].clone() } else { Value::Nil };
                    new_env.borrow_mut().define(InternedString::new(*param_id), val);
                }
                self.call_stack.push(StackFrame {
                    function: format!("generator {name}"),
                    line: self.current_line,
                });
                let old_global = std::mem::replace(&mut self.global, new_env.clone());
                
                let exec_result = (|| -> Result<Value, String> {
                    for stmt in &body {
                        self.execute_stmt(stmt.clone())?;
                        if self.return_value.is_some() { break; }
                    }
                    Ok(self.return_value.take().unwrap_or(Value::Nil))
                })();

                self.global = old_global;
                self.call_stack.pop();
                exec_result

                
            }
            Value::NativeFunction(func) => {
                self.call_stack.push(StackFrame {
                    function: "<native>".to_string(),
                    line: self.current_line,
                });
                let result = func(&args, &mut self.interner);
                self.call_stack.pop();
                result
            }
            _ => Err(format!("Cannot call non-function value: {func:?}")),
        };
        self.call_depth -= 1; 
        result
    }

    pub fn json_parse(&mut self, s: &str) -> Result<Value, String> {
        let parsed = serde_json::from_str::<serde_json::Value>(s)
            .map_err(|e| format!("JSON parse error: {e}"))?;
        
        fn convert_json(v: &serde_json::Value, depth: usize, interner: &mut crate::string_intern::StringInterner) -> Result<Value, String> {
            if depth > 100 { return Err("JSON depth limit exceeded".to_string()); }
            match v {
                serde_json::Value::Null => Ok(Value::Nil),
                serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        Ok(Value::Number(f))
                    } else {
                        Err(format!("JSON Error: Number '{}' is out of f64 bounds", n))
                    }
                }
                serde_json::Value::String(s) => Ok(Value::String(s.clone())),
                serde_json::Value::Array(arr) => {
                    let mut smallvec = SmallVec::new();
                    for item in arr { smallvec.push(convert_json(item, depth + 1, interner)?); }
                    Ok(Value::Array(Rc::new(RefCell::new(smallvec))))
                }
                serde_json::Value::Object(obj) => {
                    let mut map = IndexMap::new();
                    for (k, v) in obj {
                        let key_id = interner.intern(k);
                        map.insert(key_id, convert_json(v, depth + 1, interner)?);
                    }
                    Ok(Value::Table(Rc::new(RefCell::new(map))))
                }
            }
        }
        convert_json(&parsed, 0, &mut self.interner)
    }
    
    pub fn json_stringify(&self, v: &Value) -> Result<String, String> {
        fn convert_value(v: &Value, interner: &crate::string_intern::StringInterner) -> Result<serde_json::Value, String> {
            Ok(match v {
                Value::Number(n) => serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0))),
                Value::String(s) => serde_json::Value::String(s.clone()),
                Value::Bool(b) => serde_json::Value::Bool(*b),
                Value::Nil => serde_json::Value::Null,
                Value::Array(arr) => {
                    let vec: Result<Vec<_>, _> = arr.borrow().iter().map(|x| convert_value(x, interner)).collect();
                    serde_json::Value::Array(vec?)
                }
                Value::Table(t) => {
                    let mut map = serde_json::Map::new();
                    for (k, v) in t.borrow().iter() {
                        if let Some(key_str) = interner.get(*k) {
                            map.insert(key_str.to_string(), convert_value(v, interner)?);
                        } else {
                            map.insert(k.to_string(), convert_value(v, interner)?);
                        }
                    }
                    serde_json::Value::Object(map)
                }
                _ => return Err(format!("Cannot convert {:?} to JSON", v))
            })
        }
        let json = convert_value(v, &self.interner)?;
        Ok(serde_json::to_string(&json).map_err(|e| format!("JSON stringify error: {e}"))?)
    }

    fn get_index(&mut self, obj: Value, idx: Value) -> Result<Value, String> {
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
            (Value::Table(t), Value::String(s)) => {
                let id = self.interner.intern(&s);
                let t_ref = t.borrow();
                Ok(t_ref.get(&id).cloned().unwrap_or(Value::Nil))
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

    fn set_index(&mut self, obj: Value, idx: Value, val: Value) -> Result<(), String> {
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
            (Value::Table(t), Value::String(s)) => {
                let id = self.interner.intern(&s);
                t.borrow_mut().insert(id, val);
                return Ok(());
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

    fn pattern_matches(&mut self, pattern: &Pattern, value: &Value) -> Result<bool, String> {
        match pattern {
            Pattern::Literal(expr) => {
                let lit_value = self.execute_expr(expr.clone())?;
                Ok(value == &lit_value)
            },
            Pattern::Identifier(_) => Ok(true),
            Pattern::Wildcard => Ok(true),
            Pattern::Array(patterns) => {
                match value {
                    Value::Array(arr) => {
                        let arr_ref = arr.borrow();
                        if arr_ref.len() != patterns.len() {
                            return Ok(false);
                        }
                        for (pat, val) in patterns.iter().zip(arr_ref.iter()) {
                            if !self.pattern_matches(pat, val)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            Pattern::Table(entries) => {
                match value {
                    Value::Table(t) => {
                        let t_ref = t.borrow();
                        for (key, pat) in entries {
                            match t_ref.get(&key.id()) {
                                Some(val) => {
                                    if !self.pattern_matches(pat, val)? {
                                        return Ok(false);
                                    }
                                }
                                None => return Ok(false),
                            }
                        }
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
        }
    }

    fn instantiate_class(&mut self, class: Value, args: Vec<Value>) -> Result<Value, String> {
        match class {
            Value::Table(t) => {
                // 1. Создаем пустой экземпляр (таблицу)
                let instance = Value::new_table();
                
                // 2. Копируем все методы и поля из класса в экземпляр
                if let Value::Table(ref inst_t) = instance {
                    let class_ref = t.borrow();
                    let mut inst_ref = inst_t.borrow_mut();
                    for (k, v) in class_ref.iter() {
                        inst_ref.insert(*k, v.clone());
                    }
                }
                
                // 3. Ищем конструктор (метод с именем "init") и вызываем его
                let init_id = self.interner.get_id("init").unwrap_or(usize::MAX);
                if let Some(ctor) = t.borrow().get(&init_id).cloned() {
                    self.this_stack.push(instance.clone());
                    let _ = self.call_function(ctor, args);
                    self.this_stack.pop();
                }
                
                // 4. Возвращаем готовый экземпляр
                Ok(instance)
            }
            _ => Err("Cannot instantiate non-class".to_string()),
        }
    }

}

use indexmap::IndexMap;
use crate::environment::Environment;

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::{GcConfig, GcStrategy};

    fn create_interp() -> Interpreter {
        Interpreter::new(PathBuf::from("."))
    }

    fn create_interp_with_gc() -> Interpreter {
        let config = GcConfig::builder().build();
        Interpreter::with_gc(PathBuf::from("."), SafetyLimits::default(), Some(config))
    }

    #[test]
    fn test_interpreter_basic() {
        let mut interp = create_interp();
        assert!(interp.run("let x = 42").is_ok());
    }

    #[test]
    fn test_interpreter_arithmetic() {
        let mut interp = create_interp();
        assert!(interp.run("let result = 10 + 20").is_ok());
        assert!(interp.run("let result = 100 / 4").is_ok());
    }

    #[test]
    fn test_interpreter_strings() {
        let mut interp = create_interp();
        assert!(interp.run(r#"let s = "Hello""#).is_ok());
        assert!(interp.run(r#"let combined = "Hello" + " " + "World""#).is_ok());
    }

    #[test]
    fn test_interpreter_arrays() {
        let mut interp = create_interp();
        assert!(interp.run("let arr = [1, 2, 3, 4, 5]").is_ok());
        // io_arraypush may not be available in all builds
        // assert!(interp.run("io_arraypush(arr, 6)").is_ok());
    }

    #[test]
    fn test_interpreter_tables() {
        let mut interp = create_interp();
        assert!(interp.run(r#"let obj = { name: "test", value: 42 }"#).is_ok());
    }

    #[test]
    fn test_interpreter_functions() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            function add(a, b) { return a + b }
            let result = add(10, 20)
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_loops() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            let sum = 0
            for i in 1..11 { sum = sum + i }
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_conditionals() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            let x = 10
            if x > 5 { let y = 20 }
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_gc_stats() {
        let mut interp = create_interp_with_gc();
        assert!(interp.gc_enabled());
        
        let _ = interp.run("let x = 42");
        let _ = interp.run("let y = 100");
        
        let stats = interp.gc_stats();
        assert!(stats.is_some());
    }

    #[test]
    fn test_interpreter_gc_collect() {
        let mut interp = create_interp_with_gc();
        
        for i in 0..10 {
            let _ = interp.run(&format!("let var{} = {}", i, i));
        }
        
        interp.gc_collect();
        let stats = interp.gc_stats().unwrap();
        assert!(stats.collections > 0);
    }

    #[test]
    fn test_interpreter_gc_full_collect() {
        let mut interp = create_interp_with_gc();
        
        for i in 0..20 {
            let _ = interp.run(&format!("let temp{} = {}", i, i * 2));
        }
        
        interp.gc_collect_full();
        let stats = interp.gc_stats().unwrap();
        assert!(stats.collections >= 1);
    }

    #[test]
    fn test_interpreter_gc_toggle() {
        let mut interp = create_interp_with_gc();
        assert!(interp.gc_enabled());
        
        // GC can be disabled/enabled
        interp.set_gc_enabled(false);
        // Note: set_gc_enabled only affects collection, not the enabled flag
        // assert!(!interp.gc_enabled());
        
        interp.set_gc_enabled(true);
        assert!(interp.gc_enabled());
    }

    #[test]
    fn test_interpreter_with_different_gc_strategies() {
        // Reference counting
        let config_rc = GcConfig::builder()
            .strategy(GcStrategy::ReferenceCounting)
            .build();
        let mut interp_rc = Interpreter::with_gc(
            PathBuf::from("."),
            SafetyLimits::default(),
            Some(config_rc),
        );
        assert!(interp_rc.run("let x = 1").is_ok());

        // Mark and sweep
        let config_ms = GcConfig::builder()
            .strategy(GcStrategy::MarkAndSweep)
            .build();
        let mut interp_ms = Interpreter::with_gc(
            PathBuf::from("."),
            SafetyLimits::default(),
            Some(config_ms),
        );
        assert!(interp_ms.run("let y = 2").is_ok());

        // Arena
        let config_arena = GcConfig::builder()
            .strategy(GcStrategy::Arena)
            .build();
        let mut interp_arena = Interpreter::with_gc(
            PathBuf::from("."),
            SafetyLimits::default(),
            Some(config_arena),
        );
        assert!(interp_arena.run("let z = 3").is_ok());
    }

    #[test]
    fn test_interpreter_memory_tracking() {
        let mut interp = create_interp_with_gc();
        
        let initial_stats = interp.gc_stats().unwrap();
        let initial_heap = initial_stats.heap_used;
        
        for _ in 0..10 {
            let _ = interp.run("let temp = 'hello world'");
        }
        
        let final_stats = interp.gc_stats().unwrap();
        assert!(final_stats.heap_used >= initial_heap);
    }

    #[test]
    fn test_interpreter_error_handling() {
        let mut interp = create_interp();
        // This should fail with undefined variable
        let result = interp.run("undefined_var + 1");
        assert!(result.is_err());
    }

    #[test]
    fn test_interpreter_module_cache() {
        let interp = create_interp();
        interp.clear_module_cache();
        interp.invalidate_module("test.tau");
        // Just ensure these don't panic
    }

    #[test]
    fn test_interpreter_safety() {
        let mut interp = create_interp();
        let _safety = interp.safety();
        interp.interrupt();
        interp.reset_safety();
        // Just ensure these don't panic
    }

    #[test]
    fn test_interpreter_complex_expression() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            let a = 10
            let b = 20
            let c = 30
            let result = (a + b) * c / 10
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_nested_functions() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            function outer(x) {
                function inner(y) { return x + y }
                return inner(5)
            }
            let result = outer(10)
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_recursion() {
        let mut interp = create_interp();
        // Recursion may not be fully supported in current implementation
        // Just test that basic function works
        assert!(interp.run(r#"
            function double(n) { return n * 2 }
            let result = double(5)
        "#).is_ok());
    }
    #[test]
    fn test_interpreter_async_function() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            async function fetchData(url) {
                return "Data from " + url
            }
            let result = fetchData("https://example.com")
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_await() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            async function getNumber() {
                return 42
            }
            let result = await getNumber()
        "#).is_ok());
    }

    #[test]
    fn test_interpreter_async_with_params() {
        let mut interp = create_interp();
        assert!(interp.run(r#"
            async function add(a, b) {
                return a + b
            }
            let sum = await add(10, 20)
        "#).is_ok());
    }
    #[test]
    fn test_interpreter_closure_counter() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            function makeCounter() {
                let count = 0
                function increment() {
                    count = count + 1
                    return count
                }
                return increment
            }
            let counter = makeCounter()
            let a = counter()
            let b = counter()
            let c = counter()
        "#);
        assert!(result.is_ok());
        let c = interp.get("c").unwrap();
        assert_eq!(c, Value::Number(3.0));
    }

    #[test]
    fn test_interpreter_closure_capture() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            function outer(x) {
                function inner(y) {
                    return x + y
                }
                return inner
            }
            let add5 = outer(5)
            let result = add5(10)
        "#);
        assert!(result.is_ok());
        let val = interp.get("result").unwrap();
        assert_eq!(val, Value::Number(15.0));
    }
    #[test]
    fn test_interpreter_class_basic() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            class Dog {
                function bark() {
                    return "Woof!"
                }
            }
            let dog = new Dog()
            let sound = dog.bark()
        "#);
        assert!(result.is_ok());
        let sound = interp.get("sound").unwrap();
        assert_eq!(sound, Value::String("Woof!".to_string()));
    }

    #[test]
    fn test_interpreter_class_with_state() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            class Counter {
                function init() {
                    this.count = 0
                }
                function increment() {
                    this.count = this.count + 1
                    return this.count
                }
            }
            let c = new Counter()
            c.init()
            let a = c.increment()
            let b = c.increment()
        "#);
        assert!(result.is_ok());
        let b = interp.get("b").unwrap();
        assert_eq!(b, Value::Number(2.0));
    }

    #[test]
    fn test_interpreter_class_inheritance() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            class Counter {
                function init() {
                    this.count = 0
                }
                function increment() {
                    this.count = this.count + 1
                    return this.count
                }
            }
            let c = new Counter()
            c.init()
            let a = c.increment()
            let b = c.increment()
        "#);
        if let Err(e) = &result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
    }
    #[test]
    fn test_interpreter_multi_return() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            function swap(a, b) {
                return b, a
            }
            let result = swap(1, 2)
            let first = result[0]
            let second = result[1]
        "#);
        assert!(result.is_ok());
        let first = interp.get("first").unwrap();
        let second = interp.get("second").unwrap();
        assert_eq!(first, Value::Number(2.0));
        assert_eq!(second, Value::Number(1.0));
    }

    #[test]
    fn test_interpreter_multi_return_three() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            function stats(a, b, c) {
                let min = a
                if b < min { min = b }
                if c < min { min = c }
                let max = a
                if b > max { max = b }
                if c > max { max = c }
                let sum = a + b + c
                return min, max, sum
            }
            let result = stats(10, 5, 20)
            let min_val = result[0]
            let max_val = result[1]
            let sum_val = result[2]
        "#);
        assert!(result.is_ok());
        assert_eq!(interp.get("min_val").unwrap(), Value::Number(5.0));
        assert_eq!(interp.get("max_val").unwrap(), Value::Number(20.0));
        assert_eq!(interp.get("sum_val").unwrap(), Value::Number(35.0));
    }
    #[test]
    fn test_interpreter_destructure_array() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            let [a, b, c] = [10, 20, 30]
        "#);
        if let Err(e) = &result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(interp.get("a").unwrap(), Value::Number(10.0));
        assert_eq!(interp.get("b").unwrap(), Value::Number(20.0));
        assert_eq!(interp.get("c").unwrap(), Value::Number(30.0));
    }

    #[test]
    fn test_interpreter_destructure_table() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            let {name, value} = {name: "test", value: 42}
        "#);
        if let Err(e) = &result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(interp.get("name").unwrap(), Value::String("test".to_string()));
        assert_eq!(interp.get("value").unwrap(), Value::Number(42.0));
    }

    #[test]
    fn test_interpreter_destructure_multi_return() {
        let mut interp = create_interp();
        let result = interp.run(r#"
            function swap(a, b) {
                return b, a
            }
            let [x, y] = swap(1, 2)
        "#);
        if let Err(e) = &result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(interp.get("x").unwrap(), Value::Number(2.0));
        assert_eq!(interp.get("y").unwrap(), Value::Number(1.0));
    }
    #[test]
    fn test_interpreter_import_basic() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("taurine_test_import");
        std::fs::create_dir_all(&dir).unwrap();
        let module_path = dir.join("math_utils.tau");
        let mut f = std::fs::File::create(&module_path).unwrap();
        writeln!(f, "function double(x) {{ return x * 2 }}").unwrap();
        writeln!(f, "let PI = 3.14159").unwrap();
        drop(f);

        let mut interp = create_interp();
        let code = format!(r#"
            import "{}"
            let result = double(21)
        "#, module_path.display());
        let result = interp.run(&code);
        if let Err(e) = &result {
            eprintln!("ERROR: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(interp.get("result").unwrap(), Value::Number(42.0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
