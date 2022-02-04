use ansi_term::Color;
use std::{cell::RefCell, io::Write};

use super::{
    rewriter::PathRewriter, ExecutedMutant, MutationOutcome, Reporter, SyntectContext,
    SyntectFileContext,
};
use crate::{
    config::ReportConfig,
    error::{Error, Result},
};

pub struct CLIReporter<'a> {
    writer: RefCell<&'a mut dyn Write>,
    path_rewriter: Option<PathRewriter>,
    highlighter_context: SyntectContext,
}

impl std::fmt::Display for MutationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MutationOutcome::Alive => Color::Red.paint("ALIVE"),
            MutationOutcome::Killed => Color::Green.paint("KILLED"),
            MutationOutcome::Timeout => Color::Yellow.paint("TIMEOUT"),
            MutationOutcome::Error => Color::Yellow.paint("ERROR"),
        };

        write!(f, "{s}")
    }
}

impl<'a> CLIReporter<'a> {
    pub fn new<W: Write>(config: &ReportConfig, writer: &'a mut W) -> Result<Self> {
        let path_rewriter = if let Some((regex, replacement)) = &config.path_rewrite {
            Some(PathRewriter::new(regex, replacement)?)
        } else {
            None
        };

        Ok(CLIReporter {
            writer: RefCell::new(writer),
            path_rewriter,
            highlighter_context: SyntectContext::new("Solarized (dark)"),
        })
    }

    fn summary(&self, executed_mutants: &[ExecutedMutant]) {
        let (alive, timeout, killed, error) = executed_mutants.iter().fold(
            (0, 0, 0, 0),
            |(alive, timeout, killed, error), outcome| match outcome.outcome {
                MutationOutcome::Alive => (alive + 1, timeout, killed, error),
                MutationOutcome::Killed => (alive, timeout, killed + 1, error),
                MutationOutcome::Timeout => (alive, timeout + 1, killed, error),
                MutationOutcome::Error => (alive, timeout, killed, error + 1),
            },
        );
        let mut writer = self.writer.borrow_mut();

        let alive_str = format!("{}:", MutationOutcome::Alive);
        let timeout_str = format!("{}:", MutationOutcome::Timeout);
        let error_str = format!("{}:", MutationOutcome::Error);
        let killed_str = format!("{}:", MutationOutcome::Killed);

        writeln!(writer).unwrap();
        writeln!(writer, "{:>30} {}", alive_str, alive).unwrap();
        writeln!(writer, "{:>30} {}", timeout_str, timeout).unwrap();
        writeln!(writer, "{:>30} {}", error_str, error).unwrap();
        writeln!(writer, "{:>30} {}", killed_str, killed).unwrap();

        let mutation_score =
            100f32 * (timeout + killed + error) as f32 / (alive + timeout + killed + error) as f32;

        let f = format!("{}:", Color::White.paint("Mutation score"));

        writeln!(writer, "{:>30} {}%", f, mutation_score).unwrap();
    }

    fn enumerate_mutants(&self, executed_mutants: &[ExecutedMutant]) -> Result<()> {
        // Get a map filename -> (LineNumberMutantMap)
        let file_map: super::FileMutantMap = super::map_mutants_to_files(executed_mutants);

        for (file, line_map) in file_map {
            // line_map is map line_nr -> Vec<ExecutedMutants>

            let highlighter = self.highlighter_context.file_context(file);

            for (_, mutants) in line_map {
                for mutant in mutants {
                    self.print_mutant(mutant, &highlighter);
                    // if mutant.outcome == MutationOutcome::Alive {

                    // }
                }
            }
        }

        Ok(())
    }

    fn print_mutant(&self, mutant: &ExecutedMutant, highlighter: &SyntectFileContext) {
        let mut file_line_col = String::new();

        let mut line_in_file = String::new();
        let mut column_indicator = String::new();

        if let Some(file) = mutant.location.file.as_deref() {
            file_line_col += file;

            if let Some(line_nr) = mutant.location.line {
                file_line_col += &format!(":{line_nr}");

                let file = if let Some(path_rewriter) = &self.path_rewriter {
                    path_rewriter.rewrite(file)
                } else {
                    file.into()
                };

                match Self::get_line_from_file(&file, line_nr) {
                    Ok(line) => {
                        line_in_file = highlighter.terminal_string(&line);
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
        let outcome = &mutant.outcome;

        // let status = color.paint(format!("{:?}", mutant.outcome));

        let mut writer = self.writer.borrow_mut();

        let color_reset = "\x1b[0m";
        writeln!(
            writer,
            "{file_line_col}: \n{outcome}: {description}\n{line_in_file}{color_reset}\n{column_indicator}\n"
        )
        .unwrap();
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

        Err(Error::ReportGenerationFailed("Line not found"))
    }
}

impl<'a> Reporter for CLIReporter<'a> {
    fn report(&self, executed_mutants: &[ExecutedMutant]) -> Result<()> {
        self.enumerate_mutants(executed_mutants)?;
        self.summary(executed_mutants);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{addressresolver::CodeLocation, operator::ops::BinaryOperatorAddToSub};
    use parity_wasm::elements::Instruction;
    use std::io::{Read, Seek};

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

    fn report_to_string(executed_mutants: Vec<ExecutedMutant>) -> String {
        let buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(buffer);

        let config = ReportConfig {
            path_rewrite: Some(("/home/lukas/Repos/wasmut/".into(), "".into())),
        };

        let reporter = CLIReporter::new(&config, &mut cursor).unwrap();
        reporter.report(&executed_mutants).unwrap();
        let mut output = String::new();
        cursor.seek(std::io::SeekFrom::Start(0)).unwrap();
        cursor.read_to_string(&mut output).unwrap();
        output
    }

    #[test]
    fn cli_reporter_single_mutant() {
        let executed_mutants = vec![ExecutedMutant {
            location: CodeLocation {
                file: Some("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                function: Some("add".into()),
                line: Some(3),
                column: Some(14),
            },
            outcome: MutationOutcome::Timeout,
            operator: Box::new(BinaryOperatorAddToSub(
                Instruction::I32Add,
                Instruction::I32Sub,
            )),
        }];

        let output = report_to_string(executed_mutants);

        assert!(output.contains("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c:3:14"));
        assert!(output.contains("return"));
        assert!(output.contains("TIMEOUT"));
    }

    #[test]
    fn cli_reporter_summary() {
        let executed_mutants = vec![
            ExecutedMutant {
                location: CodeLocation {
                    file: Some("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                    function: Some("add".into()),
                    line: Some(3),
                    column: Some(14),
                },
                outcome: MutationOutcome::Alive,
                operator: Box::new(BinaryOperatorAddToSub(
                    Instruction::I32Add,
                    Instruction::I32Sub,
                )),
            },
            ExecutedMutant {
                location: CodeLocation {
                    file: Some("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                    function: Some("add".into()),
                    line: Some(3),
                    column: Some(14),
                },
                outcome: MutationOutcome::Killed,
                operator: Box::new(BinaryOperatorAddToSub(
                    Instruction::I32Add,
                    Instruction::I32Sub,
                )),
            },
            ExecutedMutant {
                location: CodeLocation {
                    file: Some("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                    function: Some("add".into()),
                    line: Some(3),
                    column: Some(14),
                },
                outcome: MutationOutcome::Timeout,
                operator: Box::new(BinaryOperatorAddToSub(
                    Instruction::I32Add,
                    Instruction::I32Sub,
                )),
            },
            ExecutedMutant {
                location: CodeLocation {
                    file: Some("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c".into()),
                    function: Some("add".into()),
                    line: Some(3),
                    column: Some(14),
                },
                outcome: MutationOutcome::Error,
                operator: Box::new(BinaryOperatorAddToSub(
                    Instruction::I32Add,
                    Instruction::I32Sub,
                )),
            },
        ];

        let output = report_to_string(executed_mutants);

        assert!(output.contains("75%"));
    }
}
