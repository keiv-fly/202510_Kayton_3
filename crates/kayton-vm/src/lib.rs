use kayton_bytecode::{BytecodeModule, Constant, FunctionId, Instruction};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    String(String),
    Unit,
}

impl Value {
    fn as_int(&self) -> Option<i64> {
        if let Value::Int(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

impl From<&Constant> for Value {
    fn from(constant: &Constant) -> Self {
        match constant {
            Constant::Int(v) => Value::Int(*v),
            Constant::Bool(v) => Value::Bool(*v),
            Constant::String(s) => Value::String(s.clone()),
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
    #[error("local index out of range")]
    BadLocal,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("type error: expected {expected}")]
    TypeError { expected: &'static str },
    #[error("call arity mismatch: expected {expected}, found {found}")]
    CallArity { expected: usize, found: usize },
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

pub fn run_module(module: &BytecodeModule, entry: &str) -> Result<Value, VmError> {
    let entry_id = module
        .function_index(entry)
        .ok_or_else(|| VmError::EntryNotFound(entry.to_string()))?;
    let mut vm = Vm::new(module);
    vm.run(entry_id)
}

struct Vm<'a> {
    module: &'a BytecodeModule,
    stack: Vec<Value>,
    frames: Vec<Frame>,
}

impl<'a> Vm<'a> {
    fn new(module: &'a BytecodeModule) -> Self {
        Self {
            module,
            stack: Vec::new(),
            frames: Vec::new(),
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
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::LoadLocal(idx) => {
                    let value = self.frames[frame_index]
                        .locals
                        .get(idx as usize)
                        .cloned()
                        .ok_or(VmError::BadLocal)?;
                    self.stack.push(value);
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
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
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Sub => {
                    self.binary_int(|a, b| a - b)?;
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Mul => {
                    self.binary_int(|a, b| a * b)?;
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Div => {
                    self.binary_int(|a, b| a / b)?;
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Neg => {
                    let value = self.pop_int()?;
                    self.stack.push(Value::Int(-value));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Not => {
                    let value = self.pop_bool()?;
                    self.stack.push(Value::Bool(!value));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Eq => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs == rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Ne => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs != rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Lt => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs < rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Le => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs <= rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Gt => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs > rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Ge => {
                    let rhs = self.pop_int()?;
                    let lhs = self.pop_int()?;
                    self.stack.push(Value::Bool(lhs >= rhs));
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
                Instruction::Call(func, arg_count) => {
                    let mut args = Vec::with_capacity(arg_count as usize);
                    for _ in 0..arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    self.call_function(func, args)?;
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
                    if let Some(frame) = self.frames.get_mut(frame_index) {
                        frame.ip += 1;
                    }
                }
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
        run_module(&module, "main").expect("vm run")
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
}
