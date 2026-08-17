pub mod lexer;
pub mod ast;
pub mod parser;
pub mod value;
pub mod environment;
pub mod interpreter;
pub mod optimizer;
pub mod formatter;
pub mod ffi;
pub mod error;
pub mod safety;
pub mod string_intern;
pub mod bytecode;
pub mod arena;
pub mod gc;
pub mod benchmark;
#[cfg(feature = "async")]
pub mod async_rt;

pub use interpreter::Interpreter;
pub use interpreter::execute::ControlFlow;
pub use interpreter::execute::StackFrame;
pub use value::Value;
pub use environment::Environment;
pub use ast::{Expr, Stmt, Program};
pub use parser::Parser;
pub use lexer::{tokenize, tokenize_with_interner, Token, TokenKind};
#[cfg(feature = "async")]
pub use async_rt::{AsyncRuntime, FutureValue};
pub use optimizer::Optimizer;
pub use formatter::Formatter;
pub use error::TaurineError;
pub use safety::{SafetyLimits, SafetyContext, SecurityLevel, Permissions, SecurityError, ResourceTracker, InputValidator};
pub use string_intern::{StringInterner, InternedString};
pub use bytecode::{Compiler, VirtualMachine, BytecodeProgram, BytecodeFunction, OpCode, Instruction};
pub use arena::AstArena;
pub use gc::{GcStrategy, GarbageCollector, GcStats, GcPtr, GcConfig};
pub use benchmark::{BenchmarkResult, run_all_benchmarks};
