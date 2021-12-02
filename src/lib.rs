pub mod runtime;

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
pub struct MutationPosition {
    function_number: usize,
    statement_number: usize,
    operator_type: MutationType,
}

#[derive(Debug)]
pub struct TestFunction {
    pub name: String,
}
