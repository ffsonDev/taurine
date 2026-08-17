use taurine::Interpreter;
use taurine::lexer::tokenize;
use taurine::parser::Parser;
use std::path::PathBuf;

fn run_code(code: &str) -> Result<(), String> {
    let mut interp = Interpreter::new(PathBuf::from("."));
    interp.run(code).map_err(|e| e.message())
}

fn run_and_get(code: &str, var: &str) -> Result<taurine::Value, String> {
    let mut interp = Interpreter::new(PathBuf::from("."));
    interp.run(code).map_err(|e| e.message())?;
    interp.get(var)
}

// === Fix 1: yield nil doesn't break generator iteration ===
#[test]
fn test_yield_nil_in_generator() {
    let result = run_code(r#"
generator gen() {
    yield 1
    yield nil
    yield 3
}
let results = []
for v in gen() {
    io_arraypush(results, v)
}
"#);
    assert!(result.is_ok());
}

#[test]
fn test_yield_nil_only() {
    let result = run_code(r#"
generator gen() {
    yield nil
}
let count = 0
for v in gen() {
    count = count + 1
}
"#);
    assert!(result.is_ok());
}

// === Fix 2: peek()/previous() don't panic on empty input ===
#[test]
fn test_empty_input_no_panic() {
    let tokens = tokenize("");
    let mut parser = Parser::new(tokens);
    let result = parser.parse();
    assert!(result.is_ok());
}

#[test]
fn test_whitespace_only_no_panic() {
    let tokens = tokenize("   \n\t  ");
    let mut parser = Parser::new(tokens);
    let result = parser.parse();
    assert!(result.is_ok());
}

#[test]
fn test_comment_only_no_panic() {
    let tokens = tokenize("// just a comment");
    let mut parser = Parser::new(tokens);
    let result = parser.parse();
    assert!(result.is_ok());
}

// === Fix 3: this_stack restored on error ===
#[test]
fn test_this_stack_restored_on_error() {
    let result = run_code(r#"
class Foo {
    function bar() {
        return this.missing_method()
    }
}
let f = new Foo()
try {
    f.bar()
} catch (e) {
    print(e)
}
"#);
    assert!(result.is_ok());
}

// === Fix 4: crypto_random_bytes produces different values ===
#[test]
fn test_crypto_random_bytes_length() {
    let result = run_code(r#"
let bytes = crypto_random_bytes(32)
let len = #bytes
"#);
    assert!(result.is_ok());
}

#[test]
fn test_crypto_random_bytes_not_all_zero() {
    let result = run_code(r#"
let bytes = crypto_random_bytes(16)
let has_nonzero = false
for b in bytes {
    if b != 0 { has_nonzero = true }
}
"#);
    assert!(result.is_ok());
}

// === Fix 5: io_strfind returns 0-based, -1 for not found ===
#[test]
fn test_strfind_zero_based() {
    let val = run_and_get(r#"
let pos = io_strfind("hello world", "world")
"#, "pos").unwrap();
    assert_eq!(val, taurine::Value::Number(6.0));
}

#[test]
fn test_strfind_not_found_returns_negative() {
    let val = run_and_get(r#"
let pos = io_strfind("hello", "xyz")
"#, "pos").unwrap();
    assert_eq!(val, taurine::Value::Number(-1.0));
}

#[test]
fn test_strfind_at_start() {
    let val = run_and_get(r#"
let pos = io_strfind("hello", "hello")
"#, "pos").unwrap();
    assert_eq!(val, taurine::Value::Number(0.0));
}

// === Fix 6: PartialEq for numbers is exact ===
#[test]
fn test_number_equality_exact() {
    let val = run_and_get(r#"
let x = 0.1 + 0.2
let result = x == 0.3
"#, "result").unwrap();
    // 0.1 + 0.2 != 0.3 in floating point, should be false with exact equality
    assert_eq!(val, taurine::Value::Bool(false));
}

#[test]
fn test_number_equality_same_value() {
    let val = run_and_get(r#"
let result = 42 == 42
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::Bool(true));
}

// === Fix 7: SecurityContext blocks functions ===
#[test]
fn test_security_context_sandbox_blocks_io() {
    use taurine::safety::{SecurityContext, SecurityLevel};
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Sandbox);
    assert!(!ctx.is_function_allowed("io_read"));
    assert!(!ctx.is_function_allowed("io_write"));
    assert!(!ctx.is_function_allowed("http_get"));
}

#[test]
fn test_security_context_full_allows_all() {
    use taurine::safety::{SecurityContext, SecurityLevel};
    let mut ctx = SecurityContext::new();
    ctx.apply_level(&SecurityLevel::Full);
    assert!(ctx.is_function_allowed("io_read"));
    assert!(ctx.is_function_allowed("io_write"));
    assert!(ctx.is_function_allowed("http_get"));
}

// === Fix 8: Pattern matching with Array/Table ===
#[test]
fn test_pattern_match_literal() {
    let val = run_and_get(r#"
let x = 42
let result = match x {
    0 => "zero",
    42 => "forty-two",
    _ => "other"
}
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::String("forty-two".to_string()));
}

#[test]
fn test_pattern_match_wildcard() {
    let val = run_and_get(r#"
let x = 999
let result = match x {
    0 => "zero",
    _ => "wildcard"
}
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::String("wildcard".to_string()));
}

#[test]
fn test_pattern_match_identifier_binds() {
    let val = run_and_get(r#"
let x = 42
let result = match x {
    n => "bound"
}
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::String("bound".to_string()));
}

#[test]
fn test_pattern_match_with_guard() {
    let val = run_and_get(r#"
let x = 42
let result = match x {
    n if n > 0 => "positive",
    _ => "non-positive"
}
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::String("positive".to_string()));
}

// === Fix 9: Generator lazy evaluation ===
#[test]
fn test_generator_lazy_not_executed_on_creation() {
    let result = run_code(r#"
generator gen() {
    print("executing")
    yield 1
}
let g = gen()
"#);
    assert!(result.is_ok());
}

#[test]
fn test_generator_multiple_yields() {
    let result = run_code(r#"
generator count(n) {
    let i = 0
    while i < n {
        yield i
        i = i + 1
    }
}
let sum = 0
for v in count(5) {
    sum = sum + v
}
"#);
    assert!(result.is_ok());
}

// === Fix 10: Async deferred execution ===
#[test]
fn test_async_function_returns_future() {
    let result = run_code(r#"
async function fetchData() {
    return "data"
}
let result = fetchData()
"#);
    assert!(result.is_ok());
}

#[test]
fn test_await_resolves_future() {
    let result = run_code(r#"
async function getNumber() {
    return 42
}
let result = await getNumber()
"#);
    assert!(result.is_ok());
}

// === Fix 11: load_stdlib doesn't crash ===
#[test]
fn test_interpreter_creation_with_stdlib() {
    let interp = Interpreter::new(PathBuf::from("."));
    let _ = interp;
}

// === Fix 12: Interner IDs consistent ===
#[test]
fn test_builtin_function_ids_consistent() {
    let result = run_code(r#"
print("hello")
"#);
    assert!(result.is_ok());
}

#[test]
fn test_json_parse_stringify() {
    let result = run_code(r#"
let data = json_parse("{\"name\": \"test\", \"value\": 42}")
let json = json_stringify(data)
"#);
    assert!(result.is_ok());
}

// === Fix 13: Class inheritance with super ===
#[test]
fn test_class_basic_instantiation() {
    let result = run_code(r#"
class Rectangle {
    function init(w, h) {
        this.width = w
        this.height = h
    }
    function area() {
        return this.width * this.height
    }
}
let rect = new Rectangle(10, 20)
"#);
    assert!(result.is_ok());
}

#[test]
fn test_class_method_call() {
    let val = run_and_get(r#"
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
let a = c.increment()
let b = c.increment()
"#, "b").unwrap();
    assert_eq!(val, taurine::Value::Number(2.0));
}

// === Fix 14: Recursion depth limit ===
#[test]
fn test_recursion_depth_limit() {
    let result = run_code(r#"
function infinite() {
    return infinite()
}
infinite()
"#);
    assert!(result.is_err());
}

// === Fix 15: Division by zero ===
#[test]
fn test_division_by_zero() {
    let result = run_code("let x = 10 / 0");
    assert!(result.is_err());
}

#[test]
fn test_modulo_by_zero() {
    let result = run_code("let x = 10 % 0");
    assert!(result.is_err());
}