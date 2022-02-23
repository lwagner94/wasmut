use crate::operator::InstructionContext;
use crate::operator::InstructionReplacement;
use crate::operator::OperatorRegistry;
use crate::wasmmodule::CallbackType;
use crate::{config::Config, policy::MutationPolicy, wasmmodule::WasmModule};
use anyhow::Result;
use atomic_counter::AtomicCounter;
use atomic_counter::RelaxedCounter;

/// Definition of a position where and how a module is mutated.
#[derive(Debug)]
pub struct Mutation {
    /// A unique ID for this mutation
    pub id: i64,

    /// The index in the module's function table
    pub function_number: u64,

    /// The index of the instruction to be mutated, relative to the start
    /// of the function
    pub statement_number: u64,

    /// The offset in bytes relative to the start of the code section
    pub offset: u64,

    /// The mutation operator that is to be applied
    pub operator: Box<dyn InstructionReplacement>,
}

/// Used for discovering possible mutants based on
/// the module and a set of operators.
pub struct MutationEngine {
    /// The policy used to filter mutant candidates.
    mutation_policy: MutationPolicy,

    /// A list of all operators that are to be enabled.
    enabled_operators: Vec<String>,
}

impl MutationEngine {
    /// Create a new `MutationEngine`, based on a configuration.
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            mutation_policy: MutationPolicy::from_config(config)?,
            enabled_operators: config.operators().enabled_operators(),
        })
    }

    /// Discover all mutation candidates in a module.
    ///
    /// This method will return a vector of `Mutation` structs, representing the
    /// candidates.
    pub fn discover_mutation_positions(&self, module: &WasmModule) -> Result<Vec<Mutation>> {
        // Instantiate operator registry
        let registry = OperatorRegistry::new(&self.enabled_operators)?;

        // Find functions with no return / scalar return value.
        // Calls to those functions may be removed by call_remove* operators
        let call_removal_candidates = module.call_removal_candidates()?;
        let context = InstructionContext::new(call_removal_candidates);

        let id_counter = RelaxedCounter::new(0);

        // Define a callback function that is used by wasmmodule::instruction_walker
        // The callback is called for every single instruction of the module
        // and is passed the instruction and the location within
        // the module.
        let callback: CallbackType<Mutation> = &|instruction, location| {
            if self.mutation_policy.check(location.file, location.function) {
                registry
                    .mutants_for_instruction(instruction, &context)
                    .into_iter()
                    .map(|operator| Mutation {
                        id: id_counter.inc() as i64,
                        function_number: location.function_index,
                        statement_number: location.instruction_index,
                        offset: location.instruction_offset,
                        operator,
                    })
                    .collect()
            } else {
                vec![]
            }
        };

        let mutations = module.instruction_walker::<Mutation>(callback)?;
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
        let mutant = module.mutated_clone(&positions[0]);

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
