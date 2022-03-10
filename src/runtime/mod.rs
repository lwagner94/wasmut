pub mod wasmer;

use std::collections::HashSet;

use crate::wasmmodule::WasmModule;

/// Result of an executed module
#[derive(Debug)]
pub enum ExecutionResult {
    /// Normal termination
    ProcessExit { exit_code: u32, execution_cost: u64 },
    /// Execution limit exceeded
    Timeout,

    /// Execution was skipped
    Skipped,

    /// Other error (e.g. module trapped)
    Error,
}

#[derive(Default, Clone)]
pub struct TracePoints {
    points: HashSet<u64>,
}

impl TracePoints {
    fn add_point(&mut self, offset: u64) {
        self.points.insert(offset);
    }

    pub fn is_covered(&self, offset: u64) -> bool {
        self.points.contains(&offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_points() {
        let mut trace_points = TracePoints::default();

        assert!(!trace_points.is_covered(0));
        assert!(!trace_points.is_covered(1337));

        trace_points.add_point(10);
        assert!(trace_points.is_covered(10));
    }
}
