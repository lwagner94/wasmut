use std::collections::HashSet;

use crate::{addressresolver::AddressResolver, mutation::Mutation};
use parity_wasm::elements::{External, Instruction, Module, Type, ValueType};

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
    FuncReturningVoid { index: u32, params: usize },

    /// Function returns scalar and has `params` parameters
    FuncReturningScalar {
        index: u32,
        params: usize,
        return_type: Datatype,
    },
}

/// WasmModule represents a (parsed) WebAssembly module
#[derive(Clone)]
pub struct WasmModule {
    module: parity_wasm::elements::Module,
    bytes: Vec<u8>,
}

impl WasmModule {
    /// Construct a new `WasmModule` from a file path
    pub fn from_file(path: &str) -> Result<WasmModule> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes)
    }

    /// Construct a new `WasmModule` from a bytearray
    pub fn from_bytes(bytes: Vec<u8>) -> Result<WasmModule> {
        let module: Module = parity_wasm::elements::deserialize_buffer(&bytes)
            .context("Bytecode deserialization failed")?;

        if !module.has_names_section() {
            log::warn!("Module has no name section, make sure to enable the debug flag!");
        }

        Ok(WasmModule { module, bytes })
    }

    /// Traverse module, and call callback function for every instruction
    pub fn instruction_walker<R: Send>(&self, callback: CallbackType<R>) -> Result<Vec<R>> {
        let code_section = self
            .module
            .code_section()
            .context("Module has no code section")?;

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
    pub fn mutate(&mut self, mutation: &Mutation) {
        let instructions = self
            .module
            .code_section_mut()
            .expect("Module does not have a code section")
            .bodies_mut()
            .get_mut(mutation.function_number as usize)
            .expect("unexpected funtion index")
            .code_mut()
            .elements_mut();

        mutation
            .operator
            .apply(instructions, mutation.statement_number);
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

    /// Serialize module
    ///
    /// Debug information that may have been present in the original module
    /// is discarded.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        parity_wasm::serialize(self.module.clone()).context("Failed to serialize module")
    }

    /// Create a clone and apply a mutation
    pub fn mutated_clone(&self, mutation: &Mutation) -> Self {
        let mut mutant = self.clone();
        mutant.mutate(mutation);
        mutant
    }

    // TODO: Maybe return an Option here when the module has been mutated?
    /// Return the original bytecode of the module
    pub fn original_bytes(&self) -> &[u8] {
        &self.bytes
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
