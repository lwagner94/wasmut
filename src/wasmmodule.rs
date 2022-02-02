use std::collections::HashSet;

use crate::{
    addressresolver::AddressResolver,
    error::{Error, Result},
    mutation::Mutation,
};
use parity_wasm::elements::{Instruction, Module};
use rayon::prelude::*;

pub type CallbackType<'a, R> =
    &'a (dyn Fn(&Instruction, &InstructionWalkerLocation) -> Vec<R> + Send + Sync);

pub struct InstructionWalkerLocation<'a> {
    pub file: Option<&'a str>,
    pub function: Option<&'a str>,
    pub function_index: u32,
    pub instruction_index: u32,
    pub instruction_offset: u32,
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

    // TODO: See if it makes sense to generalize tests for both runtimes?

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
}
