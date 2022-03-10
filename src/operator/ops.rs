use crate::wasmmodule::CallRemovalCandidate;
use parity_wasm::elements::Instruction::{self, *};
use parity_wasm::elements::{BlockType, ValueType};

use super::{InstructionContext, InstructionReplacement};

macro_rules! common_functions {
    () => {
        fn old_instruction(&self) -> &Instruction {
            &self.old
        }
        fn new_instruction(&self) -> &Instruction {
            &self.new
        }

        fn result(&self) -> BlockType {
            self.result_type
        }

        fn parameters(&self) -> &[ValueType] {
            &self.parameters
        }

        fn description(&self) -> String {
            format!(
                "{}: Replaced {:?} with {:?}",
                Self::name(),
                self.old_instruction(),
                self.new_instruction()
            )
        }
    };
}

macro_rules! implement_replacement_op {
    ($op_name:ident, $name:expr, $($from:path => $to:path > $params:expr => $result:expr),* $(,)?) => {
        #[derive(Debug, Clone)]
        pub struct $op_name {
            pub old: Instruction,
            pub new: Instruction,
            pub result_type: BlockType,
            pub parameters: Vec<ValueType>
        }

        impl InstructionReplacement for $op_name {
            common_functions!();

            fn name() -> &'static str {
                $name
            }

            fn replacement(&self) -> Vec<Instruction> {
                vec![self.new_instruction().clone()]
            }


            fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
            where
                Self: Sized + Send + Sync + 'static,
            {
                fn make(instr: &Instruction, _: &InstructionContext) -> Option<Box<dyn InstructionReplacement>> {
                    $op_name::new(instr).map(|f| Box::new(f) as Box<dyn InstructionReplacement >)
                }
                make
            }
        }

        impl $op_name {
            pub fn new(instr: &Instruction) -> Option<Self> {
                match instr {
                    $($from => Some(Self{
                        old: $from,
                        new: $to,
                        result_type: $result,
                        parameters: $params.into()
                    }),)*
                    _ => None
                }
            }
        }
    };
}

use BlockType::Value;
use ValueType::*;

implement_replacement_op! {
    BinaryOperatorAddToSub,
    "binop_add_to_sub",
    I32Add => I32Sub > [I32, I32] => Value(I32),
    I64Add => I64Sub > [I64, I64] => Value(I64),
    F32Add => F32Sub > [F32, F32] => Value(F32),
    F64Add => F64Sub > [F64, F64] => Value(F64)
}

implement_replacement_op! {
    BinaryOperatorSubToAdd,
    "binop_sub_to_add",
    I32Sub => I32Add > [I32, I32] => Value(I32),
    I64Sub => I64Add > [I64, I64] => Value(I64),
    F32Sub => F32Add > [F32, F32] => Value(F32),
    F64Sub => F64Add > [F64, F64] => Value(F64),
}

implement_replacement_op! {
    BinaryOperatorMulToDivU,
    "binop_mul_to_div",
    I32Mul => I32DivU > [I32, I32] => Value(I32),
    I64Mul => I64DivU > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorMulToDivS,
    "binop_mul_to_div",
    I32Mul => I32DivS > [I32, I32] => Value(I32),
    I64Mul => I64DivS > [I64, I64] => Value(I64),
    F32Mul => F32Div  > [F32, F32] => Value(F32),
    F64Mul => F64Div  > [F64, F64] => Value(F64),
}

implement_replacement_op! {
    BinaryOperatorDivXToMul,
    "binop_div_to_mul",
    I32DivS => I32Mul > [I32, I32] => Value(I32),
    I64DivS => I64Mul > [I64, I64] => Value(I64),
    I32DivU => I32Mul > [I32, I32] => Value(I32),
    I64DivU => I64Mul > [I64, I64] => Value(I64),
    F32Div => F32Mul  > [F32, F32] => Value(F32),
    F64Div => F64Mul  > [F64, F64] => Value(F64),
}

implement_replacement_op! {
    BinaryOperatorShlToShrS,
    "binop_shl_to_shr",
    I32Shl => I32ShrS > [I32, I32] => Value(I32),
    I64Shl => I64ShrS > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorShlToShrU,
    "binop_shl_to_shr",
    I32Shl => I32ShrU > [I32, I32] => Value(I32),
    I64Shl => I64ShrU > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorShrXToShl,
    "binop_shr_to_shl",
    I32ShrS => I32Shl > [I32, I32] => Value(I32),
    I32ShrU => I32Shl > [I32, I32] => Value(I32),
    I64ShrS => I64Shl > [I64, I64] => Value(I64),
    I64ShrU => I64Shl > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorRemToDiv,
    "binop_rem_to_div",
    I32RemS => I32DivS > [I32, I32] =>Value(I32),
    I32RemU => I32DivU > [I32, I32] => Value(I32),
    I64RemS => I64DivS > [I64, I64] => Value(I64),
    I64RemU => I64DivU > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorDivToRem,
    "binop_div_to_rem",
    I32DivS => I32RemS > [I32, I32] =>Value(I32),
    I32DivU => I32RemU > [I32, I32] =>Value(I32),
    I64DivS => I64RemS > [I64, I64] => Value(I64),
    I64DivU => I64RemU > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorAndToOr,
    "binop_and_to_or",
    I32And => I32Or > [I32, I32] => Value(I32),
    I64And => I64Or > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorOrToAnd,
    "binop_or_to_and",
    I32Or => I32And > [I32, I32] => Value(I32),
    I64Or => I64And > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorXorToOr,
    "binop_xor_to_or",
    I32Xor => I32Or > [I32, I32] =>Value(I32),
    I64Xor => I64Or > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorOrToXor,
    "binop_or_to_xor",
    I32Or => I32Xor > [I32, I32] =>Value(I32),
    I64Or => I64Xor > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorRotrToRotl,
    "binop_rotr_to_rotl",
    I32Rotr => I32Rotl > [I32, I32] =>Value(I32),
    I64Rotr => I64Rotl > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    BinaryOperatorRotlToRotr,
    "binop_rotl_to_rotr",
    I32Rotl => I32Rotr > [I32, I32] =>Value(I32),
    I64Rotl => I64Rotr > [I64, I64] => Value(I64),
}

implement_replacement_op! {
    UnaryOperatorNegToNop,
    "unop_neg_to_nop",
    F32Neg => Nop > [F32] => Value(F32),
    F64Neg => Nop > [F64] => Value(F64),
}

implement_replacement_op! {
    RelationalOperatorEqToNe,
    "relop_eq_to_ne",
    I32Eq => I32Ne > [I32, I32] => Value(I32),
    I64Eq => I64Ne > [I64, I64] => Value(I32),
    F32Eq => F32Ne > [F32, F32] => Value(I32),
    F64Eq => F64Ne > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorNeToEq,
    "relop_ne_to_eq",
    I32Ne => I32Eq > [I32, I32] => Value(I32),
    I64Ne => I64Eq > [I64, I64] => Value(I32),
    F32Ne => F32Eq > [F32, F32] => Value(I32),
    F64Ne => F64Eq > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorLeToGt,
    "relop_le_to_gt",
    I32LeU => I32GtU > [I32, I32] => Value(I32),
    I64LeU => I64GtU > [I64, I64] => Value(I32),
    I32LeS => I32GtS > [I32, I32] => Value(I32),
    I64LeS => I64GtS > [I64, I64] => Value(I32),
    F32Le => F32Gt > [F32, F32] => Value(I32),
    F64Le => F64Gt > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorLeToLt,
    "relop_le_to_lt",
    I32LeU => I32LtU > [I32, I32] => Value(I32),
    I64LeU => I64LtU > [I64, I64] => Value(I32),
    I32LeS => I32LtS > [I32, I32] => Value(I32),
    I64LeS => I64LtS > [I64, I64] => Value(I32),
    F32Le => F32Lt > [F32, F32] => Value(I32),
    F64Le => F64Lt > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorLtToGe,
    "relop_lt_to_ge",
    I32LtU => I32GeU > [I32, I32] => Value(I32),
    I64LtU => I64GeU > [I64, I64] => Value(I32),
    I32LtS => I32GeS > [I32, I32] => Value(I32),
    I64LtS => I64GeS > [I64, I64] => Value(I32),
    F32Lt => F32Ge > [F32, F32] => Value(I32),
    F64Lt => F64Ge > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorLtToLe,
    "relop_lt_to_le",
    I32LtU => I32LeU > [I32, I32] => Value(I32),
    I64LtU => I64LeU > [I64, I64] => Value(I32),
    I32LtS => I32LeS > [I32, I32] => Value(I32),
    I64LtS => I64LeS > [I64, I64] => Value(I32),
    F32Lt => F32Le > [F32, F32] => Value(I32),
    F64Lt => F64Le > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorGeToGt,
    "relop_ge_to_gt",
    I32GeU => I32GtU > [I32, I32] => Value(I32),
    I64GeU => I64GtU > [I64, I64] => Value(I32),
    I32GeS => I32GtS > [I32, I32] => Value(I32),
    I64GeS => I64GtS > [I64, I64] => Value(I32),
    F32Ge  => F32Gt > [F32, F32] => Value(I32),
    F64Ge  => F64Gt > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorGeToLt,
    "relop_ge_to_lt",
    I32GeU => I32LtU > [I32, I32] => Value(I32),
    I64GeU => I64LtU > [I64, I64] => Value(I32),
    I32GeS => I32LtS > [I32, I32] => Value(I32),
    I64GeS => I64LtS > [I64, I64] => Value(I32),
    F32Ge  => F32Lt > [F32, F32] => Value(I32),
    F64Ge  => F64Lt > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorGtToGe,
    "relop_gt_to_ge",
    I32GtU => I32GeU > [I32, I32] => Value(I32),
    I64GtU => I64GeU > [I64, I64] => Value(I32),
    I32GtS => I32GeS > [I32, I32] => Value(I32),
    I64GtS => I64GeS > [I64, I64] => Value(I32),
    F32Gt  => F32Ge > [F32, F32] => Value(I32),
    F64Gt  => F64Ge > [F64, F64] => Value(I32),
}

implement_replacement_op! {
    RelationalOperatorGtToLe,
    "relop_gt_to_le",
    I32GtU => I32LeU > [I32, I32] => Value(I32),
    I64GtU => I64LeU > [I64, I64] => Value(I32),
    I32GtS => I32LeS > [I32, I32] => Value(I32),
    I64GtS => I64LeS > [I64, I64] => Value(I32),
    F32Gt  => F32Le > [F32, F32] => Value(I32),
    F64Gt  => F64Le > [F64, F64] => Value(I32),
}

#[derive(Debug, Clone)]
pub struct ConstReplaceZero {
    pub old: Instruction,
    pub new: Instruction,
    pub result_type: BlockType,
    pub parameters: Vec<ValueType>,
}

impl InstructionReplacement for ConstReplaceZero {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "const_replace_zero"
    }

    fn replacement(&self) -> Vec<Instruction> {
        vec![self.new_instruction().clone()]
    }
    fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
    where
        Self: Sized + Send + Sync + 'static,
    {
        fn make(
            instr: &Instruction,
            _: &InstructionContext,
        ) -> Option<Box<dyn InstructionReplacement>> {
            ConstReplaceZero::new(instr).map(|f| Box::new(f) as Box<dyn InstructionReplacement>)
        }

        make
    }
}

impl ConstReplaceZero {
    pub fn new(instr: &Instruction) -> Option<Self> {
        match *instr {
            I32Const(i) if i == 0 => Some(Self {
                old: I32Const(i),
                new: I32Const(42),
                result_type: Value(I32),
                parameters: [].into(),
            }),
            I64Const(i) if i == 0 => Some(Self {
                old: I64Const(i),
                new: I64Const(42),
                result_type: Value(I64),
                parameters: [].into(),
            }),
            F32Const(i) if f32::from_bits(i) == 0.0 => Some(Self {
                old: F32Const(i),
                new: F32Const(42f32.to_bits()),
                result_type: Value(F32),
                parameters: [].into(),
            }),
            F64Const(i) if f64::from_bits(i) == 0.0 => Some(Self {
                old: F64Const(i),
                new: F64Const(42f64.to_bits()),
                result_type: Value(F64),
                parameters: [].into(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstReplaceNonZero {
    pub old: Instruction,
    pub new: Instruction,
    pub result_type: BlockType,
    pub parameters: Vec<ValueType>,
}

impl InstructionReplacement for ConstReplaceNonZero {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "const_replace_nonzero"
    }

    fn replacement(&self) -> Vec<Instruction> {
        vec![self.new_instruction().clone()]
    }

    fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
    where
        Self: Sized + Send + Sync + 'static,
    {
        fn make(
            instr: &Instruction,
            _: &InstructionContext,
        ) -> Option<Box<dyn InstructionReplacement>> {
            ConstReplaceNonZero::new(instr).map(|f| Box::new(f) as Box<dyn InstructionReplacement>)
        }

        make
    }
}

impl ConstReplaceNonZero {
    pub fn new(instr: &Instruction) -> Option<Self> {
        match *instr {
            I32Const(i) if i != 0 => Some(Self {
                old: I32Const(i),
                new: I32Const(0),
                result_type: Value(I32),
                parameters: [].into(),
            }),
            I64Const(i) if i != 0 => Some(Self {
                old: I64Const(i),
                new: I64Const(0),
                result_type: Value(I64),
                parameters: [].into(),
            }),
            F32Const(i) if f32::from_bits(i) != 0.0 => Some(Self {
                old: F32Const(i),
                new: F32Const(0f32.to_bits()),
                result_type: Value(F32),
                parameters: [].into(),
            }),
            F64Const(i) if f64::from_bits(i) != 0.0 => Some(Self {
                old: F64Const(i),
                new: F64Const(0f64.to_bits()),
                result_type: Value(F64),
                parameters: [].into(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallRemoveVoidCall {
    pub old: Instruction,
    pub new: Instruction,
    pub result_type: BlockType,
    pub parameters: Vec<ValueType>,
}
impl InstructionReplacement for CallRemoveVoidCall {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "call_remove_void_call"
    }

    fn replacement(&self) -> Vec<Instruction> {
        let mut replacement = vec![Drop; self.parameters.len()];
        replacement.push(self.new_instruction().clone());
        replacement
    }

    fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
    where
        Self: Sized + Send + Sync + 'static,
    {
        fn make(
            instr: &Instruction,
            ctx: &InstructionContext,
        ) -> Option<Box<dyn InstructionReplacement>> {
            CallRemoveVoidCall::new(instr, ctx)
                .map(|f| Box::new(f) as Box<dyn InstructionReplacement>)
        }

        make
    }
}

impl CallRemoveVoidCall {
    pub fn new(instr: &Instruction, ctx: &InstructionContext) -> Option<Self> {
        match *instr {
            Call(func_ref) => {
                for candidate in ctx.call_removal_candidates() {
                    if let CallRemovalCandidate::FuncReturningVoid { index, params } = candidate {
                        if *index == func_ref {
                            return Some(Self {
                                old: instr.clone(),
                                new: Nop,
                                result_type: BlockType::NoResult,
                                parameters: params.clone(),
                            });
                        }
                    }
                }

                None
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]

pub struct CallRemoveScalarCall {
    pub old: Instruction,
    pub new: Instruction,
    pub result_type: BlockType,
    pub parameters: Vec<ValueType>,
}

impl InstructionReplacement for CallRemoveScalarCall {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "call_remove_scalar_call"
    }

    fn replacement(&self) -> Vec<Instruction> {
        let mut replacement = vec![Drop; self.parameters.len()];
        replacement.push(self.new_instruction().clone());
        replacement
    }

    fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
    where
        Self: Sized + Send + Sync + 'static,
    {
        fn make(
            instr: &Instruction,
            ctx: &InstructionContext,
        ) -> Option<Box<dyn InstructionReplacement>> {
            CallRemoveScalarCall::new(instr, ctx)
                .map(|f| Box::new(f) as Box<dyn InstructionReplacement>)
        }

        make
    }
}

impl CallRemoveScalarCall {
    pub fn new(instr: &Instruction, ctx: &InstructionContext) -> Option<Self> {
        match *instr {
            Call(func_ref) => {
                for candidate in ctx.call_removal_candidates() {
                    if let CallRemovalCandidate::FuncReturningScalar {
                        index,
                        params,
                        return_type,
                    } = candidate
                    {
                        if *index == func_ref {
                            let replacement = match return_type {
                                ValueType::I32 => I32Const(42),
                                ValueType::I64 => I64Const(42),
                                ValueType::F32 => F32Const(42f32.to_bits()),
                                ValueType::F64 => F64Const(42f64.to_bits()),
                            };

                            let result_type = Value(*return_type);

                            return Some(Self {
                                old: instr.clone(),
                                new: replacement,
                                result_type,
                                parameters: params.clone(),
                            });
                        }
                    }
                }

                None
            }
            _ => None,
        }
    }
}
