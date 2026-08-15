use proptest::prelude::*;
use taurine::lexer::{tokenize, TokenKind};
use taurine::parser::Parser;
use taurine::string_intern::StringInterner;

fn arb_identifier() -> impl Strategy<Value = String> {
    "[a-zA-Z_][a-zA-Z0-9_]{0,20}"
}

fn arb_number() -> impl Strategy<Value = f64> {
    0.0..1e15
}

fn arb_simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_number().prop_map(|n| format!("{}", n)),
        arb_identifier(),
        (arb_number(), arb_number()).prop_map(|(a, b)| format!("{} + {}", a, b)),
        (arb_number(), arb_number()).prop_map(|(a, b)| format!("{} * {}", a, b)),
    ]
}

proptest! {
    #[test]
    fn test_lexer_never_panics(input in "[a-zA-Z0-9_ \\t\\n+\\-*/=<>!(){}\\[\\],;:.#\"']{0,200}") {
        let _tokens = tokenize(&input);
    }

    #[test]
    fn test_lexer_roundtrip_identifiers(name in arb_identifier()) {
        let tokens = tokenize(&name);
        prop_assert!(!tokens.is_empty());
        prop_assert_eq!(&tokens[0].kind, &TokenKind::Identifier);
        prop_assert_eq!(tokens[0].lexeme.as_str(), name.as_str());
    }

    #[test]
    fn test_lexer_numbers(n in arb_number()) {
        let s = format!("{}", n);
        let tokens = tokenize(&s);
        if !tokens.is_empty() {
            prop_assert_eq!(&tokens[0].kind, &TokenKind::Number);
        }
    }

    #[test]
    fn test_parser_never_panics(expr in arb_simple_expr()) {
        let code = format!("let x = {}", expr);
        let tokens = tokenize(&code);
        let mut parser = Parser::new(tokens);
        let _ = parser.parse();
    }

    #[test]
    fn test_parser_declaration(name in arb_identifier(), val in arb_number()) {
        let code = format!("let {} = {}", name, val);
        let tokens = tokenize(&code);
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        prop_assert!(result.is_ok());
        let program = result.unwrap();
        prop_assert_eq!(program.statements.len(), 1);
    }

    #[test]
    fn test_interner_idempotent(s in "[a-zA-Z_][a-zA-Z0-9_]{0,50}") {
        let mut interner = StringInterner::new();
        let id1 = interner.intern(&s);
        let id2 = interner.intern(&s);
        prop_assert_eq!(id1, id2);
        prop_assert_eq!(interner.get(id1), Some(s.as_str()));
    }

    #[test]
    fn test_interner_unique_strings(
        s1 in "[a-zA-Z_][a-zA-Z0-9_]{0,20}",
        s2 in "[a-zA-Z_][a-zA-Z0-9_]{0,20}"
    ) {
        prop_assume!(s1 != s2);
        let mut interner = StringInterner::new();
        let id1 = interner.intern(&s1);
        let id2 = interner.intern(&s2);
        prop_assert_ne!(id1, id2);
    }
}