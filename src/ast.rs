use crate::lexer::TokenKind;
use crate::string_intern::InternedString;

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    String(String),
    Identifier(InternedString),
    Binary {
        left: Box<Expr>,
        op: TokenKind,
        right: Box<Expr>,
        line: usize,
    },
    Unary {
        op: TokenKind,
        expr: Box<Expr>,
        line: usize,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        line: usize,
    },
    LiteralTrue,
    LiteralFalse,
    LiteralNil,
    Table {
        entries: Vec<(InternedString, Expr)>,
        line: usize,
    },
    Array {
        items: Vec<Expr>,
        line: usize,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        line: usize,
    },
    SetIndex {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
        line: usize,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        line: usize,
    },
    Length {
        expr: Box<Expr>,
        line: usize,
    },
    Get {
        object: Box<Expr>,
        name: InternedString,
        line: usize,
    },
    SafeGet {
        object: Box<Expr>,
        name: InternedString,
        line: usize,
    },
    SetProperty {
        object: Box<Expr>,
        name: InternedString,
        value: Box<Expr>,
        line: usize,
    },
    Throw {
        expr: Box<Expr>,
        line: usize,
    },
    FunctionLiteral {
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    AsyncFunctionLiteral {
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    GeneratorLiteral {
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    Lambda {
        params: Vec<(InternedString, Option<Expr>)>,
        body: Box<Expr>,
        line: usize,
    },
    Spread {
        expr: Box<Expr>,
        line: usize,
    },
    Set {
        items: Vec<Expr>,
        line: usize,
    },
    NullCoalesce {
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
        line: usize,
    },
    Require {
        path: String,
        line: usize,
    },
    Export {
        name: InternedString,
        value: Box<Expr>,
        line: usize,
    },
    Class {
        name: InternedString,
        superclass: Option<InternedString>,
        methods: Vec<(InternedString, Expr)>,
        line: usize,
    },
    NewInstance {
        class_name: InternedString,
        arguments: Vec<Expr>,
        line: usize,
    },
    FString {
        parts: Vec<(String, Option<Box<Expr>>)>,
        line: usize,
    },
    This {
        line: usize,
    },
    Super {
        method: InternedString,
        line: usize,
    },
    Await {
        future: Box<Expr>,
        line: usize,
    },
    Yield {
        value: Option<Box<Expr>>,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Expr),
    Identifier(InternedString),
    Wildcard,
    Array(Vec<Pattern>),
    Table(Vec<(InternedString, Pattern)>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Declaration {
        name: InternedString,
        initializer: Option<Expr>,
        line: usize,
        is_const: bool,
    },
    Destructure {
        names: Vec<InternedString>,
        initializer: Expr,
        line: usize,
    },
    Assignment {
        name: InternedString,
        value: Expr,
        line: usize,
        is_const_assign: bool,
    },
    NullCoalesceAssign {
        name: InternedString,
        value: Expr,
        line: usize,
    },
    Expression(Expr),
    Return(Option<Expr>),
    ReturnMulti(Vec<Expr>),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        line: usize,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        line: usize,
    },
    For {
        initializer: Option<Box<Expr>>,
        condition: Box<Expr>,
        increment: Option<Box<Expr>>,
        body: Vec<Stmt>,
        line: usize,
    },
    ForIn {
        variable: InternedString,
        iterable: Expr,
        body: Vec<Stmt>,
        line: usize,
    },
    Function {
        name: InternedString,
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    Block(Vec<Stmt>),
    Import {
        path: String,
        alias: Option<InternedString>,
        line: usize,
    },
    Try {
        body: Vec<Stmt>,
        catch_var: Option<InternedString>,
        catch_body: Vec<Stmt>,
        line: usize,
    },
    Throw {
        value: Expr,
        line: usize,
    },
    Break,
    Continue,
    Class {
        name: InternedString,
        superclass: Option<InternedString>,
        methods: Vec<(InternedString, Expr)>,
        line: usize,
    },
    Export {
        name: InternedString,
        value: Expr,
        line: usize,
    },
    // Async function declaration
    AsyncFunction {
        name: InternedString,
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    // Generator function declaration
    Generator {
        name: InternedString,
        params: Vec<(InternedString, Option<Expr>)>,
        body: Vec<Stmt>,
        line: usize,
    },
    Set {
        object: Box<Expr>,
        name: InternedString,
        value: Expr,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Stmt>,
}
