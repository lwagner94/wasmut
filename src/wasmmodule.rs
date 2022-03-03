use std::{borrow::Cow, collections::HashSet};

use crate::{
    addressresolver::AddressResolver,
    mutation::{Mutation, MutationLocation},
};
use parity_wasm::elements::{
    External, FunctionType, GlobalEntry, GlobalType, ImportEntry, InitExpr, Instruction, Internal,
    Module, TableElementType, Type, ValueType,
};

use anyhow::{Context, Result};

use rayon::prelude::*;

/// Callback type used by wasmmodule::instruction_walker
pub type CallbackType<'a, R> =
    &'a (dyn Fn(&Instruction, &InstructionWalkerLocation) -> Vec<R> + Send + Sync);

/// Code location passed to `CallbackType`, it represents where
/// we are when traversing the module.
pub struct InstructionWalkerLocation<'a> {
    pub file: Option<&'a str>,
    pub function: Option<&'a str>,
    pub function_index: u64,
    pub instruction_index: u64,
    pub instruction_offset: u64,
}

/// WebAssembly native datatypes
#[derive(Debug, PartialEq)]
pub enum Datatype {
    I32,
    I64,
    F32,
    F64,
}

impl From<ValueType> for Datatype {
    fn from(val: ValueType) -> Datatype {
        match val {
            ValueType::I32 => Datatype::I32,
            ValueType::I64 => Datatype::I64,
            ValueType::F32 => Datatype::F32,
            ValueType::F64 => Datatype::F64,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CallRemovalCandidate {
    /// Function does not return anything and has `params` parameters
    FuncReturningVoid { index: u32, params: Vec<ValueType> },

    /// Function returns scalar and has `params` parameters
    FuncReturningScalar {
        index: u32,
        params: Vec<ValueType>,
        return_type: Datatype,
    },
}

/// WasmModule represents a (parsed) WebAssembly module
#[derive(Clone)]
pub struct WasmModule<'a> {
    module: parity_wasm::elements::Module,
    path: Cow<'a, str>,
}

impl<'a> WasmModule<'a> {
    /// Construct a new `WasmModule` from a file path
    pub fn from_file(path: &str) -> Result<WasmModule> {
        // let p: Cow<'_, str> = Cow::Owned(path.to_string());

        let module: Module = parity_wasm::elements::deserialize_file(path)
            .context("Bytecode deserialization failed")?;

        if !module.has_names_section() {
            log::warn!("Module has no name section, make sure to enable the debug flag!");
        }

        Ok(WasmModule {
            module,
            path: path.into(),
        })
    }

    /// Traverse module, and call callback function for every instruction
    pub fn instruction_walker<R: Send>(&self, callback: CallbackType<R>) -> Result<Vec<R>> {
        let code_section = self
            .module
            .code_section()
            .context("Module has no code section")?;

        let bytes = std::fs::read(&self.path.as_ref())
            .with_context(|| format!("Could not read bytecode from {}", self.path))?;

        Ok(code_section
            .bodies()
            .par_iter()
            .enumerate()
            .map_init(
                || AddressResolver::new(&bytes),
                |resolver, (func_index, func_body)| {
                    let instructions = func_body.code().elements();
                    let offsets = func_body.code().offsets();

                    let mut results = Vec::new();

                    for ((instr_index, instruction), offset) in
                        instructions.iter().enumerate().zip(offsets)
                    {
                        // Relative offset of the instruction, in relation
                        // to the start of the code section
                        let code_offset = *offset - code_section.offset();

                        let location = resolver.lookup_address(code_offset);

                        results.extend(callback(
                            instruction,
                            &InstructionWalkerLocation {
                                // We need as_ref here because otherwise
                                // location is moved into the and_then function
                                file: location.as_ref().and_then(|l| l.file.as_deref()),
                                function: location.as_ref().and_then(|l| l.function.as_deref()),
                                function_index: func_index as u64,
                                instruction_index: instr_index as u64,
                                instruction_offset: code_offset,
                            },
                        ))
                    }

                    results
                },
            )
            .flatten_iter()
            .collect())
    }

    /// Apply a mutation
    fn mutate(&mut self, mutation_location: &MutationLocation, mutation_index: usize) {
        let instructions = self
            .module
            .code_section_mut()
            .expect("Module does not have a code section") // TODO: Error handling?
            .bodies_mut()
            .get_mut(mutation_location.function_number as usize)
            .expect("unexpected funtion index")
            .code_mut()
            .elements_mut();

        let mutation = mutation_location
            .mutations
            .get(mutation_index)
            .expect("Invalid mutation index");

        mutation
            .operator
            .apply(instructions, mutation_location.statement_number);
    }

    /// Apply all given mutations
    fn mutate_all(&mut self, locations: &[MutationLocation]) -> Result<()> {
        let type_index = self.find_or_insert_check_mutant_function_signature()?;
        let function_index =
            self.add_trace_function_import("__wasmut_check_mutant_id", type_index)?;

        // Increment all function-indices, since the
        // function section now contains the trace_function at index 0
        self.fix_call_instructions();
        self.fix_tables();
        self.fix_exports();

        if let Some(global_section) = self.module.global_section_mut() {
            for i in 0..10 {
                global_section.entries_mut().push(GlobalEntry::new(
                    GlobalType::new(ValueType::I64, true),
                    InitExpr::empty(),
                ));
            }
        }

        let bodies = self
            .module
            .code_section_mut()
            .context("Module does not have a code section")? // TODO: Error handling?
            .bodies_mut();

        let mut locations = locations.to_vec();

        locations.sort_by(|a, b| b.statement_number.cmp(&a.statement_number));

        for location in locations {
            let instructions = bodies
                .get_mut(location.function_number as usize)
                .context("unexpected funtion index")?
                .code_mut()
                .elements_mut();

            let tail = instructions.split_off(location.statement_number as usize);
            // Save parameters
            // let (save_vars, restore_vars) = generate_preamble(globals, &location.mutations);
            let new_sequence = generate_mutant_sequence(function_index, &location.mutations);
            instructions.extend_from_slice(&new_sequence);
            instructions.extend_from_slice(&tail[1..]);
        }

        Ok(())
    }

    /// Return a set of all function names in the module
    pub fn functions(&self) -> HashSet<String> {
        let callback: CallbackType<String> = &|_, location| {
            if let Some(function) = location.function {
                vec![function.into()]
            } else {
                vec![]
            }
        };

        let results = self.instruction_walker(callback).unwrap_or_default();
        results.into_iter().collect()
    }

    /// Return a set of all file names in the module
    pub fn source_files(&self) -> HashSet<String> {
        let callback: CallbackType<String> = &|_, location| {
            if let Some(file) = location.file {
                vec![file.into()]
            } else {
                vec![]
            }
        };

        let results = self.instruction_walker(callback).unwrap_or_default();
        results.into_iter().collect()
    }

    /// Examine import section and function section of the module
    /// to check which call instruction may be removed using
    /// the `call_remove_*` operators.
    pub fn call_removal_candidates(&self) -> Result<Vec<CallRemovalCandidate>> {
        let type_section = self
            .module
            .type_section()
            .context("Module has no type section")?;

        let mut candidates = Vec::new();

        let check_type = |index: u32, type_ref: usize| {
            let ty = type_section.types().get(type_ref)?;

            let Type::Function(func_type) = ty;

            if func_type.results().is_empty() {
                Some(CallRemovalCandidate::FuncReturningVoid {
                    index,
                    params: func_type.params().into(),
                })
            } else if func_type.results().len() == 1 {
                Some(CallRemovalCandidate::FuncReturningScalar {
                    index,
                    params: func_type.params().into(),
                    return_type: func_type.results()[0].into(),
                })
            } else {
                None
            }
        };

        if let Some(import_section) = self.module.import_section() {
            for (index, import) in import_section.entries().iter().enumerate() {
                if let External::Function(type_ref) = import.external() {
                    if let Some(f) = check_type(index as u32, *type_ref as usize) {
                        candidates.push(f);
                    }
                }
            }
        }

        if let Some(function_section) = self.module.function_section() {
            let number_of_imports = self
                .module
                .import_section()
                .map(|f| f.entries().len())
                .unwrap_or(0);

            for (index, func) in function_section.entries().iter().enumerate() {
                let index = index + number_of_imports;
                if let Some(f) = check_type(index as u32, func.type_ref() as usize) {
                    candidates.push(f);
                }
            }
        }

        Ok(candidates)
    }

    /// Insert calls to our coverage tracing function.
    pub fn insert_trace_points(&mut self) -> Result<()> {
        // Make sure that the type signature of the trace function
        // is contained in the function table
        let type_index = self.find_or_insert_trace_function_signature()?;

        // Add trace function to the import section
        let function_index = self.add_trace_function_import("__wasmut_trace", type_index)?;

        // Increment all function-indices, since the
        // function section now contains the trace_function at index 0
        self.fix_call_instructions();
        self.fix_tables();
        self.fix_exports();

        // Finally, insert a call to the trace function before every single
        // instruction
        self.insert_trace_calls(function_index);

        Ok(())
    }

    fn find_or_insert_trace_function_signature(&mut self) -> Result<u32> {
        self.find_or_insert_type_signature(&[ValueType::I64], &[])
    }

    fn find_or_insert_check_mutant_function_signature(&mut self) -> Result<u32> {
        self.find_or_insert_type_signature(&[ValueType::I64], &[ValueType::I32])
    }

    fn find_or_insert_type_signature(
        &mut self,
        params: &[ValueType],
        results: &[ValueType],
    ) -> Result<u32> {
        let type_section = self
            .module
            .type_section_mut()
            .context("module does not have a type section")?;

        let types = type_section.types_mut();

        let index = types
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let Type::Function(f) = t;
                if f.params() == params && f.results() == results {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .next();

        Ok(index.unwrap_or_else(|| {
            types.push(Type::Function(FunctionType::new(
                params.into(),
                results.into(),
            )));
            (types.len() - 1) as u32
        }))
    }

    fn add_trace_function_import(&mut self, func_name: &str, type_index: u32) -> Result<u32> {
        // TODO: What should happen if there aren't imports there yet?
        let import_section = self
            .module
            .import_section_mut()
            .context("Module does not have an import section")?
            .entries_mut();

        import_section.insert(
            0,
            ImportEntry::new(
                "wasmut_api".into(),
                func_name.into(),
                External::Function(type_index),
            ),
        );

        Ok(0)
    }

    fn fix_tables(&mut self) {
        if let Some(table_section) = self.module.table_section() {
            let function_tables_indices = table_section
                .entries()
                .iter()
                .enumerate()
                .filter_map(|(i, table)| {
                    if table.elem_type() == TableElementType::AnyFunc {
                        Some(i as u32)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<u32>>();

            if let Some(element_section) = self.module.elements_section_mut() {
                for entry in element_section.entries_mut() {
                    if function_tables_indices.contains(&entry.index()) {
                        entry
                            .members_mut()
                            .iter_mut()
                            .for_each(|func_index| *func_index += 1)
                    }
                }
            }
        }
    }

    fn fix_call_instructions(&mut self) {
        if let Some(code_section) = self.module.code_section_mut() {
            for func_body in code_section.bodies_mut() {
                for instruction in func_body.code_mut().elements_mut() {
                    if let Instruction::Call(index) = instruction {
                        *index += 1;
                    }
                }
            }
        }
    }

    fn fix_exports(&mut self) {
        if let Some(export_section) = self.module.export_section_mut() {
            for entry in export_section.entries_mut() {
                if let Internal::Function(index) = entry.internal_mut() {
                    *index += 1;
                }
            }
        }
    }

    fn insert_trace_calls(&mut self, function_index: u32) {
        if let Some(code_section) = self.module.code_section_mut() {
            let code_section_offset = code_section.offset();

            for func_body in code_section.bodies_mut() {
                let code = func_body.code_mut();

                let mut instructions = Vec::new();

                for (instr, instr_offset) in code.elements().iter().zip(code.offsets()) {
                    let offset = instr_offset - code_section_offset;

                    instructions.push(Instruction::I64Const(offset as i64));
                    instructions.push(Instruction::Call(function_index));
                    instructions.push(instr.clone());
                }

                *code.elements_mut() = instructions;
            }
        }
    }

    /// Serialize module
    ///
    /// Debug information that may have been present in the original module
    /// is discarded.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        parity_wasm::serialize(self.module.clone()).context("Failed to serialize module")
    }

    /// Create a clone and apply a mutation
    pub fn clone_and_mutate(&self, location: &MutationLocation, mutation_index: usize) -> Self {
        let mut mutant = self.clone();
        mutant.mutate(&location, mutation_index);
        mutant
    }

    /// Create a clone and apply a mutation
    pub fn clone_and_mutate_all(&self, locations: &[MutationLocation]) -> Self {
        let mut mutant = self.clone();
        mutant.mutate_all(&locations);
        mutant
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

fn generate_mutant_sequence(func_index: u32, mutations: &[Mutation]) -> Vec<Instruction> {
    let mut instructions = Vec::new();

    let mutation = mutations
        .get(0)
        .expect("mutation slice is empty, this is bug.");

    instructions.push(Instruction::I64Const(mutation.id));
    instructions.push(Instruction::Call(func_index));
    instructions.push(Instruction::If(mutation.operator.result()));

    instructions.append(&mut mutation.operator.replacement());
    instructions.push(Instruction::Else);

    let next = &mutations[1..];
    if next.is_empty() {
        instructions.push(mutations[0].operator.old_instruction().clone());
    } else {
        instructions.append(&mut generate_mutant_sequence(func_index, next));
    }

    instructions.push(Instruction::End);

    instructions
}

#[cfg(test)]
mod tests {
    use crate::operator::ops::{
        BinaryOperatorAddToSub, BinaryOperatorMulToDivS, BinaryOperatorMulToDivU,
    };

    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
    use anyhow::Result;
    use parity_wasm::elements::BlockType;

    #[test]
    #[should_panic]
    fn generate_empty_case() {
        generate_mutant_sequence(1337, &[]);
    }

    #[test]
    fn generate_base_case() {
        let result = generate_mutant_sequence(
            1337,
            &[Mutation {
                id: 1234,
                operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
            }],
        );

        assert_eq!(
            result,
            vec![
                Instruction::I64Const(1234),
                Instruction::Call(1337),
                Instruction::If(BlockType::Value(ValueType::I32)),
                Instruction::I32Sub,
                Instruction::Else,
                Instruction::I32Add,
                Instruction::End
            ]
        );
    }

    #[test]
    fn generate_recursive_case() {
        let result = generate_mutant_sequence(
            1337,
            &[
                Mutation {
                    id: 1234,
                    operator: Box::new(BinaryOperatorMulToDivS::new(&Instruction::I32Mul).unwrap()),
                },
                Mutation {
                    id: 1235,
                    operator: Box::new(BinaryOperatorMulToDivU::new(&Instruction::I32Mul).unwrap()),
                },
            ],
        );

        assert_eq!(
            result,
            vec![
                Instruction::I64Const(1234),
                Instruction::Call(1337),
                Instruction::If(BlockType::Value(ValueType::I32)),
                Instruction::I32DivS,
                Instruction::Else,
                Instruction::I64Const(1235),
                Instruction::Call(1337),
                Instruction::If(BlockType::Value(ValueType::I32)),
                Instruction::I32DivU,
                Instruction::Else,
                Instruction::I32Mul,
                Instruction::End,
                Instruction::End
            ]
        );
    }

    #[test]
    fn test_load_from_file() {
        assert!(WasmModule::from_file("testdata/simple_add/test.wasm").is_ok());
    }

    #[test]
    fn test_into_buffer() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let _: Vec<u8> = module.to_bytes()?;
        Ok(())
    }

    #[test]
    fn get_functions() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let functions = module.functions();
        assert!(functions.contains("_start"));
        assert!(functions.contains("add"));
        Ok(())
    }

    #[test]
    fn get_files() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let files = module.source_files();

        assert_eq!(files.len(), 2);
        for file in files {
            assert!(file.ends_with("simple_add.c") || file.ends_with("test.c"));
        }

        Ok(())
    }

    #[test]
    fn scalar_functions() -> Result<()> {
        use CallRemovalCandidate::*;
        use Datatype::*;

        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;

        let result = module.call_removal_candidates().unwrap();
        dbg!(&result);

        let expected = vec![
            FuncReturningVoid {
                index: 0,
                params: [ValueType::I32].into(),
            },
            FuncReturningVoid {
                index: 1,
                params: [].into(),
            },
            FuncReturningScalar {
                index: 2,
                params: [ValueType::I32, ValueType::I32].into(),
                return_type: I32,
            },
            FuncReturningScalar {
                index: 3,
                params: [].into(),
                return_type: I32,
            },
        ];
        assert_eq!(result[0..4], expected);
        Ok(())
    }

    #[test]
    fn find_or_insert_type_signature_should_insert() -> Result<()> {
        let mut module = WasmModule::from_file("testdata/factorial/test.wasm")?;
        let index = module.find_or_insert_trace_function_signature()?;
        assert_eq!(index, 4);
        Ok(())
    }

    #[test]
    fn find_or_insert_type_signature_reuse() -> Result<()> {
        let mut module = WasmModule::from_file("testdata/i64_param/test.wasm")?;
        let index = module.find_or_insert_trace_function_signature()?;
        assert_eq!(index, 2);
        Ok(())
    }

    #[test]
    fn add_trace_function_import_expected_function_index() -> Result<()> {
        let mut module = WasmModule::from_file("testdata/i64_param/test.wasm")?;
        let type_index = module.find_or_insert_trace_function_signature()?;
        let function_index = module.add_trace_function_import("__wasmut_trace", type_index)?;
        assert_eq!(function_index, 0);
        Ok(())
    }
}
