use std::collections::HashMap;

use kayton_front::diagnostics::Diagnostic;
use kayton_front::hir::*;
use kayton_front::interner::Symbol;
use kayton_front::span::Span;

pub mod fast {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum FastType {
        Int,
        Bool,
        String,
        Unit,
        Function {
            arity: usize,
            return_ty: Box<FastType>,
        },
        Unknown,
    }

    impl FastType {
        fn unify(&self, other: &FastType) -> FastType {
            match (self, other) {
                (FastType::Unknown, _) => other.clone(),
                (_, FastType::Unknown) => self.clone(),
                _ if self == other => self.clone(),
                _ => FastType::Unknown,
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct FastAnalysis {
        pub types: HashMap<HirId, FastType>,
        pub diagnostics: Vec<Diagnostic>,
    }

    impl FastAnalysis {
        pub fn type_of(&self, id: HirId) -> Option<&FastType> {
            self.types.get(&id)
        }
    }

    pub fn analyze(module: &HirModule) -> FastAnalysis {
        let mut ctx = Context::new();
        ctx.analyze_module(module);
        FastAnalysis {
            types: ctx.types,
            diagnostics: ctx.diagnostics,
        }
    }

    #[derive(Clone)]
    struct Binding {
        ty: FastType,
    }

    struct FunctionContext {
        return_ty: FastType,
        has_explicit_return: bool,
    }

    struct Context {
        types: HashMap<HirId, FastType>,
        diagnostics: Vec<Diagnostic>,
        scopes: Vec<HashMap<Symbol, Binding>>,
        current_function: Option<FunctionContext>,
    }

    impl Context {
        fn new() -> Self {
            Self {
                types: HashMap::new(),
                diagnostics: Vec::new(),
                scopes: vec![HashMap::new()],
                current_function: None,
            }
        }

        fn analyze_module(&mut self, module: &HirModule) {
            for item in &module.items {
                if let HirItem::Function(func) = item {
                    let func_ty = FastType::Function {
                        arity: func.params.len(),
                        return_ty: Box::new(FastType::Unknown),
                    };
                    self.types.insert(func.id, func_ty.clone());
                    self.bind(func.name, func_ty);
                }
            }

            for item in &module.items {
                match item {
                    HirItem::Let(binding) => {
                        let ty = self.analyze_expr(&binding.value);
                        self.types.insert(binding.id, ty.clone());
                        self.bind(binding.name, ty);
                    }
                    HirItem::Function(func) => self.analyze_function(func),
                }
            }
        }

        fn analyze_function(&mut self, func: &HirFunction) {
            self.scopes.push(HashMap::new());
            self.current_function = Some(FunctionContext {
                return_ty: FastType::Unknown,
                has_explicit_return: false,
            });

            for param in &func.params {
                self.types.insert(param.id, FastType::Unknown);
                self.bind(param.name, FastType::Unknown);
            }

            let body_ty = self.analyze_block(&func.body);
            let fn_ctx = self
                .current_function
                .take()
                .expect("current function context missing");
            let final_return = if fn_ctx.has_explicit_return {
                fn_ctx.return_ty
            } else {
                body_ty
            };
            if let Some(entry) = self.types.get_mut(&func.id) {
                *entry = FastType::Function {
                    arity: func.params.len(),
                    return_ty: Box::new(final_return.clone()),
                };
            }
            self.pop_scope();
        }

        fn analyze_block(&mut self, block: &HirBlock) -> FastType {
            self.push_scope();
            for stmt in &block.statements {
                self.analyze_stmt(stmt);
            }
            let tail_ty = if let Some(tail) = &block.tail {
                self.analyze_expr(tail)
            } else {
                FastType::Unit
            };
            self.types.insert(block.id, tail_ty.clone());
            self.pop_scope();
            tail_ty
        }

        fn analyze_stmt(&mut self, stmt: &HirStmt) {
            match stmt {
                HirStmt::Let(binding) => {
                    let ty = self.analyze_expr(&binding.value);
                    self.types.insert(binding.id, ty.clone());
                    self.bind(binding.name, ty);
                }
                HirStmt::While(while_stmt) => {
                    let cond_ty = self.analyze_expr(&while_stmt.condition);
                    if !matches!(cond_ty, FastType::Bool | FastType::Unknown) {
                        self.error("while condition must be bool", while_stmt.span);
                    }
                    let body_ty = self.analyze_block(&while_stmt.body);
                    if !matches!(body_ty, FastType::Unit | FastType::Unknown) {
                        self.error("while body must produce unit", while_stmt.body.span);
                    }
                    self.types.insert(while_stmt.id, FastType::Unit);
                }
                HirStmt::Return(ret) => {
                    let ty = ret
                        .value
                        .as_ref()
                        .map(|expr| self.analyze_expr(expr))
                        .unwrap_or(FastType::Unit);
                    self.types.insert(ret.id, ty.clone());
                    let mut conflict = None;
                    if let Some(fn_ctx) = self.current_function.as_mut() {
                        let unified = fn_ctx.return_ty.unify(&ty);
                        if matches!(unified, FastType::Unknown) && !matches!(ty, FastType::Unknown)
                        {
                            conflict = Some(ret.span);
                        }
                        if !matches!(ty, FastType::Unknown) {
                            fn_ctx.return_ty = ty.clone();
                        }
                        fn_ctx.has_explicit_return = true;
                    }
                    if let Some(span) = conflict {
                        self.error("conflicting return types", span);
                    }
                }
                HirStmt::Expr(expr) => {
                    let ty = self.analyze_expr(expr);
                    if !matches!(ty, FastType::Unit | FastType::Unknown) {
                        self.error("expression statement must evaluate to unit", expr.span());
                    }
                }
            }
        }

        fn analyze_expr(&mut self, expr: &HirExpr) -> FastType {
            match expr {
                HirExpr::Literal(lit) => match lit {
                    HirLiteral::Int(int) => {
                        self.types.insert(int.id, FastType::Int);
                        FastType::Int
                    }
                    HirLiteral::String(string) => {
                        self.types.insert(string.id, FastType::String);
                        FastType::String
                    }
                    HirLiteral::Bool(boolean) => {
                        self.types.insert(boolean.id, FastType::Bool);
                        FastType::Bool
                    }
                    HirLiteral::Unit(unit) => {
                        self.types.insert(unit.id, FastType::Unit);
                        FastType::Unit
                    }
                },
                HirExpr::Name(name) => {
                    if let Some(binding) = self.lookup(name.name) {
                        self.types.insert(name.id, binding.ty.clone());
                        binding.ty
                    } else {
                        FastType::Unknown
                    }
                }
                HirExpr::Call(call) => {
                    let callee_ty = self.analyze_expr(&call.callee);
                    for arg in &call.args {
                        self.analyze_expr(arg);
                    }
                    match callee_ty {
                        FastType::Function {
                            arity,
                            ref return_ty,
                        } => {
                            if arity != call.args.len() {
                                self.error(
                                    format!(
                                        "expected {arity} arguments, found {}",
                                        call.args.len()
                                    ),
                                    call.span,
                                );
                            }
                            let ret = (*return_ty.clone()).clone();
                            self.types.insert(call.id, ret.clone());
                            ret
                        }
                        FastType::Unknown => FastType::Unknown,
                        _ => {
                            self.error("cannot call non-function", call.span);
                            FastType::Unknown
                        }
                    }
                }
                HirExpr::If(if_expr) => {
                    let cond_ty = self.analyze_expr(&if_expr.condition);
                    if !matches!(cond_ty, FastType::Bool | FastType::Unknown) {
                        self.error("if condition must be bool", if_expr.condition.span());
                    }
                    let then_ty = self.analyze_block(&if_expr.then_branch);
                    let else_ty = if let Some(else_branch) = &if_expr.else_branch {
                        self.analyze_block(else_branch)
                    } else {
                        FastType::Unit
                    };
                    let unified = then_ty.unify(&else_ty);
                    if matches!(unified, FastType::Unknown)
                        && !matches!(then_ty, FastType::Unknown | FastType::Unit)
                        && !matches!(else_ty, FastType::Unknown | FastType::Unit)
                    {
                        self.error("mismatched branch types", if_expr.span);
                    }
                    self.types.insert(if_expr.id, unified.clone());
                    unified
                }
                HirExpr::Block(block) => {
                    let ty = self.analyze_block(block);
                    self.types.insert(block.id, ty.clone());
                    ty
                }
                HirExpr::Binary(bin) => {
                    let lhs = self.analyze_expr(&bin.lhs);
                    let rhs = self.analyze_expr(&bin.rhs);
                    let (required, result) = match bin.op {
                        HirBinaryOp::Add
                        | HirBinaryOp::Sub
                        | HirBinaryOp::Mul
                        | HirBinaryOp::Div => (FastType::Int, FastType::Int),
                        HirBinaryOp::Eq
                        | HirBinaryOp::Ne
                        | HirBinaryOp::Lt
                        | HirBinaryOp::Le
                        | HirBinaryOp::Gt
                        | HirBinaryOp::Ge => (FastType::Int, FastType::Bool),
                    };
                    if !matches!(lhs, FastType::Unknown) && lhs != required {
                        self.error("left operand has wrong type", bin.lhs.span());
                    }
                    if !matches!(rhs, FastType::Unknown) && rhs != required {
                        self.error("right operand has wrong type", bin.rhs.span());
                    }
                    self.types.insert(bin.id, result.clone());
                    result
                }
                HirExpr::Unary(un) => {
                    let operand_ty = self.analyze_expr(&un.expr);
                    let (required, result) = match un.op {
                        HirUnaryOp::Neg => (FastType::Int, FastType::Int),
                        HirUnaryOp::Not => (FastType::Bool, FastType::Bool),
                    };
                    if !matches!(operand_ty, FastType::Unknown) && operand_ty != required {
                        self.error("unary operand has wrong type", un.expr.span());
                    }
                    self.types.insert(un.id, result.clone());
                    result
                }
            }
        }

        fn lookup(&self, name: Symbol) -> Option<Binding> {
            for scope in self.scopes.iter().rev() {
                if let Some(binding) = scope.get(&name) {
                    return Some(binding.clone());
                }
            }
            None
        }

        fn bind(&mut self, name: Symbol, ty: FastType) {
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(name, Binding { ty });
            }
        }

        fn push_scope(&mut self) {
            self.scopes.push(HashMap::new());
        }

        fn pop_scope(&mut self) {
            self.scopes.pop();
        }

        fn error(&mut self, message: impl Into<String>, span: Span) {
            let diag = Diagnostic::error(message.into(), span);
            self.diagnostics.push(diag);
        }
    }

    trait ExprExt {
        fn span(&self) -> Span;
    }

    impl ExprExt for HirExpr {
        fn span(&self) -> Span {
            match self {
                HirExpr::Literal(lit) => match lit {
                    HirLiteral::Int(int) => int.span,
                    HirLiteral::String(string) => string.span,
                    HirLiteral::Bool(boolean) => boolean.span,
                    HirLiteral::Unit(unit) => unit.span,
                },
                HirExpr::Name(name) => name.span,
                HirExpr::Call(call) => call.span,
                HirExpr::If(if_expr) => if_expr.span,
                HirExpr::Block(block) => block.span,
                HirExpr::Binary(bin) => bin.span,
                HirExpr::Unary(un) => un.span,
            }
        }
    }
}
