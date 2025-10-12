use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::hir::*;
use crate::interner::{Symbol, SymbolInterner};
use crate::source::SourceMap;
use smol_str::SmolStr;

pub struct LoweringContext {
    interner: SymbolInterner,
    ids: HirIdAllocator,
    diagnostics: Vec<Diagnostic>,
    _source_map: SourceMap,
}

impl LoweringContext {
    pub fn new(source_map: SourceMap) -> Self {
        Self {
            interner: SymbolInterner::new(),
            ids: HirIdAllocator::new(),
            diagnostics: Vec::new(),
            _source_map: source_map,
        }
    }

    pub fn lower_module(&mut self, module: Module) -> HirModule {
        let module_id = self.ids.alloc();
        let mut items = Vec::new();
        for item in module.items {
            if let Some(hir_item) = self.lower_item(item) {
                items.push(hir_item);
            }
        }
        let interner = std::mem::take(&mut self.interner);
        HirModule {
            id: module_id,
            items,
            interner,
        }
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn lower_item(&mut self, item: Item) -> Option<HirItem> {
        match item {
            Item::Let(let_stmt) => Some(HirItem::Let(self.lower_let(let_stmt))),
            Item::Function(func) => Some(HirItem::Function(self.lower_function(func))),
        }
    }

    fn lower_function(&mut self, func: Function) -> HirFunction {
        let id = self.ids.alloc();
        let name = self.intern(func.name);
        let params = func
            .params
            .into_iter()
            .map(|param| HirParam {
                id: self.ids.alloc(),
                name: self.intern(param.name),
                span: param.span,
            })
            .collect();
        let body = self.lower_block(func.body);
        HirFunction {
            id,
            name,
            params,
            body,
            span: func.span,
        }
    }

    fn lower_let(&mut self, let_stmt: LetStatement) -> HirLetBinding {
        let id = self.ids.alloc();
        let name = self.intern(let_stmt.name);
        let value = self.lower_expr(let_stmt.value);
        HirLetBinding {
            id,
            name,
            value,
            span: let_stmt.span,
        }
    }

    fn lower_block(&mut self, block: Block) -> HirBlock {
        let id = self.ids.alloc();
        let statements = block
            .statements
            .into_iter()
            .map(|stmt| self.lower_stmt(stmt))
            .collect();
        let tail = block.tail.map(|expr| Box::new(self.lower_expr(*expr)));
        HirBlock {
            id,
            statements,
            tail,
            span: block.span,
        }
    }

    fn lower_stmt(&mut self, stmt: Stmt) -> HirStmt {
        match stmt {
            Stmt::Let(let_stmt) => HirStmt::Let(self.lower_let(let_stmt)),
            Stmt::Return(ret_stmt) => HirStmt::Return(self.lower_return(ret_stmt)),
            Stmt::While(while_stmt) => HirStmt::While(self.lower_while(while_stmt)),
            Stmt::Expr(expr) => HirStmt::Expr(self.lower_expr(expr)),
        }
    }

    fn lower_return(&mut self, ret: ReturnStatement) -> HirReturn {
        let id = self.ids.alloc();
        let value = ret.value.map(|expr| Box::new(self.lower_expr(expr)));
        HirReturn {
            id,
            value,
            span: ret.span,
        }
    }

    fn lower_while(&mut self, while_stmt: WhileStatement) -> HirWhile {
        let id = self.ids.alloc();
        let condition = Box::new(self.lower_expr(while_stmt.condition));
        let body = Box::new(self.lower_block(*while_stmt.body));
        HirWhile {
            id,
            condition,
            body,
            span: while_stmt.span,
        }
    }

    fn lower_expr(&mut self, expr: Expr) -> HirExpr {
        match expr {
            Expr::Literal(lit) => HirExpr::Literal(self.lower_literal(lit)),
            Expr::Name(name) => HirExpr::Name(self.lower_name(name)),
            Expr::Call(call) => HirExpr::Call(self.lower_call(call)),
            Expr::If(if_expr) => HirExpr::If(Box::new(self.lower_if(*if_expr))),
            Expr::Block(block) => HirExpr::Block(Box::new(self.lower_block(*block))),
            Expr::Paren(inner) => self.lower_expr(*inner),
            Expr::Binary(bin) => HirExpr::Binary(self.lower_binary(bin)),
            Expr::Unary(unary) => HirExpr::Unary(self.lower_unary(unary)),
        }
    }

    fn lower_literal(&mut self, literal: Literal) -> HirLiteral {
        match literal {
            Literal::Int(int) => HirLiteral::Int(HirIntLiteral {
                id: self.ids.alloc(),
                value: int.value.to_string(),
                span: int.span,
            }),
            Literal::String(string) => HirLiteral::String(HirStringLiteral {
                id: self.ids.alloc(),
                value: string.value.to_string(),
                span: string.span,
            }),
            Literal::Bool(bool_lit) => HirLiteral::Bool(HirBoolLiteral {
                id: self.ids.alloc(),
                value: bool_lit.value,
                span: bool_lit.span,
            }),
            Literal::Unit(unit) => HirLiteral::Unit(HirUnitLiteral {
                id: self.ids.alloc(),
                span: unit.span,
            }),
        }
    }

    fn lower_name(&mut self, name: NameRef) -> HirNameRef {
        HirNameRef {
            id: self.ids.alloc(),
            name: self.intern(name.name),
            span: name.span,
        }
    }

    fn lower_call(&mut self, call: CallExpr) -> HirCall {
        HirCall {
            id: self.ids.alloc(),
            callee: Box::new(self.lower_expr(*call.callee)),
            args: call
                .args
                .into_iter()
                .map(|arg| self.lower_expr(arg))
                .collect(),
            span: call.span,
        }
    }

    fn lower_if(&mut self, if_expr: IfExpr) -> HirIf {
        HirIf {
            id: self.ids.alloc(),
            condition: Box::new(self.lower_expr(*if_expr.condition)),
            then_branch: Box::new(self.lower_block(*if_expr.then_branch)),
            else_branch: if_expr
                .else_branch
                .map(|block| Box::new(self.lower_block(*block))),
            span: if_expr.span,
        }
    }

    fn lower_binary(&mut self, bin: BinaryExpr) -> HirBinary {
        HirBinary {
            id: self.ids.alloc(),
            op: match bin.op {
                BinaryOp::Add => HirBinaryOp::Add,
                BinaryOp::Sub => HirBinaryOp::Sub,
                BinaryOp::Mul => HirBinaryOp::Mul,
                BinaryOp::Div => HirBinaryOp::Div,
                BinaryOp::Eq => HirBinaryOp::Eq,
                BinaryOp::Ne => HirBinaryOp::Ne,
                BinaryOp::Lt => HirBinaryOp::Lt,
                BinaryOp::Le => HirBinaryOp::Le,
                BinaryOp::Gt => HirBinaryOp::Gt,
                BinaryOp::Ge => HirBinaryOp::Ge,
            },
            lhs: Box::new(self.lower_expr(*bin.lhs)),
            rhs: Box::new(self.lower_expr(*bin.rhs)),
            span: bin.span,
        }
    }

    fn lower_unary(&mut self, unary: UnaryExpr) -> HirUnary {
        HirUnary {
            id: self.ids.alloc(),
            op: match unary.op {
                UnaryOp::Neg => HirUnaryOp::Neg,
                UnaryOp::Not => HirUnaryOp::Not,
            },
            expr: Box::new(self.lower_expr(*unary.expr)),
            span: unary.span,
        }
    }

    fn intern(&mut self, name: SmolStr) -> Symbol {
        self.interner.intern(name)
    }
}
