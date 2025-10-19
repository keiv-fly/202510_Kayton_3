use std::sync::Arc;

use kayton_api::{KayCtx, KayError, KayHandle, KayValueKind};
use kayton_bytecode::{BytecodeModule, ConstId, Constant, FunctionId, HostSlot, Instruction};
use kayton_host::KayHost;
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(Arc<str>),
    Unit,
    Handle(KayHandle),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(lhs), Value::Int(rhs)) => lhs == rhs,
            (Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
            (Value::Str(lhs), Value::Str(rhs)) => lhs == rhs,
            (Value::Unit, Value::Unit) => true,
            (Value::Handle(lhs), Value::Handle(rhs)) => lhs.raw() == rhs.raw(),
            _ => false,
        }
    }
}

impl Value {
    fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<&Constant> for Value {
    fn from(constant: &Constant) -> Self {
        match constant {
            Constant::Int(v) => Value::Int(*v),
            Constant::Bool(v) => Value::Bool(*v),
            Constant::String(s) => Value::Str(Arc::from(s.as_str())),
            Constant::Unit => Value::Unit,
        }
    }
}

#[derive(Debug, Error)]
pub enum VmError {
    #[error("entry function `{0}` not found")]
    EntryNotFound(String),
    #[error("function index {0} out of range")]
    BadFunction(FunctionId),
    #[error("constant index {0} out of range")]
    BadConstant(ConstId),
    #[error("host call requires a string constant")]
    HostNameType,
    #[error("local index out of range")]
    BadLocal,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("type error: expected {expected}")]
    TypeError { expected: &'static str },
    #[error("call arity mismatch: expected {expected}, found {found}")]
    CallArity { expected: usize, found: usize },
    #[error("host call failed: {0:?}")]
    HostFailure(KayError),
}

impl From<KayError> for VmError {
    fn from(value: KayError) -> Self {
        VmError::HostFailure(value)
    }
}

struct Frame {
    function: FunctionId,
    ip: usize,
    locals: Vec<Value>,
}

impl Frame {
    fn new(function_id: FunctionId, locals: Vec<Value>) -> Self {
        Self {
            function: function_id,
            ip: 0,
            locals,
        }
    }
}

pub fn run_module(module: &BytecodeModule, entry: &str, host: &KayHost) -> Result<Value, VmError> {
    let entry_id = module
        .function_index(entry)
        .ok_or_else(|| VmError::EntryNotFound(entry.to_string()))?;
    let ctx = host.api_ctx();
    let mut vm = Vm::new(module, ctx);
    vm.run(entry_id)
}

struct Vm<'a> {
    module: &'a BytecodeModule,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    ctx: KayCtx,
}

impl<'a> Vm<'a> {
    fn new(module: &'a BytecodeModule, ctx: KayCtx) -> Self {
        Self {
            module,
            stack: Vec::new(),
            frames: Vec::new(),
            ctx,
        }
    }

    fn run(&mut self, entry: FunctionId) -> Result<Value, VmError> {
        self.call_function(entry, Vec::new())?;
        loop {
            let frame_index = match self.frames.len() {
                0 => return Ok(Value::Unit),
                len => len - 1,
            };
            let current_ip = self.frames[frame_index].ip;
            let function_id = self.frames[frame_index].function;
            let instruction = {
                let function = self
                    .module
                    .functions
                    .get(function_id as usize)
                    .ok_or(VmError::BadFunction(function_id))?;
                if current_ip >= function.instructions.len() {
                    return Err(VmError::BadFunction(function_id));
                }
                function.instructions[current_ip].clone()
            };
            match instruction {
                Instruction::LoadConst(id) => {
                    let value = self
                        .module
                        .constants
                        .get(id as usize)
                        .map(Value::from)
                        .unwrap_or(Value::Unit);
                    self.stack.push(value);
                    self.advance_ip(frame_index);
                }
                Instruction::LoadLocal(idx) => {
                    let value = self.frames[frame_index]
                        .locals
                        .get(idx as usize)
                        .cloned()
                        .ok_or(VmError::BadLocal)?;
                    self.stack.push(value);
                    self.advance_ip(frame_index);
                }
                Instruction::StoreLocal(idx) => {
                    let value = self.pop()?;
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        let slot = frame
                            .locals
                            .get_mut(idx as usize)
                            .ok_or(VmError::BadLocal)?;
                        *slot = value;
                        frame.ip += 1;
                    }
                }
                Instruction::Jump(target) => {
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip = target;
                    }
                }
                Instruction::JumpIfFalse(target) => {
                    let cond = self.pop_bool()?;
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        if !cond {
                            frame.ip = target;
                        } else {
                            frame.ip += 1;
                        }
                    }
                }
                Instruction::Add => {
                    self.binary_int(|a, b| a + b)?;
                    self.advance_ip(frame_index);
                }
                Instruction::Sub => {
                    self.binary_int(|a, b| a - b)?;
                    self.advance_ip(frame_index);
                }
                Instruction::Mul => {
                    self.binary_int(|a, b| a * b)?;
                    self.advance_ip(frame_index);
                }
                Instruction::Div => {
                    self.binary_int(|a, b| a / b)?;
                    self.advance_ip(frame_index);
                }
                Instruction::Neg => {
                    let value = self.pop_int()?;
                    self.stack.push(Value::Int(-value));
                    self.advance_ip(frame_index);
                }
                Instruction::Not => {
                    let value = self.pop_bool()?;
                    self.stack.push(Value::Bool(!value));
                    self.advance_ip(frame_index);
                }
                Instruction::Eq => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs == rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Ne => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs != rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Lt => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs < rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Le => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs <= rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Gt => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs > rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Ge => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs >= rhs));
                    self.advance_ip(frame_index);
                }
                Instruction::Call(func, arg_count) => {
                    let mut args = Vec::with_capacity(arg_count as usize);
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    self.call_function(func, args)?;
                }
                Instruction::CallHost(slot, arg_count) => {
                    let result = self.invoke_host(slot, arg_count)?;
                    self.stack.push(result);
                    self.advance_ip(frame_index);
                }
                Instruction::CallHostDynamic(name_const, arg_count) => {
                    let name = self
                        .module
                        .constants
                        .get(name_const as usize)
                        .ok_or(VmError::BadConstant(name_const))?;
                    let symbol = if let Constant::String(sym) = name {
                        sym.clone()
                    } else {
                        return Err(VmError::HostNameType);
                    };
                    let result = self.invoke_host_dynamic(symbol, arg_count)?;
                    self.stack.push(result);
                    self.advance_ip(frame_index);
                }
                Instruction::Return => {
                    let result = self.stack.pop().unwrap_or(Value::Unit);
                    self.frames.pop();
                    if let Some(prev) = self.frames.last_mut() {
                        self.stack.push(result);
                        prev.ip += 1;
                    } else {
                        return Ok(result);
                    }
                }
                Instruction::Pop => {
                    self.pop()?;
                    self.advance_ip(frame_index);
                }
            }
        }
    }

    fn invoke_host(&mut self, slot: HostSlot, arg_count: u16) -> Result<Value, VmError> {
        let args = self.collect_host_args(arg_count)?;
        let handle = self.ctx.call_slot(slot, &args).map_err(VmError::from)?;
        self.handle_to_value(handle)
    }

    fn invoke_host_dynamic(&mut self, name: String, arg_count: u16) -> Result<Value, VmError> {
        let args = self.collect_host_args(arg_count)?;
        let handle = self.ctx.call_dynamic(&name, &args).map_err(VmError::from)?;
        self.handle_to_value(handle)
    }

    fn collect_host_args(&mut self, arg_count: u16) -> Result<Vec<KayHandle>, VmError> {
        let mut handles = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
            let value = self.pop()?;
            let handle = self.ensure_handle(value)?;
            handles.push(handle);
        }
        handles.reverse();
        Ok(handles)
    }

    fn ensure_handle(&mut self, value: Value) -> Result<KayHandle, VmError> {
        match value {
            Value::Int(v) => self.ctx.alloc_int(v).map_err(VmError::from),
            Value::Bool(v) => self.ctx.alloc_bool(v).map_err(VmError::from),
            Value::Str(s) => self.ctx.alloc_string(s).map_err(VmError::from),
            Value::Unit => self.ctx.alloc_unit().map_err(VmError::from),
            Value::Handle(handle) => Ok(handle),
        }
    }

    fn handle_to_value(&self, handle: KayHandle) -> Result<Value, VmError> {
        match handle.describe().map_err(VmError::from)? {
            KayValueKind::Int(value) => Ok(Value::Int(value)),
            KayValueKind::Bool(value) => Ok(Value::Bool(value)),
            KayValueKind::Unit => Ok(Value::Unit),
            KayValueKind::String(_) | KayValueKind::Bytes(_) | KayValueKind::Capsule { .. } => {
                Ok(Value::Handle(handle))
            }
        }
    }

    fn call_function(&mut self, func_id: FunctionId, args: Vec<Value>) -> Result<(), VmError> {
        let function = self
            .module
            .functions
            .get(func_id as usize)
            .ok_or(VmError::BadFunction(func_id))?;
        let expected = function.params as usize;
        if expected != args.len() {
            return Err(VmError::CallArity {
                expected,
                found: args.len(),
            });
        }
        let mut locals = vec![Value::Unit; function.locals as usize];
        for (idx, arg) in args.into_iter().enumerate() {
            locals[idx] = arg;
        }
        self.frames.push(Frame::new(func_id, locals));
        Ok(())
    }

    fn advance_ip(&mut self, frame_index: usize) {
        if let Some(frame) = self.frames.get_mut(frame_index) {
            frame.ip += 1;
        }
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn pop_int(&mut self) -> Result<i64, VmError> {
        self.pop()?
            .as_int()
            .ok_or(VmError::TypeError { expected: "int" })
    }

    fn pop_bool(&mut self) -> Result<bool, VmError> {
        self.pop()?
            .as_bool()
            .ok_or(VmError::TypeError { expected: "bool" })
    }

    fn binary_int<F>(&mut self, op: F) -> Result<(), VmError>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let rhs = self.pop_int()?;
        let lhs = self.pop_int()?;
        self.stack.push(Value::Int(op(lhs, rhs)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kayton_emitter_bc::emit;
    use kayton_front::tests_support::parse_str;
    use kayton_host::KayHost;
    use kayton_sema::fast::analyze;

    fn compile_and_run(source: &str) -> Value {
        let parsed = parse_str("test.ktn", source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let analysis = analyze(&parsed.module);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let module = emit(&parsed.module, &analysis).expect("emit");
        let host = KayHost::new();
        host.register_extensions(kayton_stdlib::extensions())
            .expect("register stdlib");
        run_module(&module, "main", &host).expect("vm run")
    }

    #[test]
    fn runs_arithmetic() {
        let value = compile_and_run(
            r#"
fn main():
    let x = 2
    let y = 3
    x + y
"#,
        );
        assert_eq!(value, Value::Int(5));
    }

    #[test]
    fn runs_branch() {
        let value = compile_and_run(
            r#"
fn main():
    if 1 < 2:
        10
    else:
        20
"#,
        );
        assert_eq!(value, Value::Int(10));
    }

    #[test]
    fn runs_recursion() {
        let value = compile_and_run(
            r#"
fn fact(n):
    if n < 2:
        1
    else:
        n * fact(n - 1)

fn main():
    fact(5)
"#,
        );
        assert_eq!(value, Value::Int(120));
    }

    #[test]
    fn calls_host_extension() {
        let value = compile_and_run(
            r#"
fn main():
    len("hi")
"#,
        );
        assert_eq!(value, Value::Int(2));
    }
}
