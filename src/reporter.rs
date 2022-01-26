use crate::runtime::ExecutionResult;

pub fn report_results(results: &[ExecutionResult]) {
    let r = results
        .iter()
        .fold((0, 0, 0, 0), |acc, outcome| match outcome {
            ExecutionResult::ProcessExit { exit_code, .. } => {
                if *exit_code == 0 {
                    (acc.0 + 1, acc.1, acc.2, acc.3)
                } else {
                    (acc.0, acc.1, acc.2 + 1, acc.3)
                }
            }

            ExecutionResult::Timeout => (acc.0, acc.1 + 1, acc.2, acc.3),
            ExecutionResult::Error => (acc.0, acc.1, acc.2, acc.3 + 1),
        });

    log::info!("Alive: {}", r.0);
    log::info!("Timeout: {}", r.1);
    log::info!("Killed: {}", r.2);
    log::info!("Error: {}", r.3);
}
