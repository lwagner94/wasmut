use crate::error::Result;
use crate::operator::InstructionContext;
use crate::operator::InstructionReplacement;
use crate::operator::OperatorRegistry;
use crate::wasmmodule::CallbackType;
use crate::{config::Config, policy::MutationPolicy, wasmmodule::WasmModule};

#[derive(Debug)]
pub struct Mutation {
    pub function_number: u64,
    pub statement_number: u64,
    pub offset: u64,
    pub operator: Box<dyn InstructionReplacement>,
}

pub struct MutationEngine {
    mutation_policy: MutationPolicy,
}

impl MutationEngine {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            mutation_policy: MutationPolicy::from_config(config)?,
        })
    }

    pub fn discover_mutation_positions(&self, module: &WasmModule) -> Result<Vec<Mutation>> {
        let registry = OperatorRegistry::default();

        let r = &registry;

        let c = module.call_removal_candidates()?;

        let context = InstructionContext::new(c);

        let callback: CallbackType<Mutation> =
            &|instruction, location| {
                let mut mutations = Vec::new();

                if self.mutation_policy.check(location.file, location.function) {
                    mutations.extend(r.from_instruction(instruction, &context).into_iter().map(
                        |op| Mutation {
                            function_number: location.function_index,
                            statement_number: location.instruction_index,
                            offset: location.instruction_offset,
                            operator: op,
                        },
                    ));
                }

                mutations
            };

        let mutations = module.instruction_walker::<Mutation>(callback).unwrap();
        log::info!("Generated {} mutations", mutations.len());
        Ok(mutations)
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

        let positions = engine.discover_mutation_positions(&module).unwrap();

        assert!(!positions.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let engine = MutationEngine {
            mutation_policy: MutationPolicy::allow_all(),
        };

        let positions = engine.discover_mutation_positions(&module).unwrap();
        let mut mutant = module.clone();
        mutant.mutate(&positions[0]);

        let mutated_bytecode: Vec<u8> = mutant.try_into().unwrap();
        let original_bytecode: Vec<u8> = module.try_into().unwrap();

        assert_ne!(mutated_bytecode, original_bytecode);
        Ok(())
    }
}
