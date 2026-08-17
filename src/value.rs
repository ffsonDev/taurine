//! Value types
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use indexmap::IndexMap;
use smallvec::SmallVec;
use crate::environment::Environment;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    Table(Rc<RefCell<IndexMap<usize, Value>>>),
    Array(Rc<RefCell<SmallVec<[Value; 4]>>>),
    Range {
        start: f64,
        end: f64,
    },
    Function {
        name: usize,
        params: Vec<usize>,
        default_params: Vec<Option<crate::ast::Expr>>,
        body: Vec<crate::ast::Stmt>,
        closure: Rc<RefCell<Environment>>,
    },
    AsyncFunction {
        name: usize,
        params: Vec<usize>,
        default_params: Vec<Option<crate::ast::Expr>>,
        body: Vec<crate::ast::Stmt>,
        closure: Rc<RefCell<Environment>>,
    },
    Generator {
        name: usize,
        params: Vec<usize>,
        body: Vec<crate::ast::Stmt>,
        closure: Rc<RefCell<Environment>>,
        state: Rc<RefCell<GeneratorState>>,
    },
    NativeFunction(Rc<dyn Fn(&[Value], &mut crate::string_intern::StringInterner) -> Result<Value, String>>),
    Future(Rc<RefCell<FutureState>>),
    Error(String),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "Number({n})"),
            Value::String(s) => write!(f, "String({s:?})"),
            Value::Bool(b) => write!(f, "Bool({b})"),
            Value::Nil => write!(f, "Nil"),
            Value::Table(t) => write!(f, "Table({:?})", t.borrow()),
            Value::Array(arr) => write!(f, "Array({:?})", arr.borrow()),
            Value::Range { start, end } => write!(f, "Range({start}..{end})"),
            Value::Function { name, .. } => write!(f, "Function({name})"),
            Value::AsyncFunction { name, .. } => write!(f, "AsyncFunction({name})"),
            Value::Generator { name, .. } => write!(f, "Generator({name})"),
            Value::NativeFunction(_) => write!(f, "NativeFunction(<fn>)"),
            Value::Future(state) => write!(f, "Future({:?})", state.borrow()),
            Value::Error(msg) => write!(f, "Error({msg:?})"),
        }
    }
}

/// Generator execution state with lazy evaluation support
#[derive(Clone, Debug)]
pub struct GeneratorState {
    /// Values that have been yielded so far
    pub yielded_values: Vec<Value>,
    /// Index of next value to consume
    pub consumed_index: usize,
    /// Whether the generator has finished execution
    pub is_done: bool,
    /// Current execution state for lazy evaluation
    pub execution_state: GeneratorExecutionState,
}

/// Execution state for lazy generator evaluation
#[derive(Clone, Debug)]
pub enum GeneratorExecutionState {
    /// Generator hasn't started yet
    NotStarted,
    /// Generator is currently suspended at a yield point
    Suspended {
        /// Remaining statements to execute
        remaining_body: Vec<crate::ast::Stmt>,
        /// Current environment/closure
        closure: std::rc::Rc<std::cell::RefCell<crate::environment::Environment>>,
    },
    /// Generator has completed
    Finished,
}

impl Default for GeneratorState {
    fn default() -> Self {
        Self {
            yielded_values: Vec::new(),
            consumed_index: 0,
            is_done: false,
            execution_state: GeneratorExecutionState::NotStarted,
        }
    }
}

/// Future state for async/await
#[derive(Clone, Debug)]
pub enum FutureState {
    Pending,
    Ready(Value),
    Deferred {
        body: Vec<crate::ast::Stmt>,
        closure: std::rc::Rc<std::cell::RefCell<crate::environment::Environment>>,
        args: Vec<Value>,
    },
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Error(a), Value::Error(b)) => a == b,
            (Value::Range { start: s1, end: e1 }, Value::Range { start: s2, end: e2 }) => {
                s1 == s2 && e1 == e2
            }
            (Value::Future(_), Value::Future(_)) => false,
            (Value::Generator { .. }, Value::Generator { .. }) => false,
            (Value::AsyncFunction { name: n1, .. }, Value::AsyncFunction { name: n2, .. }) => n1 == n2,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nil => write!(f, "nil"),
            Value::Table(t) => {
                let table = t.borrow();
                if table.is_empty() {
                    write!(f, "{{}}")
                } else {
                    let items: Vec<String> = table.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                    write!(f, "{{{}}}", items.join(", "))
                }
            }
            Value::Array(arr) => {
                let array = arr.borrow();
                let items: Vec<String> = array.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Range { start, end } => write!(f, "{}..{}", *start as i64, *end as i64),
            Value::Function { name, .. } => write!(f, "<function {name}>"),
            Value::AsyncFunction { name, .. } => write!(f, "<async function {name}>"),
            Value::Generator { name, .. } => write!(f, "<generator {name}>"),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::Future(state) => {
                let s = state.borrow();
                match &*s {
                    FutureState::Pending => write!(f, "<future pending>"),
                    FutureState::Ready(v) => write!(f, "<future {v}>"),
                    &FutureState::Deferred { .. } => todo!(),
                }
            }
            Value::Error(msg) => write!(f, "error: {msg}"),
        }
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil | Value::Bool(false) => false,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn new_table() -> Self {
        Value::Table(Rc::new(RefCell::new(IndexMap::new())))
    }

    pub fn new_array() -> Self {
        Value::Array(Rc::new(RefCell::new(SmallVec::new())))
    }
}
