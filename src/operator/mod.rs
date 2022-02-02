#[allow(unused_imports)]
use parity_wasm::elements::Instruction::{self, *};

pub mod ops;

use ops::*;

pub trait InstructionReplacement: Send + Sync {
    fn new(instr: Instruction) -> Option<Self>
    where
        Self: Sized;
    fn old_instruction(&self) -> &Instruction;
    fn new_instruction(&self) -> &Instruction;

    fn description(&self) -> String;
    fn apply(&self, instr_to_be_mutated: &mut Instruction) {
        assert_eq!(instr_to_be_mutated, self.old_instruction());

        *instr_to_be_mutated = self.new_instruction().clone();
    }

    fn name() -> &'static str
    where
        Self: Sized + 'static;
}

pub struct InstructionContext {}

pub trait ContextAwareOperator: Send + Sync {
    fn new(instr: Instruction, ctx: InstructionContext) -> Option<Self>
    where
        Self: Sized;
    fn old_instruction(&self) -> &Instruction;
    fn new_instruction(&self) -> &Instruction;

    fn description(&self) -> String;
    fn apply(&self, instr_to_be_mutated: &mut Instruction) {
        assert_eq!(instr_to_be_mutated, self.old_instruction());

        *instr_to_be_mutated = self.new_instruction().clone();
    }

    fn name() -> &'static str
    where
        Self: Sized + 'static;
}

type FactoryFunction = fn(Instruction) -> Option<Box<dyn InstructionReplacement>>;

pub struct OperatorRegistry {
    operators: Vec<FactoryFunction>,
}

macro_rules! register_operator {
    ($operator:ident, $v:ident, $enabled_ops:ident) => {
        if $enabled_ops.contains(&$operator::name()) {
            $v.push($operator::factory());
        }
    };
}

impl OperatorRegistry {
    pub fn new(enabled_ops: &[&str]) -> Self {
        let mut ops = Vec::new();

        register_operator!(BinaryOperatorSubToAdd, ops, enabled_ops);
        register_operator!(BinaryOperatorAddToSub, ops, enabled_ops);

        register_operator!(BinaryOperatorMulToDivS, ops, enabled_ops);
        register_operator!(BinaryOperatorMulToDivU, ops, enabled_ops);
        register_operator!(BinaryOperatorDivXToMul, ops, enabled_ops);

        register_operator!(BinaryOperatorShlToShrS, ops, enabled_ops);
        register_operator!(BinaryOperatorShlToShrU, ops, enabled_ops);
        register_operator!(BinaryOperatorShrXToShl, ops, enabled_ops);

        register_operator!(BinaryOperatorRemToDiv, ops, enabled_ops);
        register_operator!(BinaryOperatorDivToRem, ops, enabled_ops);

        register_operator!(BinaryOperatorAndToOr, ops, enabled_ops);
        register_operator!(BinaryOperatorOrToAnd, ops, enabled_ops);

        register_operator!(BinaryOperatorXorToOr, ops, enabled_ops);
        register_operator!(BinaryOperatorOrToXor, ops, enabled_ops);

        register_operator!(BinaryOperatorRotlToRotr, ops, enabled_ops);
        register_operator!(BinaryOperatorRotrToRotl, ops, enabled_ops);

        register_operator!(UnaryOperatorNegToNop, ops, enabled_ops);

        register_operator!(RelationalOperatorEqToNe, ops, enabled_ops);
        register_operator!(RelationalOperatorNeToEq, ops, enabled_ops);

        register_operator!(RelationalOperatorLeToGt, ops, enabled_ops);
        register_operator!(RelationalOperatorLeToLt, ops, enabled_ops);

        register_operator!(RelationalOperatorLtToGe, ops, enabled_ops);
        register_operator!(RelationalOperatorLtToLe, ops, enabled_ops);

        register_operator!(RelationalOperatorGeToGt, ops, enabled_ops);
        register_operator!(RelationalOperatorGeToLt, ops, enabled_ops);

        register_operator!(RelationalOperatorGtToGe, ops, enabled_ops);
        register_operator!(RelationalOperatorGtToLe, ops, enabled_ops);

        register_operator!(ConstReplaceZero, ops, enabled_ops);
        register_operator!(ConstReplaceNonZero, ops, enabled_ops);

        Self { operators: ops }
    }

    pub fn from_instruction(
        &self,
        instruction: &Instruction,
    ) -> Vec<Box<dyn InstructionReplacement>> {
        let mut results = Vec::new();
        for op in &self.operators {
            // TODO: Does it make sense to have clone here?
            if let Some(operator_instance) = op(instruction.clone()) {
                results.push(operator_instance);
            }
        }

        results
    }

    pub fn asdf(&self) -> bool {
        true
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        let ops = vec![
            BinaryOperatorSubToAdd::factory(),
            BinaryOperatorAddToSub::factory(),
            BinaryOperatorMulToDivS::factory(),
            BinaryOperatorMulToDivU::factory(),
            BinaryOperatorDivXToMul::factory(),
            BinaryOperatorShlToShrS::factory(),
            BinaryOperatorShlToShrU::factory(),
            BinaryOperatorShrXToShl::factory(),
            BinaryOperatorRemToDiv::factory(),
            BinaryOperatorDivToRem::factory(),
            BinaryOperatorAndToOr::factory(),
            BinaryOperatorOrToAnd::factory(),
            BinaryOperatorXorToOr::factory(),
            BinaryOperatorOrToXor::factory(),
            BinaryOperatorRotlToRotr::factory(),
            BinaryOperatorRotrToRotl::factory(),
            UnaryOperatorNegToNop::factory(),
            RelationalOperatorEqToNe::factory(),
            RelationalOperatorNeToEq::factory(),
            RelationalOperatorLeToGt::factory(),
            RelationalOperatorLeToLt::factory(),
            RelationalOperatorLtToGe::factory(),
            RelationalOperatorLtToLe::factory(),
            RelationalOperatorGeToGt::factory(),
            RelationalOperatorGeToLt::factory(),
            RelationalOperatorGtToGe::factory(),
            RelationalOperatorGtToLe::factory(),
            ConstReplaceZero::factory(),
            ConstReplaceNonZero::factory(),
        ];
        Self { operators: ops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concat_idents::concat_idents;

    macro_rules! generate_test {
        ($operator:ident, $original:ident, $replacement:ident) => {
            concat_idents!(test_name = $operator, _enabled_, $original, _, $replacement {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let enabled_operator = stringify!($operator);
                    let registry = OperatorRegistry::new([enabled_operator].as_slice());

                    let ops = registry.from_instruction(&$original);
                    assert!(ops.len() > 0);

                    let mut found = false;

                    for op in &ops {
                        let mut instr = $original;
                        op.apply(&mut instr);
                        if instr == $replacement {
                            found = true;
                        }
                    }
                    assert!(found);
                }
            });

            concat_idents!(test_name = $operator, _disabled_, $original, _, $replacement {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let registry = OperatorRegistry::new([].as_slice());
                    let instr = $original;
                    let ops = registry.from_instruction(&instr);
                    assert_eq!(ops.len(), 0);
                }
            });
        };
     }

    macro_rules! generate_const_test {
        ($operator:ident, $suffix:ident, $original:expr, $replacement:expr) => {
            concat_idents!(test_name = $operator, _, $suffix,  _enabled {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let enabled_operator = stringify!($operator);
                    let registry = OperatorRegistry::new([enabled_operator].as_slice());

                    let ops = registry.from_instruction(&$original);
                    assert_eq!(ops.len(), 1);

                    let mut instr = $original;
                    ops[0].apply(&mut instr);
                    assert_eq!(instr, $replacement);
                }
            });

            concat_idents!(test_name = $operator, _, $suffix,  _disabled {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let registry = OperatorRegistry::new([].as_slice());
                    let instr = $original;
                    let ops = registry.from_instruction(&instr);
                    assert_eq!(ops.len(), 0);
                }
            });
        };
     }

    generate_test!(binop_sub_to_add, I32Sub, I32Add);
    generate_test!(binop_sub_to_add, I64Sub, I64Add);
    generate_test!(binop_sub_to_add, F32Sub, F32Add);
    generate_test!(binop_sub_to_add, F64Sub, F64Add);

    generate_test!(binop_add_to_sub, I32Add, I32Sub);
    generate_test!(binop_add_to_sub, I64Add, I64Sub);
    generate_test!(binop_add_to_sub, F32Add, F32Sub);
    generate_test!(binop_add_to_sub, F64Add, F64Sub);

    generate_test!(binop_mul_to_div, I32Mul, I32DivU);
    generate_test!(binop_mul_to_div, I32Mul, I32DivS);
    generate_test!(binop_mul_to_div, I64Mul, I64DivU);
    generate_test!(binop_mul_to_div, I64Mul, I64DivS);
    generate_test!(binop_mul_to_div, F32Mul, F32Div);
    generate_test!(binop_mul_to_div, F64Mul, F64Div);

    generate_test!(binop_div_to_mul, I32DivU, I32Mul);
    generate_test!(binop_div_to_mul, I32DivS, I32Mul);
    generate_test!(binop_div_to_mul, I64DivU, I64Mul);
    generate_test!(binop_div_to_mul, I64DivS, I64Mul);
    generate_test!(binop_div_to_mul, F32Div, F32Mul);
    generate_test!(binop_div_to_mul, F64Div, F64Mul);

    generate_test!(binop_shl_to_shr, I32Shl, I32ShrU);
    generate_test!(binop_shl_to_shr, I32Shl, I32ShrS);
    generate_test!(binop_shl_to_shr, I64Shl, I64ShrU);
    generate_test!(binop_shl_to_shr, I64Shl, I64ShrS);

    generate_test!(binop_rem_to_div, I32RemU, I32DivU);
    generate_test!(binop_rem_to_div, I32RemS, I32DivS);
    generate_test!(binop_rem_to_div, I64RemU, I64DivU);
    generate_test!(binop_rem_to_div, I64RemS, I64DivS);

    generate_test!(binop_div_to_rem, I32DivU, I32RemU);
    generate_test!(binop_div_to_rem, I32DivS, I32RemS);
    generate_test!(binop_div_to_rem, I64DivU, I64RemU);
    generate_test!(binop_div_to_rem, I64DivS, I64RemS);

    generate_test!(binop_and_to_or, I32And, I32Or);
    generate_test!(binop_and_to_or, I64And, I64Or);

    generate_test!(binop_or_to_and, I32Or, I32And);
    generate_test!(binop_or_to_and, I64Or, I64And);

    generate_test!(binop_xor_to_or, I32Xor, I32Or);
    generate_test!(binop_xor_to_or, I64Xor, I64Or);

    generate_test!(binop_or_to_xor, I32Or, I32Xor);
    generate_test!(binop_or_to_xor, I64Or, I64Xor);

    generate_test!(binop_rotl_to_rotr, I32Rotl, I32Rotr);
    generate_test!(binop_rotl_to_rotr, I64Rotl, I64Rotr);

    generate_test!(binop_rotr_to_rotl, I32Rotr, I32Rotl);
    generate_test!(binop_rotr_to_rotl, I64Rotr, I64Rotl);

    generate_test!(unop_neg_to_nop, F32Neg, Nop);
    generate_test!(unop_neg_to_nop, F64Neg, Nop);

    generate_test!(relop_eq_to_ne, I32Eq, I32Ne);
    generate_test!(relop_eq_to_ne, I64Eq, I64Ne);
    generate_test!(relop_eq_to_ne, F32Eq, F32Ne);
    generate_test!(relop_eq_to_ne, F64Eq, F64Ne);

    generate_test!(relop_ne_to_eq, I32Ne, I32Eq);
    generate_test!(relop_ne_to_eq, I64Ne, I64Eq);
    generate_test!(relop_ne_to_eq, F32Ne, F32Eq);
    generate_test!(relop_ne_to_eq, F64Ne, F64Eq);

    generate_test!(relop_le_to_gt, I32LeU, I32GtU);
    generate_test!(relop_le_to_gt, I32LeS, I32GtS);
    generate_test!(relop_le_to_gt, I64LeU, I64GtU);
    generate_test!(relop_le_to_gt, I64LeS, I64GtS);
    generate_test!(relop_le_to_gt, F32Le, F32Gt);
    generate_test!(relop_le_to_gt, F64Le, F64Gt);

    generate_test!(relop_le_to_lt, I32LeU, I32LtU);
    generate_test!(relop_le_to_lt, I32LeS, I32LtS);
    generate_test!(relop_le_to_lt, I64LeU, I64LtU);
    generate_test!(relop_le_to_lt, I64LeS, I64LtS);
    generate_test!(relop_le_to_lt, F32Le, F32Lt);
    generate_test!(relop_le_to_lt, F64Le, F64Lt);

    generate_test!(relop_lt_to_ge, I32LtU, I32GeU);
    generate_test!(relop_lt_to_ge, I32LtS, I32GeS);
    generate_test!(relop_lt_to_ge, I64LtU, I64GeU);
    generate_test!(relop_lt_to_ge, I64LtS, I64GeS);
    generate_test!(relop_lt_to_ge, F32Lt, F32Ge);
    generate_test!(relop_lt_to_ge, F64Lt, F64Ge);

    generate_test!(relop_lt_to_le, I32LtU, I32LeU);
    generate_test!(relop_lt_to_le, I32LtS, I32LeS);
    generate_test!(relop_lt_to_le, I64LtU, I64LeU);
    generate_test!(relop_lt_to_le, I64LtS, I64LeS);
    generate_test!(relop_lt_to_le, F32Lt, F32Le);
    generate_test!(relop_lt_to_le, F64Lt, F64Le);

    generate_test!(relop_ge_to_gt, I32GeU, I32GtU);
    generate_test!(relop_ge_to_gt, I32GeS, I32GtS);
    generate_test!(relop_ge_to_gt, I64GeU, I64GtU);
    generate_test!(relop_ge_to_gt, I64GeS, I64GtS);
    generate_test!(relop_ge_to_gt, F32Ge, F32Gt);
    generate_test!(relop_ge_to_gt, F64Ge, F64Gt);

    generate_test!(relop_ge_to_lt, I32GeU, I32LtU);
    generate_test!(relop_ge_to_lt, I32GeS, I32LtS);
    generate_test!(relop_ge_to_lt, I64GeU, I64LtU);
    generate_test!(relop_ge_to_lt, I64GeS, I64LtS);
    generate_test!(relop_ge_to_lt, F32Ge, F32Lt);
    generate_test!(relop_ge_to_lt, F64Ge, F64Lt);

    generate_test!(relop_gt_to_ge, I32GtU, I32GeU);
    generate_test!(relop_gt_to_ge, I32GtS, I32GeS);
    generate_test!(relop_gt_to_ge, I64GtU, I64GeU);
    generate_test!(relop_gt_to_ge, I64GtS, I64GeS);
    generate_test!(relop_gt_to_ge, F32Gt, F32Ge);
    generate_test!(relop_gt_to_ge, F64Gt, F64Ge);

    generate_test!(relop_gt_to_le, I32GtU, I32LeU);
    generate_test!(relop_gt_to_le, I32GtS, I32LeS);
    generate_test!(relop_gt_to_le, I64GtU, I64LeU);
    generate_test!(relop_gt_to_le, I64GtS, I64LeS);
    generate_test!(relop_gt_to_le, F32Gt, F32Le);
    generate_test!(relop_gt_to_le, F64Gt, F64Le);

    generate_const_test!(const_replace_zero, i32, I32Const(0), I32Const(42));
    generate_const_test!(const_replace_zero, i64, I64Const(0), I64Const(42));
    generate_const_test!(
        const_replace_zero,
        f32,
        F32Const(0f32.to_bits()),
        F32Const(42f32.to_bits())
    );
    generate_const_test!(
        const_replace_zero,
        f64,
        F64Const(0f64.to_bits()),
        F64Const(42f64.to_bits())
    );

    generate_const_test!(const_replace_nonzero, i32, I32Const(1337), I32Const(0));
    generate_const_test!(const_replace_nonzero, i64, I64Const(1337), I64Const(0));
    generate_const_test!(
        const_replace_nonzero,
        f32,
        F32Const(1337f32.to_bits()),
        F32Const(0f32.to_bits())
    );
    generate_const_test!(
        const_replace_nonzero,
        f64,
        F64Const(1337f64.to_bits()),
        F64Const(0f64.to_bits())
    );
}
