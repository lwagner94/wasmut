use crate::error::Result;
use crate::operator::MutableInstruction;
use crate::wasmmodule::CallbackType;
use crate::{config::Config, operator::Mutation, policy::MutationPolicy, wasmmodule::WasmModule};

pub struct MutationEngine {
    mutation_policy: MutationPolicy,
}

impl MutationEngine {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            mutation_policy: MutationPolicy::from_config(config)?,
        })
    }

    pub fn discover_mutation_positions(&self, module: &WasmModule) -> Vec<Mutation> {
        let callback: CallbackType<Mutation> = &|instruction, location| {
            let mut mutations = Vec::new();

            if let Some(instruction) = MutableInstruction::from_parity_instruction(instruction) {
                let mut should_mutate = false;
                if let Some(file) = location.file {
                    if self.mutation_policy.check_file(file) {
                        should_mutate = true;
                    }
                }

                if let Some(function) = &location.function {
                    if self.mutation_policy.check_function(function) {
                        should_mutate = true;
                    }
                }
                if should_mutate {
                    mutations.extend(instruction.generate_mutanted_instructions().iter().map(
                        |m| Mutation {
                            function_number: location.function_index,
                            statement_number: location.instruction_index,
                            offset: location.instruction_offset,
                            instruction: m.clone(),
                        },
                    ));
                }
            }

            mutations
        };

        let mutations = module.instruction_walker::<Mutation>(callback).unwrap();
        log::info!("Generated {} mutations", mutations.len());
        mutations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_discover_mutation_positions() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;

        let engine = MutationEngine {
            mutation_policy: MutationPolicy::allow_all(),
        };

        let positions = engine.discover_mutation_positions(&module);

        assert!(!positions.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let engine = MutationEngine {
            mutation_policy: MutationPolicy::allow_all(),
        };

        let positions = engine.discover_mutation_positions(&module);
        let mut mutant = module.clone();
        mutant.mutate(&positions[0]);

        let mutated_bytecode: Vec<u8> = mutant.try_into().unwrap();
        let original_bytecode: Vec<u8> = module.try_into().unwrap();

        assert_ne!(mutated_bytecode, original_bytecode);
        Ok(())
    }
}
