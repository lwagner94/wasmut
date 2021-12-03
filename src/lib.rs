pub mod error;
pub mod runtime;
pub mod wasmmodule;
#[derive(Debug)]
pub enum MutationType {
    BinaryOperatorReplacement(BinaryOperator, BinaryOperator),
}

#[derive(Debug)]
pub enum BinaryOperator {
    Addition,
    Subtraction,
    Multiplication,
    Division,
}

#[derive(Debug)]
pub struct TestFunction {
    pub name: String,
}
