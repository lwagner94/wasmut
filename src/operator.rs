use parity_wasm::elements::Instruction as ParityInstruction;

// #[derive(Debug)]
// pub enum MutationType {
//     BinaryOperatorReplacement(BinaryOperator, BinaryOperator),
//     // ConstantReplacenment(Constant, Constant),
// }

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    I32Add,
    I32Sub,
    I32Mul,
    I32DivU,
    I32DivS,
}

#[derive(Debug, Clone)]

pub enum Constant {
    I32(i32),
    I64(i64),
    // F32(f32),
    // F64(f64),
}

use BinaryOperator::*;
use Constant::*;

#[derive(Debug, Clone)]
pub enum MutableInstruction {
    BinaryOperatorInstr(BinaryOperator),
    ConstantInstr(Constant),
}

use MutableInstruction::*;

impl MutableInstruction {
    pub fn from_parity_instruction(parity_instruction: &ParityInstruction) -> Option<Self> {
        match parity_instruction {
            ParityInstruction::I32Add => Some(BinaryOperatorInstr(I32Add)),
            ParityInstruction::I32Sub => Some(BinaryOperatorInstr(I32Sub)),
            ParityInstruction::I32Mul => Some(BinaryOperatorInstr(I32Mul)),
            ParityInstruction::I32DivU => Some(BinaryOperatorInstr(I32DivU)),
            ParityInstruction::I32DivS => Some(BinaryOperatorInstr(I32DivS)),
            ParityInstruction::I32Const(i) => Some(ConstantInstr(I32(*i))),
            ParityInstruction::I64Const(i) => Some(ConstantInstr(I64(*i))),
            // ParityInstruction::F32Const(f) => Some(ConstantInstr(F32(f))),
            // ParityInstruction::F64Const(f) => Some(ConstantInstr(F64(f))),
            _ => None,
        }
    }

    pub fn generate_mutanted_instructions(&self) -> Vec<MutableInstruction> {
        let mut mutated_instructions = Vec::new();
        match self {
            BinaryOperatorInstr(op_istr) => match op_istr {
                I32Add => {
                    mutated_instructions.push(BinaryOperatorInstr(I32Sub));
                    mutated_instructions.push(BinaryOperatorInstr(I32Mul));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivU));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivS));
                }
                I32Sub => {
                    mutated_instructions.push(BinaryOperatorInstr(I32Add));
                    mutated_instructions.push(BinaryOperatorInstr(I32Mul));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivU));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivS));
                }
                I32Mul => {
                    mutated_instructions.push(BinaryOperatorInstr(I32Add));
                    mutated_instructions.push(BinaryOperatorInstr(I32Sub));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivU));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivS));
                }
                I32DivU => {
                    mutated_instructions.push(BinaryOperatorInstr(I32Add));
                    mutated_instructions.push(BinaryOperatorInstr(I32Sub));
                    mutated_instructions.push(BinaryOperatorInstr(I32Mul));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivS));
                }
                I32DivS => {
                    mutated_instructions.push(BinaryOperatorInstr(I32Add));
                    mutated_instructions.push(BinaryOperatorInstr(I32Sub));
                    mutated_instructions.push(BinaryOperatorInstr(I32Mul));
                    mutated_instructions.push(BinaryOperatorInstr(I32DivU));
                }
            },
            ConstantInstr(c) => match c {
                // all constant types
                I32(_i) => {
                    mutated_instructions.push(ConstantInstr(I32(0)));
                    mutated_instructions.push(ConstantInstr(I32(1)));
                    mutated_instructions.push(ConstantInstr(I32(-1)));
                    mutated_instructions.push(ConstantInstr(I32(42)));
                    mutated_instructions.push(ConstantInstr(I32(-42)));
                }
                I64(_i) => {
                    mutated_instructions.push(ConstantInstr(I64(0)));
                    mutated_instructions.push(ConstantInstr(I64(1)));
                    mutated_instructions.push(ConstantInstr(I64(-1)));
                    mutated_instructions.push(ConstantInstr(I64(42)));
                    mutated_instructions.push(ConstantInstr(I64(-42)));
                } // F32(f) => {
                  //     //mutated_instructions.push(ConstantInstr(F32(f + 1.0)));

                  // }
                  // F64(f) => {
                  //     //mutated_instructions.push(ConstantInstr(F64(f + 1.0)));
                  // }
            },
        }

        mutated_instructions
    }

    pub fn parity_instruction(&self) -> ParityInstruction {
        match self {
            BinaryOperatorInstr(op_istr) => match op_istr {
                I32Add => ParityInstruction::I32Add,
                I32Sub => ParityInstruction::I32Sub,
                I32Mul => ParityInstruction::I32Mul,
                I32DivU => ParityInstruction::I32DivU,
                I32DivS => ParityInstruction::I32DivS,
            },
            ConstantInstr(c) => {
                match c {
                    I32(i) => ParityInstruction::I32Const(*i),
                    I64(i) => ParityInstruction::I64Const(*i),
                    // F32(f) => ParityInstruction::F32Const(f),
                    // F64(f) => ParityInstruction::F64Const(f),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Mutation {
    pub function_number: usize,
    pub statement_number: usize,
    pub instruction: MutableInstruction,
}
