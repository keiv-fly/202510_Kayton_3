use std::collections::HashMap;

use kayton_bytecode::{BytecodeModule, Constant, Function, FunctionId, Instruction};
use kayton_front::hir::*;
use kayton_front::interner::Symbol;
use kayton_front::span::Span;
use kayton_sema::fast::FastAnalysis;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmitterError {
    #[error("unknown name in expression")]
    UnknownName { span: Span },
    #[error("global binding must be a literal for now")]
    UnsupportedGlobal { span: Span },
    #[error("unsupported call target")]
    UnsupportedCallee { span: Span },
    #[error("invalid integer literal")]
    InvalidInteger { span: Span },
}

pub fn emit(module: &HirModule, analysis: &FastAnalysis) -> Result<BytecodeModule, EmitterError> {
    let mut emitter = Emitter::new(module, analysis);
    emitter.collect_functions();
    emitter.emit_items()?;
    Ok(emitter.finish())
}

struct Emitter<'a> {
    module: &'a HirModule,
    _analysis: &'a FastAnalysis,
    bytecode: BytecodeModule,
    function_indices: HashMap<Symbol, FunctionId>,
    unit_const: u32,
}

impl<'a> Emitter<'a> {
    fn new(module: &'a HirModule, analysis: &'a FastAnalysis) -> Self {
        let mut bytecode = BytecodeModule::new();
        let unit_const = bytecode.add_constant(Constant::Unit);
        Self {
            module,
            _analysis: analysis,
            bytecode,
            function_indices: HashMap::new(),
            unit_const,
        }
    }

    fn collect_functions(&mut self) {
        let mut next = 0u32;
        for item in &self.module.items {
            if let HirItem::Function(func) = item {
                self.function_indices.insert(func.name, next);
                next += 1;
            }
        }
    }

    fn emit_items(&mut self) -> Result<(), EmitterError> {
        for item in &self.module.items {
            match item {
                HirItem::Let(binding) => self.emit_global(binding)?,
                HirItem::Function(func) => {
                    let function = self.emit_function(func)?;
                    self.bytecode.add_function(function);
                }
            }
        }
        Ok(())
    }

    fn emit_global(&mut self, binding: &HirLetBinding) -> Result<(), EmitterError> {
        if let Some(constant) = self.fold_constant(&binding.value) {
            let const_id = self.add_constant(constant);
            if let Some(name) = self.module.interner.resolve(binding.name) {
                self.bytecode.add_global(name.to_string(), const_id);
            }
            Ok(())
        } else {
            Err(EmitterError::UnsupportedGlobal { span: binding.span })
        }
    }

    fn emit_function(&mut self, func: &HirFunction) -> Result<Function, EmitterError> {
        let mut builder = FunctionBuilder::new(self, func);
        builder.emit_block(&func.body, true)?;
        if !builder.returned {
            builder.instructions.push(Instruction::Return);
        }
        Ok(builder.finish())
    }

    fn add_constant(&mut self, constant: Constant) -> u32 {
        if constant == Constant::Unit {
            return self.unit_const;
        }
        if let Some((idx, _)) = self
            .bytecode
            .constants
            .iter()
            .enumerate()
            .find(|(_, c)| *c == &constant)
        {
            idx as u32
        } else {
            self.bytecode.add_constant(constant)
        }
    }

    fn function_index(&self, symbol: Symbol) -> Option<FunctionId> {
        self.function_indices.get(&symbol).copied()
    }

    fn fold_constant(&self, expr: &HirExpr) -> Option<Constant> {
        match expr {
            HirExpr::Literal(lit) => match lit {
                HirLiteral::Int(int) => int.value.parse::<i64>().ok().map(Constant::Int),
                HirLiteral::String(string) => Some(Constant::String(string.value.clone())),
                HirLiteral::Bool(boolean) => Some(Constant::Bool(boolean.value)),
                HirLiteral::Unit(_) => Some(Constant::Unit),
            },
            _ => None,
        }
    }

    fn finish(self) -> BytecodeModule {
        self.bytecode
    }
}

struct FunctionBuilder<'a, 'b> {
    emitter: &'a mut Emitter<'b>,
    function: &'a HirFunction,
    instructions: Vec<Instruction>,
    scopes: Vec<HashMap<Symbol, u16>>,
    next_local: u16,
    max_local: u16,
    returned: bool,
}

impl<'a, 'b> FunctionBuilder<'a, 'b> {
    fn new(emitter: &'a mut Emitter<'b>, function: &'a HirFunction) -> Self {
        let mut builder = Self {
            emitter,
            function,
            instructions: Vec::new(),
            scopes: Vec::new(),
            next_local: 0,
            max_local: 0,
            returned: false,
        };
        builder.push_scope();
        for param in &function.params {
            let slot = builder.alloc_local(param.name);
            builder.ensure_slot(slot);
        }
        builder
    }

    fn finish(self) -> Function {
        Function::new(
            self.function_name(),
            self.function.params.len() as u16,
            self.max_local,
            self.instructions,
        )
    }

    fn function_name(&self) -> String {
        self.emitter
            .module
            .interner
            .resolve(self.function.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("fn_{}", self.function.id.raw()))
    }

    fn ensure_slot(&mut self, slot: u16) {
        if slot >= self.max_local {
            self.max_local = slot + 1;
        }
        if self.next_local <= slot {
            self.next_local = slot + 1;
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn alloc_local(&mut self, name: Symbol) -> u16 {
        let slot = self.next_local;
        self.next_local += 1;
        self.ensure_slot(slot);
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, slot);
        }
        slot
    }

    fn lookup_local(&self, name: Symbol) -> Option<u16> {
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.get(&name) {
                return Some(*slot);
            }
        }
        None
    }

    fn emit_block(&mut self, block: &HirBlock, produce_value: bool) -> Result<(), EmitterError> {
        self.push_scope();
        for stmt in &block.statements {
            self.emit_stmt(stmt)?;
        }
        if let Some(tail) = &block.tail {
            self.emit_expr(tail)?;
            if !produce_value {
                self.instructions.push(Instruction::Pop);
            }
        } else if produce_value {
            self.push_unit();
        }
        self.pop_scope();
        Ok(())
    }

    fn emit_stmt(&mut self, stmt: &HirStmt) -> Result<(), EmitterError> {
        match stmt {
            HirStmt::Let(binding) => {
                self.emit_expr(&binding.value)?;
                let slot = self.alloc_local(binding.name);
                self.instructions.push(Instruction::StoreLocal(slot));
            }
            HirStmt::While(while_stmt) => {
                let loop_start = self.instructions.len();
                self.emit_expr(&while_stmt.condition)?;
                let jump_out_pos = self.emit_jump_placeholder(true);
                self.emit_block(&while_stmt.body, false)?;
                self.instructions.push(Instruction::Jump(loop_start));
                self.patch_jump(jump_out_pos, self.instructions.len());
            }
            HirStmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.emit_expr(value)?;
                } else {
                    self.push_unit();
                }
                self.instructions.push(Instruction::Return);
                self.returned = true;
            }
            HirStmt::Expr(expr) => {
                self.emit_expr(expr)?;
                self.instructions.push(Instruction::Pop);
            }
        }
        Ok(())
    }

    fn emit_expr(&mut self, expr: &HirExpr) -> Result<(), EmitterError> {
        match expr {
            HirExpr::Literal(lit) => {
                let const_id = match lit {
                    HirLiteral::Int(int) => {
                        let value = int
                            .value
                            .parse::<i64>()
                            .map_err(|_| EmitterError::InvalidInteger { span: int.span })?;
                        self.emitter.add_constant(Constant::Int(value))
                    }
                    HirLiteral::String(string) => self
                        .emitter
                        .add_constant(Constant::String(string.value.clone())),
                    HirLiteral::Bool(boolean) => {
                        self.emitter.add_constant(Constant::Bool(boolean.value))
                    }
                    HirLiteral::Unit(_) => self.emitter.unit_const,
                };
                self.instructions.push(Instruction::LoadConst(const_id));
            }
            HirExpr::Name(name) => {
                if let Some(slot) = self.lookup_local(name.name) {
                    self.instructions.push(Instruction::LoadLocal(slot));
                } else {
                    return Err(EmitterError::UnknownName { span: name.span });
                }
            }
            HirExpr::Call(call) => {
                let callee = call.callee.as_ref();
                let name = match callee {
                    HirExpr::Name(name) => name,
                    _ => {
                        return Err(EmitterError::UnsupportedCallee {
                            span: expr_span(callee),
                        })
                    }
                };
                if let Some(func_id) = self.emitter.function_index(name.name) {
                    for arg in &call.args {
                        self.emit_expr(arg)?;
                    }
                    self.instructions
                        .push(Instruction::Call(func_id, call.args.len() as u16));
                } else {
                    let symbol = self
                        .emitter
                        .module
                        .interner
                        .resolve(name.name)
                        .ok_or(EmitterError::UnknownName { span: name.span })?;
                    for arg in &call.args {
                        self.emit_expr(arg)?;
                    }
                    let const_id = self
                        .emitter
                        .add_constant(Constant::String(symbol.to_string()));
                    self.instructions.push(Instruction::CallHostDynamic(
                        const_id,
                        call.args.len() as u16,
                    ));
                }
            }
            HirExpr::If(if_expr) => {
                self.emit_expr(&if_expr.condition)?;
                let jump_false = self.emit_jump_placeholder(true);
                self.emit_block(&if_expr.then_branch, true)?;
                let jump_end = self.emit_jump_placeholder(false);
                let else_start = self.instructions.len();
                self.patch_jump(jump_false, else_start);
                if let Some(else_branch) = &if_expr.else_branch {
                    self.emit_block(else_branch, true)?;
                } else {
                    self.push_unit();
                }
                self.patch_jump(jump_end, self.instructions.len());
            }
            HirExpr::Block(block) => {
                self.emit_block(block, true)?;
            }
            HirExpr::Binary(bin) => {
                self.emit_expr(&bin.lhs)?;
                self.emit_expr(&bin.rhs)?;
                let instr = match bin.op {
                    HirBinaryOp::Add => Instruction::Add,
                    HirBinaryOp::Sub => Instruction::Sub,
                    HirBinaryOp::Mul => Instruction::Mul,
                    HirBinaryOp::Div => Instruction::Div,
                    HirBinaryOp::Eq => Instruction::Eq,
                    HirBinaryOp::Ne => Instruction::Ne,
                    HirBinaryOp::Lt => Instruction::Lt,
                    HirBinaryOp::Le => Instruction::Le,
                    HirBinaryOp::Gt => Instruction::Gt,
                    HirBinaryOp::Ge => Instruction::Ge,
                };
                self.instructions.push(instr);
            }
            HirExpr::Unary(un) => {
                self.emit_expr(&un.expr)?;
                let instr = match un.op {
                    HirUnaryOp::Neg => Instruction::Neg,
                    HirUnaryOp::Not => Instruction::Not,
                };
                self.instructions.push(instr);
            }
        }
        Ok(())
    }

    fn push_unit(&mut self) {
        self.instructions
            .push(Instruction::LoadConst(self.emitter.unit_const));
    }

    fn emit_jump_placeholder(&mut self, conditional: bool) -> usize {
        let pos = self.instructions.len();
        if conditional {
            self.instructions.push(Instruction::JumpIfFalse(usize::MAX));
        } else {
            self.instructions.push(Instruction::Jump(usize::MAX));
        }
        pos
    }

    fn patch_jump(&mut self, index: usize, target: usize) {
        match &mut self.instructions[index] {
            Instruction::Jump(ref mut slot) | Instruction::JumpIfFalse(ref mut slot) => {
                *slot = target;
            }
            _ => {}
        }
    }
}

fn expr_span(expr: &HirExpr) -> Span {
    match expr {
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
