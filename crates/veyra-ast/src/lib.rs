use veyra_source::Span;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Unit,
    Array(Box<Type>),
    Named(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Range,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Decl {
    Function(Function),
    Struct(StructDecl),
    Import(ImportDecl),
    Global(LetStmt),
    Policy(PolicyDecl),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyDecl {
    pub name: String,
    pub owner: String,
    pub export_denied: bool,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub public: bool,
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Type,
    pub body: Block,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    pub public: bool,
    pub name: String,
    pub fields: Vec<Parameter>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    pub name: String,
    pub span: Span,
}
pub type Block = Vec<Stmt>;
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Let(LetStmt),
    Expr(Expr),
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Block,
        else_branch: Block,
        span: Span,
    },
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    For {
        name: String,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct LetStmt {
    pub name: String,
    pub mutable: bool,
    pub annotation: Option<Type>,
    pub initializer: Expr,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Unit,
    Variable(String),
    Array(Vec<Expr>),
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Get {
        object: Box<Expr>,
        name: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Private {
        policy: String,
        value: Box<Expr>,
    },
}
