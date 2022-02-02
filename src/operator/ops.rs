use parity_wasm::elements::Instruction::{self, *};

use crate::wasmmodule::{CallRemovalCandidate, Datatype};

use super::{InstructionContext, InstructionReplacement};

macro_rules! common_functions {
    () => {
        fn old_instruction(&self) -> &Instruction {
            &self.0
        }
        fn new_instruction(&self) -> &Instruction {
            &self.1
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
    ($op_name:ident, $name:expr, $($from:path => $to:path),* $(,)?) => {

        pub struct $op_name(Instruction, Instruction);
        impl InstructionReplacement for $op_name {
            common_functions!();

            fn name() -> &'static str {
                $name
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
            fn new(instr: &Instruction) -> Option<Self> {
                match instr {
                    $($from => Some(Self($from, $to)),)*
                    _ => None
                }
            }
        }
    };
}

implement_replacement_op! {
    BinaryOperatorAddToSub,
    "binop_add_to_sub",
    I32Add => I32Sub,
    I64Add => I64Sub,
    F32Add => F32Sub,
    F64Add => F64Sub
}

implement_replacement_op! {
    BinaryOperatorSubToAdd,
    "binop_sub_to_add",
    I32Sub => I32Add,
    I64Sub => I64Add,
    F32Sub => F32Add,
    F64Sub => F64Add
}

implement_replacement_op! {
    BinaryOperatorMulToDivU,
    "binop_mul_to_div",
    I32Mul => I32DivU,
    I64Mul => I64DivU,
}

implement_replacement_op! {
    BinaryOperatorMulToDivS,
    "binop_mul_to_div",
    I32Mul => I32DivS,
    I64Mul => I64DivS,
    F32Mul => F32Div,
    F64Mul => F64Div,
}

implement_replacement_op! {
    BinaryOperatorDivXToMul,
    "binop_div_to_mul",
    I32DivS => I32Mul,
    I64DivS => I64Mul,
    I32DivU => I32Mul,
    I64DivU => I64Mul,
    F32Div => F32Mul,
    F64Div => F64Mul,
}

implement_replacement_op! {
    BinaryOperatorShlToShrS,
    "binop_shl_to_shr",
    I32Shl => I32ShrS,
    I64Shl => I64ShrS,
}

implement_replacement_op! {
    BinaryOperatorShlToShrU,
    "binop_shl_to_shr",
    I32Shl => I32ShrU,
    I64Shl => I64ShrU,
}

implement_replacement_op! {
    BinaryOperatorShrXToShl,
    "binop_shr_to_shl",
    I32ShrS => I32Shl,
    I32ShrU => I32Shl,
    I64ShrS => I64Shl,
    I64ShrU => I64Shl,
}

implement_replacement_op! {
    BinaryOperatorRemToDiv,
    "binop_rem_to_div",
    I32RemS => I32DivS,
    I32RemU => I32DivU,
    I64RemS => I64DivS,
    I64RemU => I64DivU
}

implement_replacement_op! {
    BinaryOperatorDivToRem,
    "binop_div_to_rem",
    I32DivS => I32RemS,
    I32DivU => I32RemU,
    I64DivS => I64RemS,
    I64DivU => I64RemU
}

implement_replacement_op! {
    BinaryOperatorAndToOr,
    "binop_and_to_or",
    I32And => I32Or,
    I64And => I64Or
}

implement_replacement_op! {
    BinaryOperatorOrToAnd,
    "binop_or_to_and",
    I32Or => I32And,
    I64Or => I64And
}

implement_replacement_op! {
    BinaryOperatorXorToOr,
    "binop_xor_to_or",
    I32Xor => I32Or,
    I64Xor => I64Or
}

implement_replacement_op! {
    BinaryOperatorOrToXor,
    "binop_or_to_xor",
    I32Or => I32Xor,
    I64Or => I64Xor
}

implement_replacement_op! {
    BinaryOperatorRotrToRotl,
    "binop_rotr_to_rotl",
    I32Rotr => I32Rotl,
    I64Rotr => I64Rotl
}

implement_replacement_op! {
    BinaryOperatorRotlToRotr,
    "binop_rotl_to_rotr",
    I32Rotl => I32Rotr,
    I64Rotl => I64Rotr
}

implement_replacement_op! {
    UnaryOperatorNegToNop,
    "unop_neg_to_nop",
    F32Neg => Nop,
    F64Neg => Nop,
}

implement_replacement_op! {
    RelationalOperatorEqToNe,
    "relop_eq_to_ne",
    I32Eq => I32Ne,
    I64Eq => I64Ne,
    F32Eq => F32Ne,
    F64Eq => F64Ne,
}

implement_replacement_op! {
    RelationalOperatorNeToEq,
    "relop_ne_to_eq",
    I32Ne => I32Eq,
    I64Ne => I64Eq,
    F32Ne => F32Eq,
    F64Ne => F64Eq,
}

implement_replacement_op! {
    RelationalOperatorLeToGt,
    "relop_le_to_gt",
    I32LeU => I32GtU,
    I64LeU => I64GtU,
    I32LeS => I32GtS,
    I64LeS => I64GtS,
    F32Le => F32Gt,
    F64Le => F64Gt,
}

implement_replacement_op! {
    RelationalOperatorLeToLt,
    "relop_le_to_lt",
    I32LeU => I32LtU,
    I64LeU => I64LtU,
    I32LeS => I32LtS,
    I64LeS => I64LtS,
    F32Le => F32Lt,
    F64Le => F64Lt,
}

implement_replacement_op! {
    RelationalOperatorLtToGe,
    "relop_lt_to_ge",
    I32LtU => I32GeU,
    I64LtU => I64GeU,
    I32LtS => I32GeS,
    I64LtS => I64GeS,
    F32Lt => F32Ge,
    F64Lt => F64Ge,
}

implement_replacement_op! {
    RelationalOperatorLtToLe,
    "relop_lt_to_le",
    I32LtU => I32LeU,
    I64LtU => I64LeU,
    I32LtS => I32LeS,
    I64LtS => I64LeS,
    F32Lt => F32Le,
    F64Lt => F64Le,
}

implement_replacement_op! {
    RelationalOperatorGeToGt,
    "relop_ge_to_gt",
    I32GeU => I32GtU,
    I64GeU => I64GtU,
    I32GeS => I32GtS,
    I64GeS => I64GtS,
    F32Ge  => F32Gt,
    F64Ge  => F64Gt,
}

implement_replacement_op! {
    RelationalOperatorGeToLt,
    "relop_ge_to_lt",
    I32GeU => I32LtU,
    I64GeU => I64LtU,
    I32GeS => I32LtS,
    I64GeS => I64LtS,
    F32Ge  => F32Lt,
    F64Ge  => F64Lt,
}

implement_replacement_op! {
    RelationalOperatorGtToGe,
    "relop_gt_to_ge",
    I32GtU => I32GeU,
    I64GtU => I64GeU,
    I32GtS => I32GeS,
    I64GtS => I64GeS,
    F32Gt  => F32Ge,
    F64Gt  => F64Ge,
}

implement_replacement_op! {
    RelationalOperatorGtToLe,
    "relop_gt_to_le",
    I32GtU => I32LeU,
    I64GtU => I64LeU,
    I32GtS => I32LeS,
    I64GtS => I64LeS,
    F32Gt  => F32Le,
    F64Gt  => F64Le,
}
pub struct ConstReplaceZero(Instruction, Instruction);
impl InstructionReplacement for ConstReplaceZero {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "const_replace_zero"
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
    fn new(instr: &Instruction) -> Option<Self> {
        match *instr {
            I32Const(i) if i == 0 => Some(Self(I32Const(i), I32Const(42))),
            I64Const(i) if i == 0 => Some(Self(I64Const(i), I64Const(42))),
            F32Const(i) if f32::from_bits(i) == 0.0 => {
                Some(Self(F32Const(i), F32Const(42f32.to_bits())))
            }
            F64Const(i) if f64::from_bits(i) == 0.0 => {
                Some(Self(F64Const(i), F64Const(42f64.to_bits())))
            }
            _ => None,
        }
    }
}

pub struct ConstReplaceNonZero(Instruction, Instruction);
impl InstructionReplacement for ConstReplaceNonZero {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "const_replace_nonzero"
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
    fn new(instr: &Instruction) -> Option<Self> {
        match *instr {
            I32Const(i) if i != 0 => Some(Self(I32Const(i), I32Const(0))),
            I64Const(i) if i != 0 => Some(Self(I64Const(i), I64Const(0))),
            F32Const(i) if f32::from_bits(i) != 0.0 => {
                Some(Self(F32Const(i), F32Const(0f32.to_bits())))
            }
            F64Const(i) if f64::from_bits(i) != 0.0 => {
                Some(Self(F64Const(i), F64Const(0f64.to_bits())))
            }
            _ => None,
        }
    }
}

pub struct CallRemoveVoidCall(Instruction, Instruction, usize);
impl InstructionReplacement for CallRemoveVoidCall {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "call_remove_void_call"
    }

    fn apply(&self, instructions: &mut Vec<Instruction>, instr_index: u32) {
        assert_eq!(instructions[instr_index as usize], *self.old_instruction());

        instructions[instr_index as usize] = self.new_instruction().clone();

        for _ in 0..self.2 {
            instructions.insert(instr_index as usize, Drop)
        }
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
    fn new(instr: &Instruction, ctx: &InstructionContext) -> Option<Self> {
        match *instr {
            Call(func_ref) => {
                for candidate in ctx.call_removal_candidates() {
                    if let CallRemovalCandidate::FuncReturningVoid { index, params } = candidate {
                        if *index == func_ref {
                            return Some(Self(instr.clone(), Nop, *params));
                        }
                    }
                }

                None
            }
            _ => None,
        }
    }
}

pub struct CallRemoveScalarCall(Instruction, Instruction, usize);
impl InstructionReplacement for CallRemoveScalarCall {
    common_functions!();

    fn name() -> &'static str
    where
        Self: Sized + 'static,
    {
        "call_remove_scalar_call"
    }

    fn apply(&self, instructions: &mut Vec<Instruction>, instr_index: u32) {
        assert_eq!(instructions[instr_index as usize], *self.old_instruction());

        instructions[instr_index as usize] = self.new_instruction().clone();

        for _ in 0..self.2 {
            instructions.insert(instr_index as usize, Drop)
        }
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
    fn new(instr: &Instruction, ctx: &InstructionContext) -> Option<Self> {
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
                                Datatype::I32 => I32Const(42),
                                Datatype::I64 => I64Const(42),
                                Datatype::F32 => F32Const(42f32.to_bits()),
                                Datatype::F64 => F64Const(42f64.to_bits()),
                            };

                            return Some(Self(instr.clone(), replacement, *params));
                        }
                    }
                }

                None
            }
            _ => None,
        }
    }
}
