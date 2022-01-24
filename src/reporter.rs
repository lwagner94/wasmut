use crate::executor::ExecutionOutcome;

pub fn report_results(results: &[ExecutionOutcome]) {
    let r = results
        .iter()
        .fold((0, 0, 0, 0), |acc, outcome| match outcome {
            ExecutionOutcome::Alive => (acc.0 + 1, acc.1, acc.2, acc.3),
            ExecutionOutcome::Timeout => (acc.0, acc.1 + 1, acc.2, acc.3),
            ExecutionOutcome::Killed => (acc.0, acc.1, acc.2 + 1, acc.3),
            ExecutionOutcome::ExecutionError => (acc.0, acc.1, acc.2, acc.3 + 1),
        });

    log::info!("Alive: {}", r.0);
    log::info!("Timeout: {}", r.1);
    log::info!("Killed: {}", r.2);
    log::info!("Error: {}", r.3);
}
