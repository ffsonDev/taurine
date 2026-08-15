use std::time::Instant;
use taurine::{Interpreter, Optimizer, SafetyLimits, StringInterner};
use taurine::parser::Parser;
use taurine::lexer::{tokenize, tokenize_with_interner};
use std::path::PathBuf;

fn no_timeout_limits() -> SafetyLimits {
    SafetyLimits {
        timeout: None,
        ..Default::default()
    }
}

fn main() {
    println!("=== Taurine Benchmarks ===\n");

    benchmark_arithmetic();
    benchmark_loop();
    benchmark_function_calls();
    benchmark_array();
    benchmark_table();
    benchmark_string();
    benchmark_optimizer();
    benchmark_string_interning();
    benchmark_arena_allocation();
    benchmark_lexer();
    benchmark_parser();
    benchmark_interpreter_vs_bytecode();
    benchmark_gc_cycle_detection();
    benchmark_json_operations();
    benchmark_fstring();

    println!("\n=== All benchmarks completed ===");
}

fn benchmark_arithmetic() {
    let code = r#"..."#;
    let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = interp.run(code);
    }
    let _elapsed = start.elapsed();
}

fn benchmark_loop() {
    println!("\n[2] Loop Performance");

    let code = r#"
let sum = 0
for i in 1..1000 {
    sum = sum + 1
}
print(f"Loop sum: {sum}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (1000 loop): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_function_calls() {
    println!("\n[3] Function Calls");

    let code = r#"
function add(a, b) {
    return a + b
}
let result = 0
for i in 1..100 {
    result = add(i, i + 1)
}
print(f"Function result: {result}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (100 calls): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_array() {
    println!("\n[4] Array Operations");

    let code = r#"
let arr = []
for i in 1..100 {
    io_arraypush(arr, i)
}
let len = #arr
print(f"Array length: {len}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (100 push): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_table() {
    println!("\n[5] Table Operations");

    let code = r#"
let table = {}
for i in 1..50 {
    table["key_" + tostring(i)] = i * 2
}
let val = table["key_25"]
print(f"Table value: {val}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (50 inserts): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_string() {
    println!("\n[6] String Operations");

    let code = r#"
let str = "hello"
for i in 1..50 {
    str = str + " world"
}
print(f"String length: {#str}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (50 concat): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_optimizer() {
    println!("\n[7] Optimizer Comparison");

    let code = r#"
let x = 10
let y = 20
let z = x + y
let result = z * 2
if true {
    print(f"Optimized: {result}")
} else {
    print("unreachable")
}
"#;

    let tokens = taurine::tokenize(code);
    let mut parser = taurine::Parser::new(tokens);
    let program = parser.parse().unwrap();

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::new(PathBuf::from("."));
        let _ = interp.interpret(program.clone());
    }
    let elapsed_no_opt = start.elapsed();

    let tokens = taurine::tokenize(code);
    let mut parser = taurine::Parser::new(tokens);
    let program = parser.parse().unwrap();
    let mut optimizer = Optimizer::new();
    let optimized = optimizer.optimize(program);

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::new(PathBuf::from("."));
        let _ = interp.interpret(optimized.clone());
    }
    let elapsed_opt = start.elapsed();

    println!("    Without optimization: {:.2?} ({:.2} µs/iter)", elapsed_no_opt, elapsed_no_opt.as_micros() as f64 / 100.0);
    println!("    With optimization:    {:.2?} ({:.2} µs/iter)", elapsed_opt, elapsed_opt.as_micros() as f64 / 100.0);

    if elapsed_opt < elapsed_no_opt {
        let speedup = elapsed_no_opt.as_secs_f64() / elapsed_opt.as_secs_f64();
        println!("    Speedup: {:.2}x faster with optimizations", speedup);
    }
}

fn benchmark_string_interning() {
    println!("\n[8] String Interning");

    let code = r#"
let variable_name = 10
let another_variable = 20
let variable_name_copy = variable_name
let result = variable_name + another_variable
print(f"Result: {result}")
"#;

    let start = Instant::now();
    for _ in 0..1000 {
        let _tokens = tokenize(code);
    }
    let elapsed_no_intern = start.elapsed();

    let start = Instant::now();
    for _ in 0..1000 {
        let mut interner = StringInterner::with_capacity(64);
        let _tokens = tokenize_with_interner(code, &mut interner);
    }
    let elapsed_with_intern = start.elapsed();

    println!("    Without interning: {:.2?} ({:.2} µs/iter)", elapsed_no_intern, elapsed_no_intern.as_micros() as f64 / 1000.0);
    println!("    With interning:    {:.2?} ({:.2} µs/iter)", elapsed_with_intern, elapsed_with_intern.as_micros() as f64 / 1000.0);

    let mut interner = StringInterner::with_capacity(64);
    let _tokens = tokenize_with_interner(code, &mut interner);
    println!("    Interner memory: {} bytes", interner.memory_usage());
}

fn benchmark_arena_allocation() {
    println!("\n[9] Arena Allocation");

    let code = r#"
let a = 1
let b = 2
let c = 3
let d = a + b
let e = c * d
let f = e - a
function test(x, y) {
    return x + y
}
let result = test(d, e)
"#;

    let start = Instant::now();
    for _ in 0..1000 {
        let tokens = tokenize(code);
        let mut parser = Parser::new(tokens);
        let _ = parser.parse();
    }
    let elapsed = start.elapsed();
    println!("    Parser: {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 1000.0);
}

fn benchmark_lexer() {
    println!("\n[10] Lexer Performance");

    let small_code = r#"let x = 10 let y = 20 print(x + y)"#;

    let medium_code = r#"
function fibonacci(n) {
    if (n <= 1) { return n }
    return fibonacci(n - 1) + fibonacci(n - 2)
}
let result = fibonacci(10)
print(f"Fibonacci(10) = {result}")
"#;

    let large_code = r#"
class Calculator {
    function add(a, b) { return a + b }
    function sub(a, b) { return a - b }
    function mul(a, b) { return a * b }
    function div(a, b) { return a / b }
}
let calc = new Calculator()
let result = calc.add(10, 20)
result = calc.mul(result, 2)
print(f"Result: {result}")
"#;

    println!("    Small program ({} chars):", small_code.len());
    let start = Instant::now();
    for _ in 0..10000 {
        let _tokens = tokenize(small_code);
    }
    let elapsed = start.elapsed();
    println!("      {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 10000.0);

    println!("    Medium program ({} chars):", medium_code.len());
    let start = Instant::now();
    for _ in 0..1000 {
        let _tokens = tokenize(medium_code);
    }
    let elapsed = start.elapsed();
    println!("      {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 1000.0);

    println!("    Large program ({} chars):", large_code.len());
    let start = Instant::now();
    for _ in 0..100 {
        let _tokens = tokenize(large_code);
    }
    let elapsed = start.elapsed();
    println!("      {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_parser() {
    println!("\n[11] Parser Performance");

    let code = r#"
function factorial(n) {
    if (n <= 1) { return 1 }
    return n * factorial(n - 1)
}
let result = factorial(5)
print(f"Factorial(5) = {result}")
"#;

    let start = Instant::now();
    for _ in 0..1000 {
        let tokens = tokenize(code);
        let mut parser = Parser::new(tokens);
        let _ = parser.parse();
    }
    let elapsed = start.elapsed();
    println!("    Standard parser: {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 1000.0);

    let start = Instant::now();
    for _ in 0..1000 {
        let mut interner = StringInterner::with_capacity(64);
        let tokens = tokenize_with_interner(code, &mut interner);
        let mut parser = Parser::with_interner(tokens, interner);
        let _ = parser.parse();
    }
    let elapsed = start.elapsed();
    println!("    With interner:   {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 1000.0);
}

fn benchmark_interpreter_vs_bytecode() {
    println!("\n[12] Interpreter vs Bytecode");
    println!("  Interpreter: (use benchmarks 2-6 for interpreter performance)");
    println!("  Bytecode:    (skipped - limited support)");
}

fn benchmark_gc_cycle_detection() {
    println!("\n[13] GC Cycle Detection");

    let start = Instant::now();
    for _ in 0..100 {
        let config = taurine::gc::GcConfig::builder()
            .strategy(taurine::gc::GcStrategy::ReferenceCounting)
            .enable_cycle_detection(true)
            .build();
        let mut gc = taurine::gc::GarbageCollector::new(config);
        let a = gc.allocate(100);
        let b = gc.allocate(100);
        gc.add_child(a, b);
        gc.add_child(b, a);
        gc.collect_full();
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (cycle detect): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_json_operations() {
    println!("\n[14] JSON Operations");

    let code = r#"
let data = json_parse("{\"name\": \"test\", \"value\": 42, \"items\": [1, 2, 3]}")
let json = json_stringify(data)
print(f"JSON length: {#json}")
"#;

    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    100 iterations (parse+stringify): {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 100.0);
}

fn benchmark_fstring() {
    println!("\n[15] F-String Interpolation");

    let code = r#"
let name = "Taurine"
let version = 2
let msg = f"Hello from {name} v{version}!"
print(msg)
"#;

    let start = Instant::now();
    for _ in 0..1000 {
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let _ = interp.run(code);
    }
    let elapsed = start.elapsed();
    println!("    1000 iterations: {:.2?} ({:.2} µs/iter)", elapsed, elapsed.as_micros() as f64 / 1000.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_runs() {
        let code = r#"
            let x = 10
            let y = 20
            print(x + y)
            "#;
        let mut interp = Interpreter::with_limits(PathBuf::from("."), no_timeout_limits());
        let result = interp.run(code);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimizer() {
        let code = r#"
let x = 10 + 20
print(x)
"#;
        let tokens = tokenize(code);
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let mut optimizer = Optimizer::new();
        let _ = optimizer.optimize(program);
    }

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::new();
        let id1 = interner.intern("hello");
        let id2 = interner.intern("hello");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_arena() {
        use taurine::ast::Expr;
        let arena = AstArena::new();
        let expr = arena.alloc_expr(Expr::Number(42.0));
        assert!(matches!(expr, Expr::Number(42.0)));
    }
}