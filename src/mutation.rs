use crate::operator::InstructionContext;
use crate::operator::InstructionReplacement;
use crate::operator::OperatorRegistry;
use crate::wasmmodule::CallbackType;
use crate::{config::Config, policy::MutationPolicy, wasmmodule::WasmModule};
use anyhow::Result;
use atomic_counter::AtomicCounter;
use atomic_counter::RelaxedCounter;

/// Definition of a position where and how a module is mutated.
#[derive(Debug, Clone)]
pub struct Mutation {
    /// A unique ID for this mutation
    pub id: i64,

    /// The mutation operator that is to be applied
    pub operator: Box<dyn InstructionReplacement>,
}

#[derive(Debug, Clone)]
pub struct MutationLocation {
    /// The index in the module's function table
    pub function_number: u64,

    /// The index of the instruction to be mutated, relative to the start
    /// of the function
    pub statement_number: u64,

    /// The offset in bytes relative to the start of the code section
    pub offset: u64,

    /// All mutations for this location
    pub mutations: Vec<Mutation>,
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
    pub fn discover_mutation_positions(
        &self,
        module: &WasmModule,
    ) -> Result<Vec<MutationLocation>> {
        // Instantiate operator registry
        let registry = OperatorRegistry::new(&self.enabled_operators)?;

        // Find functions with no return / scalar return value.
        // Calls to those functions may be removed by call_remove* operators
        let call_removal_candidates = module.call_removal_candidates()?;
        let context = InstructionContext::new(call_removal_candidates);

        let id_counter = RelaxedCounter::new(1);

        // Define a callback function that is used by wasmmodule::instruction_walker
        // The callback is called for every single instruction of the module
        // and is passed the instruction and the location within
        // the module.
        // TODO: Refactor so that we do not return a vec?
        let callback: CallbackType<MutationLocation> = &|instruction, location| {
            if self.mutation_policy.check(location.file, location.function) {
                let mutations: Vec<Mutation> = registry
                    .mutants_for_instruction(instruction, &context)
                    .into_iter()
                    .map(|operator| Mutation {
                        id: id_counter.inc() as i64,
                        operator,
                    })
                    .collect();

                if mutations.is_empty() {
                    vec![]
                } else {
                    let mutation_location = MutationLocation {
                        function_number: location.function_index,
                        statement_number: location.instruction_index,
                        offset: location.instruction_offset,
                        mutations,
                    };
                    vec![mutation_location]
                }
            } else {
                vec![]
            }
        };

        let mutations = module.instruction_walker::<MutationLocation>(callback)?;
        log::info!("Generated {} mutations", count_mutants(&mutations));

        Ok(mutations)
    }
}

fn count_mutants(locations: &[MutationLocation]) -> i32 {
    locations
        .iter()
        .fold(0, |acc, loc| acc + loc.mutations.len() as i32)
}

#[cfg(test)]
mod tests {
    use crate::operator::ops::BinaryOperatorMulToDivS;

    use super::*;
    use anyhow::Result;
    use parity_wasm::elements::Instruction;

    #[test]
    fn test_count_mutants() {
        assert_eq!(count_mutants(&[]), 0);

        let m = Mutation {
            id: 1234,
            operator: Box::new(BinaryOperatorMulToDivS::new(&Instruction::I32Mul).unwrap()),
        };

        assert_eq!(
            count_mutants(&[MutationLocation {
                function_number: 1,
                statement_number: 1,
                offset: 1337,
                mutations: vec![m.clone(); 2],
            }]),
            2
        );

        assert_eq!(
            count_mutants(&[
                MutationLocation {
                    function_number: 1,
                    statement_number: 1,
                    offset: 1337,
                    mutations: vec![m.clone(); 2],
                },
                MutationLocation {
                    function_number: 1,
                    statement_number: 1,
                    offset: 1337,
                    mutations: vec![m; 2],
                },
            ]),
            4
        );
    }

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

        let locations = engine.discover_mutation_positions(&module).unwrap();
        dbg!(&locations);

        let mutant = module.clone_and_mutate(&locations[0], 0);

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
