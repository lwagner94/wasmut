pub mod addressresolver;
pub mod config;
pub mod error;
pub mod executor;
pub mod mutation;
pub mod operator;
pub mod policy;
pub mod reporter;
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

#[cfg(test)]
mod tests {}
