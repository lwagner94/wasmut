use std::collections::HashSet;

use crate::{
    addressresolver::AddressResolver,
    error::{Error, Result},
    mutation::Mutation,
};
use parity_wasm::elements::{External, Instruction, Module, Type, ValueType};
use rayon::prelude::*;

// TODO: Encapsulate parity_wasm::Instruction in own type?
pub type CallbackType<'a, R> =
    &'a (dyn Fn(&Instruction, &InstructionWalkerLocation) -> Vec<R> + Send + Sync);

pub struct InstructionWalkerLocation<'a> {
    pub file: Option<&'a str>,
    pub function: Option<&'a str>,
    pub function_index: u32,
    pub instruction_index: u32,
    pub instruction_offset: u32,
}

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
    FuncReturningVoid { index: u32, params: usize },

    /// Function returns scalar and has `params` parameters
    FuncReturningScalar {
        index: u32,
        params: usize,
        return_type: Datatype,
    },
}

#[derive(Clone)]
pub struct WasmModule {
    module: parity_wasm::elements::Module,
    // TODO: Make this cleaner
    pub bytes: Vec<u8>,
}

impl WasmModule {
    pub fn from_file(path: &str) -> Result<WasmModule> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<WasmModule> {
        let module: Module = parity_wasm::elements::deserialize_buffer(&bytes)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;

        Ok(WasmModule { module, bytes })
    }

    pub fn instruction_walker<R: Send>(&self, callback: CallbackType<R>) -> Result<Vec<R>> {
        let code_section = self
            .module
            .code_section()
            .ok_or(Error::WasmModuleNoCodeSection)?;

        Ok(code_section
            .bodies()
            .par_iter()
            .enumerate()
            .map_init(
                || AddressResolver::new(&self.bytes),
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
                                file: location.as_ref().and_then(|l| l.file),
                                function: location.as_ref().and_then(|l| l.function.as_deref()),
                                function_index: func_index as u32,
                                instruction_index: instr_index as u32,
                                instruction_offset: code_offset as u32,
                            },
                        ))
                    }

                    results
                },
            )
            .flatten_iter()
            .collect())
    }

    pub fn mutate(&mut self, mutation: &Mutation) {
        let instruction = self
            .module
            .code_section_mut()
            .expect("Module does not have a code section")
            .bodies_mut()
            .get_mut(mutation.function_number as usize)
            .expect("unexpected funtion index")
            .code_mut()
            .elements_mut()
            .get_mut(mutation.statement_number as usize)
            .expect("unexpected instruction index");

        // *instruction = mutation.instruction.parity_instruction();
        mutation.instruction.apply(instruction);
    }

    fn files_and_functions(&self) -> (HashSet<String>, HashSet<String>) {
        let resolver = AddressResolver::new(&self.bytes);

        let mut functions = HashSet::new();
        let mut files = HashSet::new();

        if let Some(code_section) = self.module.code_section() {
            let code_section_offset = code_section.offset();

            code_section.bodies().iter().for_each(|func_body| {
                let offsets = func_body.code().offsets();

                for offset in offsets.iter() {
                    let code_offset = *offset - code_section_offset;

                    if let Some(location) = resolver.lookup_address(code_offset) {
                        if let Some(ref file) = location.function {
                            functions.insert(file.clone());
                        }

                        if let Some(file) = location.file {
                            files.insert(file.into());
                        }
                    }
                }
            });
        }

        (files, functions)
    }

    pub fn functions(&self) -> HashSet<String> {
        self.files_and_functions().1
    }

    pub fn source_files(&self) -> HashSet<String> {
        self.files_and_functions().0
    }

    pub fn call_removal_candidates(&self) -> Result<Vec<CallRemovalCandidate>> {
        let type_section = self
            .module
            .type_section()
            .ok_or(Error::WasmModuleMalformed("No type section"))?;

        let mut candidates = Vec::new();

        let check_type = |index: u32, type_ref: usize| {
            let ty = type_section.types().get(type_ref)?;

            let Type::Function(func_type) = ty;

            let number_of_params = func_type.params().len();

            if func_type.results().is_empty() {
                Some(CallRemovalCandidate::FuncReturningVoid {
                    index,
                    params: number_of_params,
                })
            } else if func_type.results().len() == 1 {
                Some(CallRemovalCandidate::FuncReturningScalar {
                    index,
                    params: number_of_params,
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
}

impl TryFrom<WasmModule> for Vec<u8> {
    type Error = Error;
    fn try_from(module: WasmModule) -> Result<Vec<u8>> {
        let bytes = parity_wasm::serialize(module.module)
            .map_err(|e| Error::BytecodeSerialization { source: e })?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs::read;

    #[test]
    fn test_load_from_file() {
        assert!(WasmModule::from_file("testdata/simple_add/test.wasm").is_ok());
    }

    #[test]
    fn test_load_from_bytes() -> Result<()> {
        let bytecode = read("testdata/simple_add/test.wasm")?;
        WasmModule::from_bytes(bytecode)?;
        Ok(())
    }

    #[test]
    fn test_into_buffer() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let _: Vec<u8> = module.try_into()?;
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

        let expected = vec![
            FuncReturningVoid {
                index: 0,
                params: 1,
            },
            FuncReturningVoid {
                index: 1,
                params: 0,
            },
            FuncReturningScalar {
                index: 2,
                params: 2,
                return_type: I32,
            },
            FuncReturningScalar {
                index: 3,
                params: 0,
                return_type: I32,
            },
            FuncReturningScalar {
                index: 4,
                params: 0,
                return_type: I32,
            },
            FuncReturningScalar {
                index: 5,
                params: 0,
                return_type: I32,
            },
            FuncReturningVoid {
                index: 6,
                params: 1,
            },
            FuncReturningVoid {
                index: 7,
                params: 1,
            },
            FuncReturningVoid {
                index: 8,
                params: 0,
            },
            FuncReturningVoid {
                index: 9,
                params: 0,
            },
            FuncReturningVoid {
                index: 10,
                params: 1,
            },
            FuncReturningVoid {
                index: 11,
                params: 0,
            },
            FuncReturningScalar {
                index: 12,
                params: 0,
                return_type: I32,
            },
            FuncReturningScalar {
                index: 13,
                params: 0,
                return_type: I32,
            },
        ];
        assert_eq!(result, expected);
        Ok(())
    }
}
