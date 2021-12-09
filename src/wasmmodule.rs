use crate::error::{Error, Result};
use parity_wasm::elements::{ImportCountType, Module};
use rayon::prelude::*;

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
        let mut module: Module = parity_wasm::elements::deserialize_buffer(bytes)
            .map_err(|e| Error::BytecodeDeserialization { source: e })?;
        module = module.parse_names().unwrap();
        Ok(WasmModule { module })
    }

    pub fn discover_mutation_positions(&self) -> Vec<Mutation> {
        use parity_wasm::elements;

        let mut mutation_positions = Vec::new();

        let number_of_imports = self.module.import_count(ImportCountType::Function) as u32;

        // let start = time::Instant::now();
        let names = self.module.names_section().unwrap();
        let all_names = names.functions().unwrap().names();

        for section in self.module.sections() {
            // dbg!(section);

            if let elements::Section::Code(ref code_section) = *section {
                let bodies = code_section.bodies();

                mutation_positions.par_extend(
                    bodies
                        .par_iter()
                        .enumerate()
                        .filter(|filter_op| {
                            let func_name = all_names
                                .get(filter_op.0 as u32 + number_of_imports)
                                .unwrap();
                            // println!("{}", &func_name);
                            func_name == "add"

                            // TODO: Filter functions here.
                            // true
                        })
                        .flat_map_iter(|(function_number, func_body)| {
                            let instructions = func_body.code().elements();

                            let mut mutations: Vec<Mutation> = Vec::new();

                            for (statement_number, parity_instr) in instructions.iter().enumerate()
                            {
                                if let Some(instruction) =
                                    MutableInstruction::from_parity_instruction(parity_instr)
                                {
                                    mutations.extend(
                                        instruction.generate_mutanted_instructions().iter().map(
                                            |m| Mutation {
                                                function_number,
                                                statement_number,
                                                instruction: m.clone(),
                                            },
                                        ),
                                    );
                                }
                            }

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
                    if function_number != mutation.function_number {
                        continue;
                    }
                    let instructions = func_body.code_mut().elements_mut();

                    let instr = instructions.get_mut(mutation.statement_number).unwrap();

                    *instr = mutation.instruction.parity_instruction();
                }
            }
        }
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
        let positions = module.discover_mutation_positions();

        assert!(positions.len() > 0);
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let positions = module.discover_mutation_positions();
        let mut mutant = module.clone();
        mutant.mutate(&positions[0]);

        let mutated_bytecode: Vec<u8> = mutant.try_into().unwrap();
        let original_bytecode: Vec<u8> = module.try_into().unwrap();

        assert_ne!(mutated_bytecode, original_bytecode);
        Ok(())
    }
}
