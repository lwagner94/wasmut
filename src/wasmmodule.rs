use std::collections::HashSet;

use crate::{
    addressresolver::AddressResolver,
    error::{Error, Result},
    policy::MutationPolicy,
};
use parity_wasm::elements::Module;

use crate::operator::*;

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

    pub fn discover_mutation_positions(&self, mutation_policy: &MutationPolicy) -> Vec<Mutation> {
        let resolver = AddressResolver::new(&self.bytes);

        let mut mutation_positions = Vec::new();

        if let Some(code_section) = self.module.code_section() {
            let code_section_offset = code_section.offset();
            code_section
                .bodies()
                .iter()
                .enumerate()
                .for_each(|(function_number, func_body)| {
                    let instructions = func_body.code().elements();
                    let offsets = func_body.code().offsets();

                    for ((statement_number, parity_instr), offset) in
                        instructions.iter().enumerate().zip(offsets)
                    {
                        let code_offset = *offset - code_section_offset;

                        if let Some(instruction) =
                            MutableInstruction::from_parity_instruction(parity_instr)
                        {
                            // TODO: Refactor this, this is really ugly
                            let location = resolver.lookup_address(code_offset);

                            if let Some(location) = location {
                                let mut should_mutate = false;
                                if let Some(file) = location.file {
                                    if mutation_policy.check_file(file) {
                                        should_mutate = true;
                                    }
                                }

                                if let Some(function) = &location.function {
                                    if mutation_policy.check_function(function) {
                                        should_mutate = true;
                                    }
                                }
                                if should_mutate {
                                    mutation_positions.extend(
                                        instruction.generate_mutanted_instructions().iter().map(
                                            |m| Mutation {
                                                function_number: function_number as u64,
                                                statement_number: statement_number as u64,
                                                offset: code_offset,
                                                instruction: m.clone(),
                                            },
                                        ),
                                    );
                                }
                            }
                        }
                    }
                });
        }

        mutation_positions
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

        *instruction = mutation.instruction.parity_instruction();
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
    fn test_discover_mutation_positions() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let positions = module.discover_mutation_positions(&MutationPolicy::allow_all());

        assert!(!positions.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let positions = module.discover_mutation_positions(&MutationPolicy::allow_all());
        let mut mutant = module.clone();
        mutant.mutate(&positions[0]);

        let mutated_bytecode: Vec<u8> = mutant.try_into().unwrap();
        let original_bytecode: Vec<u8> = module.try_into().unwrap();

        assert_ne!(mutated_bytecode, original_bytecode);
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
