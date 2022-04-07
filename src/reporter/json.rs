use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{config::ReportConfig, output};

use super::{rewriter::PathRewriter, ReportableMutant};

#[derive(Serialize, Deserialize)]
pub struct JSONMutant {
    pub operator: String,
    pub file: Option<String>,
    pub function: Option<String>,
    pub line: Option<u64>,
    pub outcome: String,
}

#[derive(Serialize, Deserialize)]
pub struct JSONSummary {
    pub execution_time: u64,
    pub mutants: i32,
    pub killed: i32,
    pub alive: i32,
    pub timeout: i32,
    pub error: i32,
    pub skipped: i32,
    pub mutation_score: f32,
}

#[derive(Serialize, Deserialize)]
pub struct JSONReport {
    pub file: String,
    pub mutants: Vec<JSONMutant>,
    pub summary: JSONSummary,
}

pub struct JSONReporter {
    path_rewriter: Option<PathRewriter>,
    file: String,
    execution_time: u64,
}

impl JSONReporter {
    pub fn new(config: &ReportConfig, wasmfile: &str, duration: &Duration) -> Result<Self> {
        let path_rewriter = if let Some((regex, replacement)) = &config.path_rewrite() {
            Some(PathRewriter::new(regex, replacement)?)
        } else {
            None
        };

        Ok(Self {
            path_rewriter,
            file: wasmfile.into(),
            execution_time: duration.as_millis() as u64,
        })
    }

    pub fn report(&self, executed_mutants: &[ReportableMutant]) -> Result<()> {
        let mutants = self.map_to_json_mutants(executed_mutants);

        let accumulated_outcomes = super::accumulate_outcomes(executed_mutants);

        let report = JSONReport {
            file: self.file.clone(),
            mutants,
            summary: JSONSummary {
                execution_time: self.execution_time,
                mutants: accumulated_outcomes.total,
                killed: accumulated_outcomes.killed,
                alive: accumulated_outcomes.alive,
                timeout: accumulated_outcomes.timeout,
                error: accumulated_outcomes.error,
                skipped: accumulated_outcomes.skipped,
                mutation_score: accumulated_outcomes.mutation_score,
            },
        };

        let s = serde_json::to_string_pretty(&report)?;

        output::output_string(s);

        Ok(())
    }

    fn map_to_json_mutants(&self, executed_mutants: &[super::ReportableMutant]) -> Vec<JSONMutant> {
        let mutants = executed_mutants
            .iter()
            .map(|em| {
                let file = em.location.file.as_deref().map(|f| {
                    if let Some(path_rewriter) = &self.path_rewriter {
                        path_rewriter.rewrite(f)
                    } else {
                        f.into()
                    }
                });

                let outcome: String = em.outcome.clone().into();

                JSONMutant {
                    operator: em.operator.dyn_name().into(),
                    file,
                    function: em.location.function.clone(),
                    line: em.location.line,
                    outcome: outcome.to_lowercase(),
                }
            })
            .collect::<Vec<_>>();
        mutants
    }
}
