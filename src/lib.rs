pub mod error;
pub mod operator;
pub mod runtime;
pub mod wasmmodule;

#[derive(Debug)]
pub struct TestFunction {
    pub name: String,
    pub expected_result: bool,
}

#[derive(Debug)]
pub enum ExecutionResult<T> {
    // Normal termination
    Normal { return_value: T, cost: u64 },
    // Execution limit exceeded
    LimitExceeded,

    // Other error
    Error,
}

pub enum ExecutionPolicy {
    // Run the function until the execution limit is reached
    RunUntilLimit {
        // The maximum number of instructions to execute
        limit: u64,
    },
    // Run the function until the function returns
    RunUntilReturn,
}
