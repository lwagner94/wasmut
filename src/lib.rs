pub mod error;
pub mod operator;
pub mod runtime;
pub mod wasmmodule;

#[derive(Debug)]
pub struct TestFunction {
    pub name: String,
    pub expected_result: bool,
    pub function_type: TestFunctionType,
}

#[derive(Debug)]
pub enum TestFunctionType {
    StartEntryPoint,
    FuncReturningI32,
}

#[derive(Debug)]
pub enum ExecutionResult {
    // Normal termination
    FunctionReturn { return_value: i32 },
    ProcessExit { exit_code: u32 },
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
