use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

pub type ConstId = u32;
pub type FunctionId = u32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constant {
    Int(i64),
    Bool(bool),
    String(String),
    Unit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Global {
    pub name: SmolStr,
    pub value: ConstId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: SmolStr,
    pub params: u16,
    pub locals: u16,
    pub instructions: Vec<Instruction>,
}

impl Function {
    pub fn new(
        name: impl Into<SmolStr>,
        params: u16,
        locals: u16,
        instructions: Vec<Instruction>,
    ) -> Self {
        Self {
            name: name.into(),
            params,
            locals,
            instructions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    LoadConst(ConstId),
    LoadLocal(u16),
    StoreLocal(u16),
    Jump(usize),
    JumpIfFalse(usize),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Call(FunctionId, u16),
    Return,
    Pop,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BytecodeModule {
    pub constants: Vec<Constant>,
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
}

impl BytecodeModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_constant(&mut self, constant: Constant) -> ConstId {
        let id = self.constants.len() as ConstId;
        self.constants.push(constant);
        id
    }

    pub fn add_global(&mut self, name: impl Into<SmolStr>, value: ConstId) {
        self.globals.push(Global {
            name: name.into(),
            value,
        });
    }

    pub fn add_function(&mut self, function: Function) -> FunctionId {
        let id = self.functions.len() as FunctionId;
        self.functions.push(function);
        id
    }

    pub fn function_index(&self, name: &str) -> Option<FunctionId> {
        self.functions
            .iter()
            .position(|f| f.name.as_str() == name)
            .map(|idx| idx as FunctionId)
    }

    pub fn serialize(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    pub fn deserialize(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    pub fn verify(&self) -> Result<(), VerificationError> {
        Verifier.verify(self)
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum VerificationError {
    #[error("constant index out of bounds at instruction {instruction}")]
    BadConstant { instruction: usize },
    #[error("local index out of bounds at instruction {instruction}")]
    BadLocal { instruction: usize },
    #[error("function index out of bounds at instruction {instruction}")]
    BadFunction { instruction: usize },
    #[error("jump target out of bounds at instruction {instruction}")]
    BadJump { instruction: usize },
}

#[derive(Default)]
pub struct Verifier;

impl Verifier {
    pub fn verify(&self, module: &BytecodeModule) -> Result<(), VerificationError> {
        for function in &module.functions {
            let local_limit = function.locals as usize;
            for (idx, instr) in function.instructions.iter().enumerate() {
                match instr {
                    Instruction::LoadConst(id) => {
                        if module.constants.get(*id as usize).is_none() {
                            return Err(VerificationError::BadConstant { instruction: idx });
                        }
                    }
                    Instruction::LoadLocal(local) | Instruction::StoreLocal(local) => {
                        if (*local as usize) >= local_limit {
                            return Err(VerificationError::BadLocal { instruction: idx });
                        }
                    }
                    Instruction::Call(func, _) => {
                        if module.functions.get(*func as usize).is_none() {
                            return Err(VerificationError::BadFunction { instruction: idx });
                        }
                    }
                    Instruction::Jump(target) | Instruction::JumpIfFalse(target) => {
                        if *target >= function.instructions.len() {
                            return Err(VerificationError::BadJump { instruction: idx });
                        }
                    }
                    Instruction::Add
                    | Instruction::Sub
                    | Instruction::Mul
                    | Instruction::Div
                    | Instruction::Neg
                    | Instruction::Not
                    | Instruction::Eq
                    | Instruction::Ne
                    | Instruction::Lt
                    | Instruction::Le
                    | Instruction::Gt
                    | Instruction::Ge
                    | Instruction::Return
                    | Instruction::Pop => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_round_trip() {
        let mut module = BytecodeModule::new();
        let unit = module.add_constant(Constant::Unit);
        module.add_global("ANSWER", unit);
        module.add_function(Function::new(
            "main",
            0,
            0,
            vec![Instruction::LoadConst(unit), Instruction::Return],
        ));

        let bytes = module.serialize().expect("serialize");
        let decoded = BytecodeModule::deserialize(&bytes).expect("deserialize");
        assert_eq!(module.constants, decoded.constants);
        assert_eq!(module.globals.len(), decoded.globals.len());
        assert_eq!(module.functions.len(), decoded.functions.len());
    }
}
