use crate::operator::InstructionContext;
use crate::operator::InstructionReplacement;
use crate::operator::OperatorRegistry;
use crate::wasmmodule::CallbackType;
use crate::{config::Config, policy::MutationPolicy, wasmmodule::WasmModule};
use anyhow::Result;

pub struct Mutation {
    pub function_number: u64,
    pub statement_number: u64,
    pub offset: u64,
    pub operator: Box<dyn InstructionReplacement>,
}

pub struct MutationEngine {
    mutation_policy: MutationPolicy,
    enabled_operators: Vec<String>,
}

impl MutationEngine {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            mutation_policy: MutationPolicy::from_config(config)?,
            enabled_operators: config.operators().enabled_operators(),
        })
    }

    pub fn discover_mutation_positions(&self, module: &WasmModule) -> Result<Vec<Mutation>> {
        let ops = self
            .enabled_operators
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        let registry = OperatorRegistry::new(&ops)?;

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

        let config = Config::default();
        let engine = MutationEngine::new(&config)?;
        let positions = engine.discover_mutation_positions(&module).unwrap();

        assert!(!positions.is_empty());
        Ok(())
    }

    #[test]
    fn test_mutation() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        let config = Config::default();
        let engine = MutationEngine::new(&config)?;

        let positions = engine.discover_mutation_positions(&module).unwrap();
        let mut mutant = module.clone();
        mutant.mutate(&positions[0]);

        let mutated_bytecode: Vec<u8> = mutant.to_bytes().unwrap();
        let original_bytecode: Vec<u8> = module.to_bytes().unwrap();

        assert_ne!(mutated_bytecode, original_bytecode);
        Ok(())
    }

    #[test]
    fn test_enable_only_some_operators() -> Result<()> {
        fn check_number_of_mutants(config: &str) -> usize {
            let module = WasmModule::from_file("testdata/count_words/test.wasm").unwrap();
            let config = Config::parse_file(format!("testdata/count_words/{config}")).unwrap();
            let engine = MutationEngine::new(&config).unwrap();
            engine.discover_mutation_positions(&module).unwrap().len()
        }

        assert_eq!(check_number_of_mutants("wasmut_call.toml"), 7);
        assert_eq!(check_number_of_mutants("wasmut_relops.toml"), 1);
        assert_eq!(check_number_of_mutants("wasmut.toml"), 23);
        Ok(())
    }
}
