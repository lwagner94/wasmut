use colored::*;

use super::{
    rewriter::PathRewriter, MutationOutcome, ReportableMutant, SyntectContext, SyntectFileContext,
};
use crate::config::ReportConfig;
use crate::output;

use anyhow::{bail, Result};

pub struct CLIReporter {
    path_rewriter: Option<PathRewriter>,
    highlighter_context: SyntectContext,
    should_colorize: bool,
}

impl From<MutationOutcome> for ColoredString {
    fn from(m: MutationOutcome) -> Self {
        match m {
            MutationOutcome::Alive => "ALIVE".red(),
            MutationOutcome::Skipped => "SKIPPED".red(),
            MutationOutcome::Killed => "KILLED".green(),
            MutationOutcome::Timeout => "TIMEOUT".yellow(),
            MutationOutcome::Error => "ERROR".yellow(),
        }
    }
}

impl CLIReporter {
    pub fn new(config: &ReportConfig) -> Result<Self> {
        let path_rewriter = if let Some((regex, replacement)) = &config.path_rewrite() {
            Some(PathRewriter::new(regex, replacement)?)
        } else {
            None
        };

        Ok(CLIReporter {
            path_rewriter,
            highlighter_context: SyntectContext::new("Solarized (dark)"),
            should_colorize: control::ShouldColorize::from_env().should_colorize(),
        })
    }

    fn summary(&self, executed_mutants: &[ReportableMutant]) {
        let acc = super::accumulate_outcomes(executed_mutants);

        let alive_str: ColoredString = MutationOutcome::Alive.into();
        let skipped_str: ColoredString = MutationOutcome::Skipped.into();
        let timeout_str: ColoredString = MutationOutcome::Timeout.into();
        let error_str: ColoredString = MutationOutcome::Error.into();
        let killed_str: ColoredString = MutationOutcome::Killed.into();

        log::info!("{0:15} {1}", alive_str, acc.alive);
        log::info!("{0:15} {1}", skipped_str, acc.skipped);
        log::info!("{0:15} {1}", timeout_str, acc.timeout);
        log::info!("{0:15} {1}", error_str, acc.error);
        log::info!("{0:15} {1}", killed_str, acc.killed);
        log::info!("{0:15} {1:.1}%", "Mutation score", acc.mutation_score);
    }

    fn enumerate_mutants(&self, executed_mutants: &[ReportableMutant]) -> Result<()> {
        // Get a map filename -> (LineNumberMutantMap)
        let file_map: super::FileMutantMap =
            super::map_mutants_to_files(executed_mutants, self.path_rewriter.as_ref());

        for (file, line_map) in file_map {
            // line_map is map line_nr -> Vec<ExecutedMutants>

            let highlighter = self.highlighter_context.file_context(&file)?;

            for (_, mutants) in line_map {
                for mutant in mutants {
                    self.print_mutant(&file, mutant, &highlighter);
                    // if mutant.outcome == MutationOutcome::Alive {

                    // }
                }
            }
        }

        Ok(())
    }

    fn print_mutant(
        &self,
        file: &str,
        mutant: &ReportableMutant,
        highlighter: &SyntectFileContext,
    ) {
        let mut file_line_col = String::new();

        let mut line_in_file = String::new();
        let mut column_indicator = String::new();

        if mutant.location.file.as_deref().is_some() {
            file_line_col += file;

            if let Some(line_nr) = mutant.location.line {
                file_line_col += &format!(":{line_nr}");

                match Self::get_line_from_file(file, line_nr) {
                    Ok(line) => {
                        line_in_file = if self.should_colorize {
                            highlighter.terminal_string(&line).unwrap_or(line)
                        } else {
                            line
                        };
                    }
                    Err(e) => {
                        log::warn!("Could not read from file: {:?}", e);
                    }
                }

                if let Some(column) = mutant.location.column {
                    file_line_col += &format!(":{column}");

                    column_indicator = " ".repeat(column as usize) + "^";
                }
            }
        }

        let description = mutant.operator.description();
        let outcome: ColoredString = mutant.outcome.clone().into();

        let color_reset = "\x1b[0m";
        output::output_string(
            format!("{file_line_col}: \n{outcome}: {description}\n{line_in_file}{color_reset}\n{column_indicator}\n")
        );
    }

    fn get_line_from_file(file: &str, line_nr: u64) -> Result<String> {
        for (nr, line) in super::read_lines(file)?.enumerate() {
            let line = line?;

            // Line numbers start at 1, enumerations at 0,
            // so we need to subtract 1
            if nr as u64 == line_nr - 1 {
                return Ok(line);
            }
        }

        bail!("Could not read line {line_nr} from file {file}");
    }

    pub fn report(&self, executed_mutants: &[ReportableMutant]) -> Result<()> {
        self.enumerate_mutants(executed_mutants)?;
        self.summary(executed_mutants);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        addressresolver::CodeLocation, config::Config, operator::ops::BinaryOperatorAddToSub,
    };
    use wasmut_wasm::elements::Instruction;

    use super::*;
    #[test]
    fn get_line_from_file_works() {
        let line = CLIReporter::get_line_from_file("testdata/simple_add/simple_add.c", 3).unwrap();
        assert_eq!(&line, "    return a + b;");
    }

    #[test]
    fn get_line_from_file_err() {
        let line = CLIReporter::get_line_from_file("testdata/simple_add/simple_add.c", 6);
        assert!(line.is_err());
        let line = CLIReporter::get_line_from_file("invalid_file", 6);
        assert!(line.is_err());
    }

    fn report_to_string(executed_mutants: Vec<ReportableMutant>) -> String {
        let config = Config::parse(
            r#"
            [report]
            path_rewrite = ["^.*/wasmut/", ""]
        "#,
        )
        .unwrap();

        let reporter = CLIReporter::new(config.report()).unwrap();
        output::clear_output();
        reporter.report(&executed_mutants).unwrap();

        output::get_output()
    }

    #[test]
    fn cli_reporter_single_mutant() {
        let executed_mutants = vec![ReportableMutant {
            location: CodeLocation {
                file: Some("/home/user/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                function: Some("add".into()),
                line: Some(3),
                column: Some(14),
            },
            outcome: MutationOutcome::Timeout,
            operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
        }];

        let output = report_to_string(executed_mutants);

        assert!(output.contains("testdata/simple_add/simple_add.c:3:14"));
        assert!(output.contains("return"));
        assert!(output.contains("TIMEOUT"));
    }

    // #[test]
    // fn cli_reporter_summary() {
    //     let executed_mutants = vec![
    //         ReportableMutant {
    //             location: CodeLocation {
    //                 file: Some("/home/user/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
    //                 function: Some("add".into()),
    //                 line: Some(3),
    //                 column: Some(14),
    //             },
    //             outcome: MutationOutcome::Alive,
    //             operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
    //         },
    //         ReportableMutant {
    //             location: CodeLocation {
    //                 file: Some("/home/user/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
    //                 function: Some("add".into()),
    //                 line: Some(3),
    //                 column: Some(14),
    //             },
    //             outcome: MutationOutcome::Killed,
    //             operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
    //         },
    //         ReportableMutant {
    //             location: CodeLocation {
    //                 file: Some("/home/user/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
    //                 function: Some("add".into()),
    //                 line: Some(3),
    //                 column: Some(14),
    //             },
    //             outcome: MutationOutcome::Timeout,
    //             operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
    //         },
    //         ReportableMutant {
    //             location: CodeLocation {
    //                 file: Some("/home/user/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
    //                 function: Some("add".into()),
    //                 line: Some(3),
    //                 column: Some(14),
    //             },
    //             outcome: MutationOutcome::Error,
    //             operator: Box::new(BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap()),
    //         },
    //     ];

    //     let output = report_to_string(executed_mutants);

    //     assert!(output.contains("75.0%"));
    // }
}
