use crate::interner::{Symbol, SymbolInterner};
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct HirModule {
    pub id: HirId,
    pub items: Vec<HirItem>,
    pub interner: SymbolInterner,
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Let(HirLetBinding),
    Function(HirFunction),
}

#[derive(Debug, Clone)]
pub struct HirLetBinding {
    pub id: HirId,
    pub name: Symbol,
    pub value: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub id: HirId,
    pub name: Symbol,
    pub params: Vec<HirParam>,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub id: HirId,
    pub name: Symbol,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub id: HirId,
    pub statements: Vec<HirStmt>,
    pub tail: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let(HirLetBinding),
    While(HirWhile),
    Return(HirReturn),
    Expr(HirExpr),
}

#[derive(Debug, Clone)]
pub struct HirReturn {
    pub id: HirId,
    pub value: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirWhile {
    pub id: HirId,
    pub condition: Box<HirExpr>,
    pub body: Box<HirBlock>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExpr {
    Literal(HirLiteral),
    Name(HirNameRef),
    Call(HirCall),
    If(Box<HirIf>),
    Block(Box<HirBlock>),
    Binary(HirBinary),
    Unary(HirUnary),
}

#[derive(Debug, Clone)]
pub struct HirNameRef {
    pub id: HirId,
    pub name: Symbol,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirLiteral {
    Int(HirIntLiteral),
    String(HirStringLiteral),
    Bool(HirBoolLiteral),
    Unit(HirUnitLiteral),
}

#[derive(Debug, Clone)]
pub struct HirIntLiteral {
    pub id: HirId,
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStringLiteral {
    pub id: HirId,
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirBoolLiteral {
    pub id: HirId,
    pub value: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirUnitLiteral {
    pub id: HirId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirCall {
    pub id: HirId,
    pub callee: Box<HirExpr>,
    pub args: Vec<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirIf {
    pub id: HirId,
    pub condition: Box<HirExpr>,
    pub then_branch: Box<HirBlock>,
    pub else_branch: Option<Box<HirBlock>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirBinary {
    pub id: HirId,
    pub op: HirBinaryOp,
    pub lhs: Box<HirExpr>,
    pub rhs: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirUnary {
    pub id: HirId,
    pub op: HirUnaryOp,
    pub expr: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
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
pub enum HirUnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(u32);

impl HirId {
    pub fn new(raw: u32) -> Self {
        HirId(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Default)]
pub struct HirIdAllocator {
    next: u32,
}

impl HirIdAllocator {
    pub fn new() -> Self {
        Self { next: 1 }
    }

    pub fn alloc(&mut self) -> HirId {
        let id = HirId(self.next);
        self.next += 1;
        id
    }
}
