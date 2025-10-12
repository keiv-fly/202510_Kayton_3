use crate::span::Span;
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Let(LetStatement),
    Function(Function),
}

#[derive(Debug, Clone)]
pub struct LetStatement {
    pub span: Span,
    pub name: SmolStr,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub span: Span,
    pub name: SmolStr,
    pub params: Vec<Parameter>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub span: Span,
    pub name: SmolStr,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    pub statements: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStatement),
    Return(ReturnStatement),
    While(WhileStatement),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub span: Span,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct WhileStatement {
    pub span: Span,
    pub condition: Expr,
    pub body: Box<Block>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Name(NameRef),
    Call(CallExpr),
    If(Box<IfExpr>),
    Block(Box<Block>),
    Paren(Box<Expr>),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
}

#[derive(Debug, Clone)]
pub struct NameRef {
    pub span: Span,
    pub name: SmolStr,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(IntLiteral),
    String(StringLiteral),
    Bool(BoolLiteral),
    Unit(UnitLiteral),
}

#[derive(Debug, Clone)]
pub struct IntLiteral {
    pub span: Span,
    pub value: SmolStr,
}

#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub span: Span,
    pub value: SmolStr,
}

#[derive(Debug, Clone)]
pub struct BoolLiteral {
    pub span: Span,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub struct UnitLiteral {
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub span: Span,
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub span: Span,
    pub condition: Box<Expr>,
    pub then_branch: Box<Block>,
    pub else_branch: Option<Box<Block>>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub span: Span,
    pub op: BinaryOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub span: Span,
    pub op: UnaryOp,
    pub expr: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl Module {
    pub fn new(span: Span) -> Self {
        Self {
            items: Vec::new(),
            span,
        }
    }
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(lit) => match lit {
                Literal::Int(l) => l.span,
                Literal::String(l) => l.span,
                Literal::Bool(l) => l.span,
                Literal::Unit(l) => l.span,
            },
            Expr::Name(name) => name.span,
            Expr::Call(call) => call.span,
            Expr::If(if_expr) => if_expr.span,
            Expr::Block(block) => block.span,
            Expr::Paren(expr) => expr.span(),
            Expr::Binary(bin) => bin.span,
            Expr::Unary(un) => un.span,
        }
    }
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(let_stmt) => let_stmt.span,
            Stmt::Return(ret) => ret.span,
            Stmt::While(while_stmt) => while_stmt.span,
            Stmt::Expr(expr) => expr.span(),
        }
    }
}
