use crate::error::Result;
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
        // TODO: Move logic from WasmModule here
        let mutations = module.discover_mutation_positions(&self.mutation_policy);
        log::info!("Generated {} mutations", mutations.len());
        mutations
    }
}
