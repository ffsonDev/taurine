use taurine::bytecode::{Compiler, VirtualMachine};
use taurine::parser::Parser;
use taurine::lexer::tokenize;

fn run_bytecode(code: &str) -> Result<(), String> {
    let tokens = tokenize(code);
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    let compiler = Compiler::new();
    let bytecode = compiler.compile(program);
    let mut vm = VirtualMachine::new();
    vm.execute(&bytecode)
}

#[test]
fn test_bytecode_arithmetic() {
    assert!(run_bytecode("let x = 10 + 20").is_ok());
    assert!(run_bytecode("let x = 100 - 50").is_ok());
    assert!(run_bytecode("let x = 10 * 5").is_ok());
    assert!(run_bytecode("let x = 100 / 4").is_ok());
    assert!(run_bytecode("let x = 10 % 3").is_ok());
}

#[test]
fn test_bytecode_comparison() {
    assert!(run_bytecode("let x = 10 > 5").is_ok());
    assert!(run_bytecode("let x = 10 < 20").is_ok());
    assert!(run_bytecode("let x = 10 == 10").is_ok());
    assert!(run_bytecode("let x = 10 != 5").is_ok());
    assert!(run_bytecode("let x = 10 >= 10").is_ok());
    assert!(run_bytecode("let x = 10 <= 10").is_ok());
}

#[test]
fn test_bytecode_variables() {
    assert!(run_bytecode("let x = 42").is_ok());
    assert!(run_bytecode("let x = 42
let y = x + 1").is_ok());
}

#[test]
fn test_bytecode_strings() {
    assert!(run_bytecode(r#"let s = "hello""#).is_ok());
    assert!(run_bytecode(r#"let s = "hello" + " world""#).is_ok());
}

#[test]
fn test_bytecode_arrays() {
    assert!(run_bytecode("let arr = [1, 2, 3]").is_ok());
    assert!(run_bytecode("let arr = [1, 2, 3]
let x = arr[0]").is_ok());
}
#[test]
fn test_bytecode_tables() {
    assert!(run_bytecode(r#"let obj = { name: "test", value: 42 }"#).is_ok());
}

#[test]
fn test_bytecode_if_else() {
    assert!(run_bytecode("if true { let x = 1 } else { let x = 2 }").is_ok());
    assert!(run_bytecode("if false { let x = 1 } else { let x = 2 }").is_ok());
    assert!(run_bytecode("if false { let x = 1 }").is_ok());
}

#[test]
fn test_bytecode_while_loop() {
    assert!(run_bytecode("let x = 0
while x < 5 { x = x + 1 }").is_ok());
}

#[test]
fn test_bytecode_function_declaration() {
    assert!(run_bytecode("function add(a, b) { return a + b }").is_ok());
}

#[test]
fn test_bytecode_function_call() {
    assert!(run_bytecode(r#"
function add(a, b) { return a + b }
let result = add(10, 20)
"#).is_ok());
}

#[test]
fn test_bytecode_lambda() {
    assert!(run_bytecode("let double = (x) => x * 2").is_ok());
}

#[test]
fn test_bytecode_unary() {
    assert!(run_bytecode("let x = -5").is_ok());
    assert!(run_bytecode("let x = not true").is_ok());
}

#[test]
fn test_bytecode_null_coalesce() {
    assert!(run_bytecode("let x = nil ?? 42").is_ok());
    assert!(run_bytecode("let x = 10 ?? 42").is_ok());
}

#[test]
fn test_bytecode_for_in() {
    assert!(run_bytecode("for i in 1..5 { let x = i }").is_ok());
}

#[test]
fn test_bytecode_class() {
    assert!(run_bytecode(r#"
class Dog {
    function bark() { return "Woof" }
}
"#).is_ok());
}

#[test]
fn test_bytecode_empty_program() {
    assert!(run_bytecode("").is_ok());
}

#[test]
fn test_bytecode_multiple_statements() {
    assert!(run_bytecode("let a = 1
let b = 2
let c = a + b").is_ok());
}

#[test]
fn test_bytecode_nested_if() {
    assert!(run_bytecode("if true { if false { let x = 1 } else { let x = 2 } }").is_ok());
}

#[test]
fn test_bytecode_index_access() {
    assert!(run_bytecode("let arr = [1, 2, 3]
let x = arr[0]").is_ok());
}

#[test]
fn test_bytecode_property_access() {
    assert!(run_bytecode(r#"let obj = { name: "test" }
let x = obj.name"#).is_ok());
}

#[test]
fn test_bytecode_break_continue() {
    assert!(run_bytecode("let x = 0
while true { x = x + 1
if x > 5 { break } }").is_ok());
}

#[test]
fn test_bytecode_return_nil() {
    assert!(run_bytecode("function f() { return }").is_ok());
}

#[test]
fn test_bytecode_complex_expression() {
    assert!(run_bytecode("let result = (1 + 2) * 3 - 4 / 2").is_ok());
}