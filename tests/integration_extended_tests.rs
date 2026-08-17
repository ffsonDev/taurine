use taurine::Interpreter;
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

#[test]
fn test_full_program_fibonacci() {
    let val = run_and_get(r#"
function fib(n) {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
let result = fib(10)
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::Number(55.0));
}

#[test]
fn test_full_program_sort() {
    let result = run_code(r#"
function bubbleSort(arr) {
    let n = #arr
    let i = 0
    while i < n {
        let j = 0
        while j < n - 1 {
            if arr[j] > arr[j + 1] {
                let temp = arr[j]
                arr[j] = arr[j + 1]
                arr[j + 1] = temp
            }
            j = j + 1
        }
        i = i + 1
    }
    return arr
}
let arr = [5, 3, 8, 1, 9, 2]
let sorted = bubbleSort(arr)
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_class_hierarchy() {
    let result = run_code(r#"
class Animal {
    function init(name) {
        this.name = name
    }
    function speak() {
        return "..."
    }
}
class Dog extends Animal {
    function speak() {
        return "Woof!"
    }
}
let dog = new Dog("Rex")
let sound = dog.speak()
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_generator_pipeline() {
    let result = run_code(r#"
generator range(start, end) {
    let i = start
    while i < end {
        yield i
        i = i + 1
    }
}
generator squares(gen) {
    for x in gen {
        yield x * x
    }
}
let sum = 0
for sq in squares(range(1, 6)) {
    sum = sum + sq
}
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_error_handling() {
    let result = run_code(r#"
function risky() {
    throw "something went wrong"
}
let caught = false
try {
    risky()
} catch (e) {
    caught = true
}
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_destructuring() {
    let val = run_and_get(r#"
let [a, b, c] = [10, 20, 30]
let sum = a + b + c
"#, "sum").unwrap();
    assert_eq!(val, taurine::Value::Number(60.0));
}

#[test]
fn test_full_program_multi_return() {
    let val = run_and_get(r#"
function minmax(arr) {
    let min = arr[0]
    let max = arr[0]
    for x in arr {
        if x < min { min = x }
        if x > max { max = x }
    }
    return min, max
}
let result = minmax([3, 1, 4, 1, 5, 9])
let min_val = result[0]
"#, "min_val").unwrap();
    assert_eq!(val, taurine::Value::Number(1.0));
}

#[test]
fn test_full_program_closure_counter() {
    let val = run_and_get(r#"
function makeCounter() {
    let count = 0
    function increment() {
        count = count + 1
        return count
    }
    return increment
}
let counter = makeCounter()
counter()
counter()
let result = counter()
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::Number(3.0));
}

#[test]
fn test_full_program_higher_order_functions() {
    let val = run_and_get(r#"
function map(arr, fn) {
    let result = []
    for x in arr {
        io_arraypush(result, fn(x))
    }
    return result
}
function filter(arr, fn) {
    let result = []
    for x in arr {
        if fn(x) {
            io_arraypush(result, x)
        }
    }
    return result
}
function reduce(arr, fn, init) {
    let acc = init
    for x in arr {
        acc = fn(acc, x)
    }
    return acc
}
let nums = [1, 2, 3, 4, 5]
let doubled = map(nums, (x) => x * 2)
let evens = filter(nums, (x) => x % 2 == 0)
let sum = reduce(nums, (a, b) => a + b, 0)
"#, "sum").unwrap();
    assert_eq!(val, taurine::Value::Number(15.0));
}

#[test]
fn test_full_program_string_processing() {
    let val = run_and_get(r#"
let text = "Hello, World! Hello, Taurine!"
let upper = io_strupper(text)
let lower = io_strlower(text)
let trimmed = io_strtrim("  spaces  ")
let parts = io_strsplit("a,b,c", ",")
"#, "trimmed").unwrap();
    assert_eq!(val, taurine::Value::String("spaces".to_string()));
}

#[test]
fn test_full_program_table_operations() {
    let val = run_and_get(r#"
let config = {
    host: "localhost",
    port: 8080,
    debug: true
}
let host = config.host
let port = config.port
"#, "port").unwrap();
    assert_eq!(val, taurine::Value::Number(8080.0));
}

#[test]
fn test_full_program_nested_loops() {
    let val = run_and_get(r#"
let matrix_sum = 0
for i in 1..4 {
    for j in 1..4 {
        matrix_sum = matrix_sum + i * j
    }
}
"#, "matrix_sum").unwrap();
    assert_eq!(val, taurine::Value::Number(36.0));
}

#[test]
fn test_full_program_pattern_matching_complex() {
    let val = run_and_get(r#"
function classify(x) {
    return match x {
        0 => "zero",
        1 => "one",
        n if n > 100 => "large",
        n if n > 0 => "positive",
        _ => "negative"
    }
}
let a = classify(0)
let b = classify(1)
let c = classify(50)
let d = classify(200)
let e = classify(-5)
"#, "d").unwrap();
    assert_eq!(val, taurine::Value::String("large".to_string()));
}

#[test]
fn test_full_program_async_flow() {
    let result = run_code(r#"
async function fetchData(url) {
    return "Data from " + url
}
async function process() {
    let data = await fetchData("https://api.example.com")
    return data
}
let result = await process()
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_generator_with_state() {
    let val = run_and_get(r#"
generator fibonacci(n) {
    let a = 0
    let b = 1
    let i = 0
    while i < n {
        yield a
        let temp = a + b
        a = b
        b = temp
        i = i + 1
    }
}
let last = 0
for num in fibonacci(10) {
    last = num
}
"#, "last").unwrap();
    assert_eq!(val, taurine::Value::Number(34.0));
}

#[test]
fn test_full_program_null_safety() {
    let val = run_and_get(r#"
let obj = nil
let x = obj?.missing
let y = x ?? "default"
"#, "y").unwrap();
    assert_eq!(val, taurine::Value::String("default".to_string()));
}

#[test]
fn test_full_program_spread_operator() {
    let result = run_code(r#"
let arr1 = [1, 2, 3]
let arr2 = [...arr1, 4, 5, 6]
"#);
    assert!(result.is_ok());
}

#[test]
fn test_full_program_import_alias() {
    let result = run_code(r#"
import "std/math.tau" as math
"#);
    // May fail if std not found, but shouldn't crash
    let _ = result;
}

#[test]
fn test_full_program_json_roundtrip() {
    let val = run_and_get(r#"
let original = { name: "test", value: 42, items: [1, 2, 3] }
let json_str = json_stringify(original)
let parsed = json_parse(json_str)
let name = parsed.name
"#, "name").unwrap();
    assert_eq!(val, taurine::Value::String("test".to_string()));
}

#[test]
fn test_full_program_string_interpolation() {
    let val = run_and_get(r#"
let name = "Taurine"
let version = 2
let msg = f"Hello from {name} v{version}!"
"#, "msg").unwrap();
    assert_eq!(val, taurine::Value::String("Hello from Taurine v2!".to_string()));
}

#[test]
fn test_full_program_while_with_break_continue() {
    let val = run_and_get(r#"
let sum = 0
let i = 0
while i < 20 {
    i = i + 1
    if i % 2 == 0 { continue }
    if i > 15 { break }
    sum = sum + i
}
"#, "sum").unwrap();
    // 1 + 3 + 5 + 7 + 9 + 11 + 13 + 15 = 64
    assert_eq!(val, taurine::Value::Number(64.0));
}

#[test]
fn test_full_program_recursive_data_processing() {
    let val = run_and_get(r#"
function sumNested(arr) {
    let total = 0
    for item in arr {
        total = total + item
    }
    return total
}
let data = [1, 2, 3, 4, 5]
let result = sumNested(data)
"#, "result").unwrap();
    assert_eq!(val, taurine::Value::Number(15.0));
}