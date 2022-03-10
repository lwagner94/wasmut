pub mod ops;

use anyhow::Result;
use dyn_clone::DynClone;
use ops::*;
#[allow(unused_imports)]
use parity_wasm::elements::Instruction::{self, *};
use parity_wasm::elements::{BlockType, ValueType};

use crate::wasmmodule::CallRemovalCandidate;

pub trait InstructionReplacement: Send + Sync + std::fmt::Debug + DynClone {
    fn old_instruction(&self) -> &Instruction;
    fn new_instruction(&self) -> &Instruction;

    fn replacement(&self) -> Vec<Instruction>;

    fn result(&self) -> BlockType;

    fn parameters(&self) -> &[ValueType];

    fn description(&self) -> String;

    fn apply(&self, instructions: &mut Vec<Instruction>, instr_index: u64) {
        assert_eq!(instructions[instr_index as usize], *self.old_instruction());

        instructions.remove(instr_index as usize);

        for replacement_instr in self.replacement().iter().rev() {
            instructions.insert(instr_index as usize, replacement_instr.clone());
        }
    }

    fn name() -> &'static str
    where
        Self: Sized + 'static;

    fn factory() -> fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>
    where
        Self: Sized + Send + Sync + 'static;
}

dyn_clone::clone_trait_object!(InstructionReplacement);

#[derive(Default)]
pub struct InstructionContext {
    call_removal_candidates: Vec<CallRemovalCandidate>,
}

impl InstructionContext {
    pub fn new(call_removal_candidates: Vec<CallRemovalCandidate>) -> Self {
        Self {
            call_removal_candidates,
        }
    }

    fn call_removal_candidates(&self) -> &[CallRemovalCandidate] {
        &self.call_removal_candidates
    }
}

type FactoryFunction =
    fn(&Instruction, &InstructionContext) -> Option<Box<dyn InstructionReplacement>>;

#[derive(Default)]
pub struct OperatorRegistry {
    operators: Vec<FactoryFunction>,
    enabled_operator_names: Vec<String>,
    disabled_operator_names: Vec<String>,
}

macro_rules! register_operator {
    ($operator:ident, $v:ident, $regex_set:ident) => {
        if $regex_set.is_match(&$operator::name()) {
            $v.operators.push($operator::factory());
            $v.enabled_operator_names
                .push(String::from($operator::name()))
        } else {
            $v.disabled_operator_names
                .push(String::from($operator::name()))
        }
    };
}

impl OperatorRegistry {
    pub fn new<S: AsRef<str>>(enabled_ops: &[S]) -> Result<Self> {
        let mut registry: OperatorRegistry = Default::default();

        let regex_set = regex::RegexSet::new(enabled_ops).unwrap();

        register_operator!(BinaryOperatorSubToAdd, registry, regex_set);
        register_operator!(BinaryOperatorAddToSub, registry, regex_set);

        register_operator!(BinaryOperatorMulToDivS, registry, regex_set);
        register_operator!(BinaryOperatorMulToDivU, registry, regex_set);
        register_operator!(BinaryOperatorDivXToMul, registry, regex_set);

        register_operator!(BinaryOperatorShlToShrS, registry, regex_set);
        register_operator!(BinaryOperatorShlToShrU, registry, regex_set);
        register_operator!(BinaryOperatorShrXToShl, registry, regex_set);

        register_operator!(BinaryOperatorRemToDiv, registry, regex_set);
        register_operator!(BinaryOperatorDivToRem, registry, regex_set);

        register_operator!(BinaryOperatorAndToOr, registry, regex_set);
        register_operator!(BinaryOperatorOrToAnd, registry, regex_set);

        register_operator!(BinaryOperatorXorToOr, registry, regex_set);
        register_operator!(BinaryOperatorOrToXor, registry, regex_set);

        register_operator!(BinaryOperatorRotlToRotr, registry, regex_set);
        register_operator!(BinaryOperatorRotrToRotl, registry, regex_set);

        register_operator!(UnaryOperatorNegToNop, registry, regex_set);

        register_operator!(RelationalOperatorEqToNe, registry, regex_set);
        register_operator!(RelationalOperatorNeToEq, registry, regex_set);

        register_operator!(RelationalOperatorLeToGt, registry, regex_set);
        register_operator!(RelationalOperatorLeToLt, registry, regex_set);

        register_operator!(RelationalOperatorLtToGe, registry, regex_set);
        register_operator!(RelationalOperatorLtToLe, registry, regex_set);

        register_operator!(RelationalOperatorGeToGt, registry, regex_set);
        register_operator!(RelationalOperatorGeToLt, registry, regex_set);

        register_operator!(RelationalOperatorGtToGe, registry, regex_set);
        register_operator!(RelationalOperatorGtToLe, registry, regex_set);

        register_operator!(ConstReplaceZero, registry, regex_set);
        register_operator!(ConstReplaceNonZero, registry, regex_set);
        register_operator!(CallRemoveVoidCall, registry, regex_set);
        register_operator!(CallRemoveScalarCall, registry, regex_set);

        Ok(registry)
    }

    pub fn mutants_for_instruction(
        &self,
        instruction: &Instruction,
        context: &InstructionContext,
    ) -> Vec<Box<dyn InstructionReplacement>> {
        let mut results = Vec::new();
        for op in &self.operators {
            if let Some(operator_instance) = op(instruction, context) {
                results.push(operator_instance);
            }
        }

        results
    }

    #[allow(dead_code)]
    fn number_of_operators(&self) -> usize {
        self.operators.len()
    }

    pub fn enabled_operators(&self) -> &[String] {
        &self.enabled_operator_names
    }

    pub fn disabled_operators(&self) -> &[String] {
        &self.disabled_operator_names
    }
}

#[cfg(test)]
mod tests {
    use crate::wasmmodule::Datatype;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
    use concat_idents::concat_idents;
    use parity_wasm::elements::ValueType;

    macro_rules! generate_test {
        ($operator:ident, $original:ident, $replacement:ident, $block_type:expr) => {
            concat_idents!(test_name = $operator, _enabled_, $original, _, $replacement {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let enabled_operator = stringify!($operator);
                    let registry = OperatorRegistry::new([enabled_operator].as_slice()).unwrap();
                    let context = Default::default();

                    let ops = registry.mutants_for_instruction(&$original, &context);
                    assert!(ops.len() > 0);

                    let mut found = false;

                    for op in &ops {
                        let mut instr = vec![$original];
                        op.apply(&mut instr, 0);
                        if instr[0] == $replacement {
                            found = true;
                        }

                        assert_eq!(op.result(), $block_type);
                        let description = op.description();
                        assert!(description.len() > 0);
                        assert!(description.contains(stringify!($operator)));
                    }
                    assert!(found);
                }
            });

            concat_idents!(test_name = $operator, _disabled_, $original, _, $replacement {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let enabled_ops: &[String] = [].as_slice();
                    let registry = OperatorRegistry::new(enabled_ops).unwrap();
                    let instr = $original;
                    let context = Default::default();
                    let ops = registry.mutants_for_instruction(&instr, &context);
                    assert_eq!(ops.len(), 0);
                }
            });
        };
     }

    macro_rules! generate_const_test {
        ($operator:ident, $suffix:ident, $original:expr, $replacement:expr, $block_type:expr) => {
            concat_idents!(test_name = $operator, _, $suffix,  _enabled {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let enabled_operator = stringify!($operator);
                    let registry = OperatorRegistry::new([enabled_operator].as_slice()).unwrap();
                    let context = Default::default();
                    let ops = registry.mutants_for_instruction(&$original, &context);
                    assert_eq!(ops.len(), 1);

                    let mut instr = vec![$original];
                    ops[0].apply(&mut instr, 0);
                    assert_eq!(instr[0], $replacement);
                    assert_eq!(ops[0].result(), $block_type);

                    let description = ops[0].description();
                    assert!(description.len() > 0);
                    assert!(description.contains(stringify!($operator)));
                }
            });

            concat_idents!(test_name = $operator, _, $suffix,  _disabled {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let registry = OperatorRegistry::new([].as_slice() as &[&str]).unwrap();
                    let instr = $original;
                    let context = Default::default();
                    let ops = registry.mutants_for_instruction(&instr, &context);
                    assert_eq!(ops.len(), 0);
                }
            });
        };
     }

    generate_test!(
        binop_sub_to_add,
        I32Sub,
        I32Add,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_sub_to_add,
        I64Sub,
        I64Add,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_sub_to_add,
        F32Sub,
        F32Add,
        BlockType::Value(ValueType::F32)
    );
    generate_test!(
        binop_sub_to_add,
        F64Sub,
        F64Add,
        BlockType::Value(ValueType::F64)
    );

    generate_test!(
        binop_add_to_sub,
        I32Add,
        I32Sub,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_add_to_sub,
        I64Add,
        I64Sub,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_add_to_sub,
        F32Add,
        F32Sub,
        BlockType::Value(ValueType::F32)
    );
    generate_test!(
        binop_add_to_sub,
        F64Add,
        F64Sub,
        BlockType::Value(ValueType::F64)
    );

    generate_test!(
        binop_mul_to_div,
        I32Mul,
        I32DivU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_mul_to_div,
        I32Mul,
        I32DivS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_mul_to_div,
        I64Mul,
        I64DivU,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_mul_to_div,
        I64Mul,
        I64DivS,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_mul_to_div,
        F32Mul,
        F32Div,
        BlockType::Value(ValueType::F32)
    );
    generate_test!(
        binop_mul_to_div,
        F64Mul,
        F64Div,
        BlockType::Value(ValueType::F64)
    );

    generate_test!(
        binop_div_to_mul,
        I32DivU,
        I32Mul,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_div_to_mul,
        I32DivS,
        I32Mul,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_div_to_mul,
        I64DivU,
        I64Mul,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_div_to_mul,
        I64DivS,
        I64Mul,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_div_to_mul,
        F32Div,
        F32Mul,
        BlockType::Value(ValueType::F32)
    );
    generate_test!(
        binop_div_to_mul,
        F64Div,
        F64Mul,
        BlockType::Value(ValueType::F64)
    );

    generate_test!(
        binop_shl_to_shr,
        I32Shl,
        I32ShrU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_shl_to_shr,
        I32Shl,
        I32ShrS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_shl_to_shr,
        I64Shl,
        I64ShrU,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_shl_to_shr,
        I64Shl,
        I64ShrS,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_rem_to_div,
        I32RemU,
        I32DivU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_rem_to_div,
        I32RemS,
        I32DivS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_rem_to_div,
        I64RemU,
        I64DivU,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_rem_to_div,
        I64RemS,
        I64DivS,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_div_to_rem,
        I32DivU,
        I32RemU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_div_to_rem,
        I32DivS,
        I32RemS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_div_to_rem,
        I64DivU,
        I64RemU,
        BlockType::Value(ValueType::I64)
    );
    generate_test!(
        binop_div_to_rem,
        I64DivS,
        I64RemS,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_and_to_or,
        I32And,
        I32Or,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_and_to_or,
        I64And,
        I64Or,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_or_to_and,
        I32Or,
        I32And,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_or_to_and,
        I64Or,
        I64And,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_xor_to_or,
        I32Xor,
        I32Or,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_xor_to_or,
        I64Xor,
        I64Or,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_or_to_xor,
        I32Or,
        I32Xor,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_or_to_xor,
        I64Or,
        I64Xor,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_rotl_to_rotr,
        I32Rotl,
        I32Rotr,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_rotl_to_rotr,
        I64Rotl,
        I64Rotr,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        binop_rotr_to_rotl,
        I32Rotr,
        I32Rotl,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        binop_rotr_to_rotl,
        I64Rotr,
        I64Rotl,
        BlockType::Value(ValueType::I64)
    );

    generate_test!(
        unop_neg_to_nop,
        F32Neg,
        Nop,
        BlockType::Value(ValueType::F32)
    );
    generate_test!(
        unop_neg_to_nop,
        F64Neg,
        Nop,
        BlockType::Value(ValueType::F64)
    );

    generate_test!(
        relop_eq_to_ne,
        I32Eq,
        I32Ne,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_eq_to_ne,
        I64Eq,
        I64Ne,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_eq_to_ne,
        F32Eq,
        F32Ne,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_eq_to_ne,
        F64Eq,
        F64Ne,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_ne_to_eq,
        I32Ne,
        I32Eq,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ne_to_eq,
        I64Ne,
        I64Eq,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ne_to_eq,
        F32Ne,
        F32Eq,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ne_to_eq,
        F64Ne,
        F64Eq,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_le_to_gt,
        I32LeU,
        I32GtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_gt,
        I32LeS,
        I32GtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_gt,
        I64LeU,
        I64GtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_gt,
        I64LeS,
        I64GtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_gt,
        F32Le,
        F32Gt,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_gt,
        F64Le,
        F64Gt,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_le_to_lt,
        I32LeU,
        I32LtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_lt,
        I32LeS,
        I32LtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_lt,
        I64LeU,
        I64LtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_lt,
        I64LeS,
        I64LtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_lt,
        F32Le,
        F32Lt,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_le_to_lt,
        F64Le,
        F64Lt,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_lt_to_ge,
        I32LtU,
        I32GeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_ge,
        I32LtS,
        I32GeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_ge,
        I64LtU,
        I64GeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_ge,
        I64LtS,
        I64GeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_ge,
        F32Lt,
        F32Ge,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_ge,
        F64Lt,
        F64Ge,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_lt_to_le,
        I32LtU,
        I32LeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_le,
        I32LtS,
        I32LeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_le,
        I64LtU,
        I64LeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_le,
        I64LtS,
        I64LeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_le,
        F32Lt,
        F32Le,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_lt_to_le,
        F64Lt,
        F64Le,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_ge_to_gt,
        I32GeU,
        I32GtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_gt,
        I32GeS,
        I32GtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_gt,
        I64GeU,
        I64GtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_gt,
        I64GeS,
        I64GtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_gt,
        F32Ge,
        F32Gt,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_gt,
        F64Ge,
        F64Gt,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_ge_to_lt,
        I32GeU,
        I32LtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_lt,
        I32GeS,
        I32LtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_lt,
        I64GeU,
        I64LtU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_lt,
        I64GeS,
        I64LtS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_lt,
        F32Ge,
        F32Lt,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_ge_to_lt,
        F64Ge,
        F64Lt,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_gt_to_ge,
        I32GtU,
        I32GeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_ge,
        I32GtS,
        I32GeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_ge,
        I64GtU,
        I64GeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_ge,
        I64GtS,
        I64GeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_ge,
        F32Gt,
        F32Ge,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_ge,
        F64Gt,
        F64Ge,
        BlockType::Value(ValueType::I32)
    );

    generate_test!(
        relop_gt_to_le,
        I32GtU,
        I32LeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_le,
        I32GtS,
        I32LeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_le,
        I64GtU,
        I64LeU,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_le,
        I64GtS,
        I64LeS,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_le,
        F32Gt,
        F32Le,
        BlockType::Value(ValueType::I32)
    );
    generate_test!(
        relop_gt_to_le,
        F64Gt,
        F64Le,
        BlockType::Value(ValueType::I32)
    );

    generate_const_test!(
        const_replace_zero,
        i32,
        I32Const(0),
        I32Const(42),
        BlockType::Value(ValueType::I32)
    );
    generate_const_test!(
        const_replace_zero,
        i64,
        I64Const(0),
        I64Const(42),
        BlockType::Value(ValueType::I64)
    );
    generate_const_test!(
        const_replace_zero,
        f32,
        F32Const(0f32.to_bits()),
        F32Const(42f32.to_bits()),
        BlockType::Value(ValueType::F32)
    );
    generate_const_test!(
        const_replace_zero,
        f64,
        F64Const(0f64.to_bits()),
        F64Const(42f64.to_bits()),
        BlockType::Value(ValueType::F64)
    );

    generate_const_test!(
        const_replace_nonzero,
        i32,
        I32Const(1337),
        I32Const(0),
        BlockType::Value(ValueType::I32)
    );
    generate_const_test!(
        const_replace_nonzero,
        i64,
        I64Const(1337),
        I64Const(0),
        BlockType::Value(ValueType::I64)
    );
    generate_const_test!(
        const_replace_nonzero,
        f32,
        F32Const(1337f32.to_bits()),
        F32Const(0f32.to_bits()),
        BlockType::Value(ValueType::F32)
    );
    generate_const_test!(
        const_replace_nonzero,
        f64,
        F64Const(1337f64.to_bits()),
        F64Const(0f64.to_bits()),
        BlockType::Value(ValueType::F64)
    );

    #[test]
    fn call_remove_void_call_enabled() {
        let registry = OperatorRegistry::new(["call_remove_void_call"].as_slice()).unwrap();
        let context = InstructionContext::new(vec![CallRemovalCandidate::FuncReturningVoid {
            index: 0,
            params: [ValueType::I32, ValueType::I32].into(),
        }]);

        let ops = registry.mutants_for_instruction(&Call(0), &context);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].result(), BlockType::NoResult);

        let mut instructions = vec![I32Const(10), I32Const(12), Call(0), I32Const(13), Call(1)];

        ops[0].apply(&mut instructions, 2);

        let expected = vec![
            I32Const(10),
            I32Const(12),
            Drop,
            Drop,
            Nop,
            I32Const(13),
            Call(1),
        ];

        assert_eq!(instructions, expected);
    }

    #[test]
    fn call_remove_void_call_disabled() {
        let registry = OperatorRegistry::new([].as_slice() as &[&str]).unwrap();
        let context = InstructionContext::new(vec![CallRemovalCandidate::FuncReturningVoid {
            index: 0,
            params: [ValueType::I32, ValueType::I32].into(),
        }]);
        let ops = registry.mutants_for_instruction(&Call(0), &context);
        assert_eq!(ops.len(), 0);
    }

    macro_rules! generate_remove_scalar_call_test {
        ($datatype:ident, $replacement:expr) => {
            concat_idents!(test_name = call_remove_scalar_call_, $datatype, _enabled {
                #[allow(non_snake_case)]
                #[test]

                fn test_name() {
                    let registry = OperatorRegistry::new(["call_remove_scalar_call"].as_slice()).unwrap();
                    let context = InstructionContext::new(vec![CallRemovalCandidate::FuncReturningScalar {
                        index: 0,
                        params: [ValueType::I32, ValueType::I32].into(),
                        return_type: Datatype::$datatype,
                    }]);

                    let ops = registry.mutants_for_instruction(&Call(0), &context);
                    assert_eq!(ops.len(), 1);
                    assert_eq!(ops[0].result(), BlockType::Value(ValueType::$datatype));

                    let mut instructions = vec![I32Const(10), I32Const(12), Call(0), I32Const(13), Call(1)];

                    ops[0].apply(&mut instructions, 2);

                    let expected = vec![
                        I32Const(10),
                        I32Const(12),
                        Drop,
                        Drop,
                        $replacement,
                        I32Const(13),
                        Call(1),
                    ];

                    assert_eq!(instructions, expected);
                }
            });

            concat_idents!(test_name = call_remove_scalar_call_, $datatype, _disabled {
                #[allow(non_snake_case)]
                #[test]
                fn test_name() {
                    let registry = OperatorRegistry::new([].as_slice() as &[&str]).unwrap();
                    let context = InstructionContext::new(vec![CallRemovalCandidate::FuncReturningScalar {
                        index: 0,
                        params: [ValueType::I32, ValueType::I32].into(),
                        return_type: Datatype::$datatype,
                    }]);
                    let ops = registry.mutants_for_instruction(&Call(0), &context);
                    assert_eq!(ops.len(), 0);
                }
            });
        };
     }

    generate_remove_scalar_call_test!(I32, I32Const(42));
    generate_remove_scalar_call_test!(I64, I64Const(42));
    generate_remove_scalar_call_test!(F32, F32Const(42f32.to_bits()));
    generate_remove_scalar_call_test!(F64, F64Const(42f64.to_bits()));

    #[test]
    fn registry_correct_number_of_ops() {
        assert_eq!(
            OperatorRegistry::new(&["binop_mul_to_div"])
                .unwrap()
                .number_of_operators(),
            2
        );
        assert_eq!(
            OperatorRegistry::new(&["binop_shl_to_shr"])
                .unwrap()
                .number_of_operators(),
            2
        );
        assert_eq!(
            OperatorRegistry::new(&["binop_"])
                .unwrap()
                .number_of_operators(),
            16
        );
        assert_eq!(
            OperatorRegistry::new(&["const_replace_"])
                .unwrap()
                .number_of_operators(),
            2
        );
        assert_eq!(
            OperatorRegistry::new(&[""]).unwrap().number_of_operators(),
            31
        );
    }
}
