use taurine::lexer::tokenize;
use taurine::parser::Parser;
use taurine::ast::{Stmt, Expr};

fn parse(code: &str) -> Result<Vec<Stmt>, String> {
    let tokens = tokenize(code);
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    Ok(program.statements)
}

#[test]
fn test_variable_declaration() {
    let stmts = parse("let x = 42").unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        Stmt::Declaration { name, initializer, is_const, .. } => {
            assert!(!is_const);
            assert!(initializer.is_some());
        }
        _ => panic!("Expected Declaration"),
    }
}

#[test]
fn test_const_declaration() {
    let stmts = parse("const PI = 3.14").unwrap();
    match &stmts[0] {
        Stmt::Declaration { is_const, .. } => assert!(is_const),
        _ => panic!("Expected const Declaration"),
    }
}

#[test]
fn test_function_declaration() {
    let stmts = parse("function add(a, b) { return a + b }").unwrap();
    match &stmts[0] {
        Stmt::Function { name, params, body, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected Function"),
    }
}

#[test]
fn test_if_statement() {
    let stmts = parse("if x > 5 { print(x) }").unwrap();
    match &stmts[0] {
        Stmt::If { condition, then_branch, else_branch, .. } => {
            assert_eq!(then_branch.len(), 1);
            assert!(else_branch.is_none());
        }
        _ => panic!("Expected If"),
    }
}

#[test]
fn test_if_else_statement() {
    let stmts = parse("if x > 5 { print(x) } else { print(0) }").unwrap();
    match &stmts[0] {
        Stmt::If { else_branch, .. } => {
            assert!(else_branch.is_some());
        }
        _ => panic!("Expected If with else"),
    }
}

#[test]
fn test_while_loop() {
    let stmts = parse("while x < 10 { x = x + 1 }").unwrap();
    match &stmts[0] {
        Stmt::While { condition, body, .. } => {
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected While"),
    }
}

#[test]
fn test_for_in_loop() {
    let stmts = parse("for i in 1..10 { print(i) }").unwrap();
    match &stmts[0] {
        Stmt::ForIn { variable, iterable, body, .. } => {
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected ForIn"),
    }
}

#[test]
fn test_array_literal() {
    let stmts = parse("let arr = [1, 2, 3]").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Array { items, .. }), .. } => {
            assert_eq!(items.len(), 3);
        }
        _ => panic!("Expected Array"),
    }
}

#[test]
fn test_table_literal() {
    let stmts = parse("let obj = { name: \"test\", value: 42 }").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Table { entries, .. }), .. } => {
            assert_eq!(entries.len(), 2);
        }
        _ => panic!("Expected Table"),
    }
}

#[test]
fn test_lambda() {
    let stmts = parse("let add = (x, y) => x + y").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Lambda { params, .. }), .. } => {
            assert_eq!(params.len(), 2);
        }
        _ => panic!("Expected Lambda"),
    }
}

#[test]
fn test_class_declaration() {
    let stmts = parse("class Dog { function bark() { print(\"Woof\") } }").unwrap();
    match &stmts[0] {
        Stmt::Class { name, methods, .. } => {
            assert_eq!(methods.len(), 1);
        }
        _ => panic!("Expected Class"),
    }
}

#[test]
fn test_try_catch() {
    let stmts = parse("try { risky() } catch (e) { print(e) }").unwrap();
    match &stmts[0] {
        Stmt::Try { body, catch_var, catch_body, .. } => {
            assert_eq!(body.len(), 1);
            assert!(catch_var.is_some());
            assert_eq!(catch_body.len(), 1);
        }
        _ => panic!("Expected Try"),
    }
}

#[test]
fn test_import() {
    let stmts = parse("import \"module.tau\"").unwrap();
    match &stmts[0] {
        Stmt::Import { path, alias, .. } => {
            assert_eq!(path, "module.tau");
            assert!(alias.is_none());
        }
        _ => panic!("Expected Import"),
    }
}

#[test]
fn test_import_with_alias() {
    let stmts = parse("import \"module.tau\" as mod").unwrap();
    match &stmts[0] {
        Stmt::Import { path, alias, .. } => {
            assert_eq!(path, "module.tau");
            assert!(alias.is_some());
        }
        _ => panic!("Expected Import with alias"),
    }
}

#[test]
fn test_match_expression() {
    let stmts = parse("let result = match x { 0 => \"zero\", _ => \"other\" }").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Match { arms, .. }), .. } => {
            assert_eq!(arms.len(), 2);
        }
        _ => panic!("Expected Match"),
    }
}

#[test]
fn test_async_function() {
    let stmts = parse("async function fetchData() { return data }").unwrap();
    match &stmts[0] {
        Stmt::AsyncFunction { name, params, body, .. } => {
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected AsyncFunction"),
    }
}

#[test]
fn test_null_coalesce() {
    let stmts = parse("let x = a ?? b").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::NullCoalesce { .. }), .. } => {}
        _ => panic!("Expected NullCoalesce"),
    }
}

#[test]
fn test_safe_navigation() {
    let stmts = parse("let x = obj?.prop").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::SafeGet { .. }), .. } => {}
        _ => panic!("Expected SafeGet"),
    }
}

#[test]
fn test_spread_operator() {
    let stmts = parse("let arr = [...other, 4, 5]").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Array { items, .. }), .. } => {
            assert!(matches!(&items[0], Expr::Spread { .. }));
        }
        _ => panic!("Expected Array with Spread"),
    }
}

#[test]
fn test_destructuring() {
    let stmts = parse("let {a, b} = obj").unwrap();
    match &stmts[0] {
        Stmt::Destructure { names, .. } => {
            assert_eq!(names.len(), 2);
        }
        _ => panic!("Expected Destructure"),
    }
}

#[test]
fn test_return_multi() {
    let stmts = parse("function f() { return 1, 2, 3 }").unwrap();
    match &stmts[0] {
        Stmt::Function { body, .. } => {
            match &body[0] {
                Stmt::ReturnMulti(values) => assert_eq!(values.len(), 3),
                _ => panic!("Expected ReturnMulti"),
            }
        }
        _ => panic!("Expected Function"),
    }
}

#[test]
fn test_fstring() {
    let stmts = parse("let s = f\"Hello {name}\"").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::FString { parts, .. }), .. } => {
            assert!(!parts.is_empty());
        }
        _ => panic!("Expected FString"),
    }
}

#[test]
fn test_range() {
    let stmts = parse("let r = 1..10").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Range { .. }), .. } => {}
        _ => panic!("Expected Range"),
    }
}

#[test]
fn test_length_operator() {
    let stmts = parse("let len = #arr").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Length { .. }), .. } => {}
        _ => panic!("Expected Length"),
    }
}

#[test]
fn test_binary_expressions() {
    let stmts = parse("let x = 1 + 2 * 3").unwrap();
    // Should parse as 1 + (2 * 3) due to precedence
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Binary { op, .. }), .. } => {
            assert_eq!(*op, taurine::lexer::TokenKind::Plus);
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_unary_expressions() {
    let stmts = parse("let x = -5").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Unary { op, .. }), .. } => {
            assert_eq!(*op, taurine::lexer::TokenKind::Minus);
        }
        _ => panic!("Expected Unary"),
    }
}

#[test]
fn test_function_call() {
    let stmts = parse("print(42)").unwrap();
    match &stmts[0] {
        Stmt::Expression(Expr::Call { arguments, .. }) => {
            assert_eq!(arguments.len(), 1);
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_method_call() {
    let stmts = parse("obj:method(1, 2)").unwrap();
    match &stmts[0] {
        Stmt::Expression(Expr::Call { arguments, .. }) => {
            // Method call inserts 'self' as first argument
            assert_eq!(arguments.len(), 3);
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_index_access() {
    let stmts = parse("let x = arr[0]").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Index { .. }), .. } => {}
        _ => panic!("Expected Index"),
    }
}

#[test]
fn test_property_access() {
    let stmts = parse("let x = obj.prop").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Get { .. }), .. } => {}
        _ => panic!("Expected Get"),
    }
}

#[test]
fn test_assignment() {
    let stmts = parse("x = 42").unwrap();
    match &stmts[0] {
        Stmt::Assignment { name, value, .. } => {}
        _ => panic!("Expected Assignment"),
    }
}

#[test]
fn test_break_continue() {
    let stmts = parse("while true { break }").unwrap();
    match &stmts[0] {
        Stmt::While { body, .. } => {
            assert!(matches!(&body[0], Stmt::Break));
        }
        _ => panic!("Expected While with Break"),
    }
}

#[test]
fn test_export() {
    let stmts = parse("export value = 42").unwrap();
    match &stmts[0] {
        Stmt::Export { name, value, .. } => {}
        _ => panic!("Expected Export"),
    }
}

#[test]
fn test_require() {
    let stmts = parse("let mod = require(\"module.tau\")").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Require { path, .. }), .. } => {
            assert_eq!(path, "module.tau");
        }
        _ => panic!("Expected Require"),
    }
}

#[test]
fn test_new_instance() {
    let stmts = parse("let obj = new MyClass()").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::NewInstance { .. }), .. } => {}
        _ => panic!("Expected NewInstance"),
    }
}

#[test]
fn test_this() {
    let stmts = parse("function f() { return this }").unwrap();
    match &stmts[0] {
        Stmt::Function { body, .. } => {
            match &body[0] {
                Stmt::Return(Some(Expr::This { .. })) => {}
                _ => panic!("Expected This"),
            }
        }
        _ => panic!("Expected Function"),
    }
}

#[test]
fn test_super() {
    let stmts = parse("function f() { return super.method() }").unwrap();
    // Super is handled in method calls
}

#[test]
fn test_set_literal() {
    let stmts = parse("let s = set([1, 2, 3])").unwrap();
    // set() is a function call
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Call { .. }), .. } => {}
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_await() {
    let stmts = parse("let x = await promise").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::Await { .. }), .. } => {}
        _ => panic!("Expected Await"),
    }
}

#[test]
fn test_empty_program() {
    let stmts = parse("").unwrap();
    assert_eq!(stmts.len(), 0);
}

#[test]
fn test_multiple_statements() {
    let stmts = parse("let x = 1\nlet y = 2\nlet z = 3").unwrap();
    assert_eq!(stmts.len(), 3);
}

#[test]
fn test_nested_functions() {
    let stmts = parse("function outer() { function inner() { return 1 } return inner() }").unwrap();
    match &stmts[0] {
        Stmt::Function { body, .. } => {
            assert_eq!(body.len(), 2);
        }
        _ => panic!("Expected Function"),
    }
}

#[test]
fn test_class_with_superclass() {
    let stmts = parse("class Dog extends Animal { }").unwrap();
    match &stmts[0] {
        Stmt::Class { superclass, .. } => {
            assert!(superclass.is_some());
        }
        _ => panic!("Expected Class with superclass"),
    }
}

#[test]
fn test_default_parameters() {
    let stmts = parse("function f(x = 10) { return x }").unwrap();
    match &stmts[0] {
        Stmt::Function { params, .. } => {
            assert!(params[0].1.is_some()); // default value exists
        }
        _ => panic!("Expected Function with default param"),
    }
}

#[test]
fn test_complex_expression() {
    let stmts = parse("let result = (a + b) * c ?? default").unwrap();
    match &stmts[0] {
        Stmt::Declaration { initializer: Some(Expr::NullCoalesce { left, .. }), .. } => {
            assert!(matches!(&**left, Expr::Binary { .. }));
        }
        _ => panic!("Expected NullCoalesce"),
    }
}