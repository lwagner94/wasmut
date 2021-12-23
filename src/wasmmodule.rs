use crate::{
    error::{Error, Result},
    policy::MutationPolicy,
};
use parity_wasm::elements::{ImportCountType, Module};

use crate::operator::*;

#[derive(Clone)]
pub struct WasmModule {
    module: parity_wasm::elements::Module,
}

impl WasmModule {
    // TODO: Allow wat
    pub fn from_file(path: &str) -> Result<WasmModule> {
        let mut module = parity_wasm::elements::deserialize_file(path)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;
        module = module.parse_names().unwrap();
        Ok(WasmModule { module })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<WasmModule> {
        let bytes = Vec::from(bytes);
        let mut module: Module = parity_wasm::elements::deserialize_buffer(&bytes)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;
        module = module.parse_names().unwrap();
        Ok(WasmModule { module })
    }

    pub fn discover_mutation_positions(&self, mutation_policy: &MutationPolicy) -> Vec<Mutation> {
        use parity_wasm::elements;

        let mut mutation_positions = Vec::new();

        let number_of_imports = self.module.import_count(ImportCountType::Function) as u32;

        // let start = time::Instant::now();
        let names = self.module.names_section().unwrap();
        let all_names = names.functions().unwrap().names();

        for section in self.module.sections() {
            // dbg!(section);

            if let elements::Section::Code(ref code_section) = *section {
                let code_section_offset = code_section.offset();
                let bodies = code_section.bodies();

                mutation_positions.extend(
                    bodies
                        .iter()
                        .enumerate()
                        .filter(|filter_op| {
                            let func_name = all_names
                                .get(filter_op.0 as u32 + number_of_imports)
                                .unwrap();

                            mutation_policy.check_function(func_name)
                        })
                        .flat_map(|(function_number, func_body)| {
                            let instructions = func_body.code().elements();
                            let offsets = func_body.code().offsets();

                            let mut mutations: Vec<Mutation> = Vec::new();

                            for ((statement_number, parity_instr), offset) in
                                instructions.iter().enumerate().zip(offsets)
                            {
                                if let Some(instruction) =
                                    MutableInstruction::from_parity_instruction(parity_instr)
                                {
                                    mutations.extend(
                                        instruction.generate_mutanted_instructions().iter().map(
                                            |m| Mutation {
                                                function_number: function_number as u64,
                                                statement_number: statement_number as u64,
                                                offset: *offset - code_section_offset,
                                                instruction: m.clone(),
                                            },
                                        ),
                                    );
                                }
                            }

                            // println!("Function: {}\n\n", cursor.position() - initial);

                            mutations
                        }),
                );
            }
        }

        mutation_positions
    }

    pub fn mutate(&mut self, mutation: &Mutation) {
        for section in self.module.sections_mut() {
            if let parity_wasm::elements::Section::Code(ref mut code_section) = *section {
                let bodies = code_section.bodies_mut();

                for (function_number, func_body) in bodies.iter_mut().enumerate() {
                    if function_number as u64 != mutation.function_number {
                        continue;
                    }
                    let instructions = func_body.code_mut().elements_mut();

                    let instr = instructions
                        .get_mut(mutation.statement_number as usize)
                        .unwrap();

                    *instr = mutation.instruction.parity_instruction();
                }
            }
        }
    }

    pub fn functions(&self) -> Vec<&str> {
        use parity_wasm::elements;

        let mut functions = Vec::new();

        // TODO: Extract function
        let number_of_imports = self.module.import_count(ImportCountType::Function) as u32;

        let names = self.module.names_section().unwrap();
        let all_names = names.functions().unwrap().names();

        for section in self.module.sections() {
            if let elements::Section::Code(ref code_section) = *section {
                for (idx, _) in code_section.bodies().iter().enumerate() {
                    let name = all_names
                        .get(idx as u32 + number_of_imports)
                        .unwrap()
                        .as_str();
                    functions.push(name);
                }
            }
        }

        functions
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
        WasmModule::from_bytes(&bytecode)?;
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
        let positions = module.discover_mutation_positions(&MutationPolicy::default());

        assert!(positions.len() > 0);
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let positions = module.discover_mutation_positions(&MutationPolicy::default());
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
        assert!(functions.contains(&"_start"));
        assert!(functions.contains(&"add"));
        Ok(())
    }
}
