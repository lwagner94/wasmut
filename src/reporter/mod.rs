pub mod cli;
pub mod html;
pub mod json;
mod rewriter;

use std::{
    collections::BTreeMap,
    convert::AsRef,
    fs::File,
    io::BufReader,
    io::{BufRead, Lines},
    path::Path,
};

use anyhow::{Context, Result};

use crate::{
    addressresolver::{AddressResolver, CodeLocation},
    executor::ExecutedMutant,
    operator::InstructionReplacement,
    runtime::ExecutionResult,
    wasmmodule::WasmModule,
};
use serde::Serialize;
use syntect::{
    easy::HighlightLines,
    highlighting::Theme,
    parsing::{SyntaxReference, SyntaxSet},
};

use self::rewriter::PathRewriter;

#[derive(Debug, PartialEq, Clone)]
pub enum MutationOutcome {
    Alive,
    Killed,
    Timeout,
    Error,
    Skipped,
}

impl From<ExecutionResult> for MutationOutcome {
    fn from(result: ExecutionResult) -> Self {
        match result {
            ExecutionResult::ProcessExit { exit_code, .. } => {
                if exit_code == 0 {
                    MutationOutcome::Alive
                } else {
                    MutationOutcome::Killed
                }
            }
            ExecutionResult::Timeout => MutationOutcome::Timeout,
            ExecutionResult::Error => MutationOutcome::Error,
            ExecutionResult::Skipped => MutationOutcome::Skipped,
        }
    }
}

#[derive(Debug)]
pub struct ReportableMutant {
    location: CodeLocation,
    outcome: MutationOutcome,
    operator: Box<dyn InstructionReplacement>,
}

pub fn prepare_results(
    module: &WasmModule,
    results: Vec<ExecutedMutant>,
) -> Result<Vec<ReportableMutant>> {
    let bytes = std::fs::read(module.path()).context("Could not read bytecode from file")?;

    let resolver = AddressResolver::new(&bytes);

    Ok(results
        .into_iter()
        .map(|result| ReportableMutant {
            location: resolver.lookup_address(result.offset).unwrap_or_default(),
            outcome: result.result.into(),
            operator: result.mutation_operator,
        })
        .collect())
}

// pub trait Reporter {
//     fn report(&self, executed_mutants: &[ReportableMutant]) -> Result<()>;
// }

type LineNumberMutantMap<'a> = BTreeMap<u64, Vec<&'a ReportableMutant>>;
type FileMutantMap<'a> = BTreeMap<String, LineNumberMutantMap<'a>>;

fn map_mutants_to_files<'a>(
    executed_mutants: &'a [ReportableMutant],
    path_rewriter: Option<&PathRewriter>,
) -> FileMutantMap<'a> {
    let mut file_mapping = BTreeMap::new();
    for mutant in executed_mutants {
        if let (Some(file), Some(line)) = (&mutant.location.file, mutant.location.line) {
            let file = if let Some(path_rewriter) = path_rewriter {
                path_rewriter.rewrite(file)
            } else {
                file.clone()
            };

            let entry = file_mapping
                .entry(file.clone())
                .or_insert_with(BTreeMap::new);
            let entry = entry.entry(line).or_insert_with(Vec::new);
            entry.push(mutant);
        }
    }
    file_mapping
}

fn read_lines<P>(filename: P) -> Result<Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}

#[derive(Serialize, Clone)]
pub struct AccumulatedOutcomes {
    pub total: i32,
    pub alive: i32,
    pub timeout: i32,
    pub killed: i32,
    pub error: i32,
    pub skipped: i32,
    pub mutation_score: f32,
}

impl AsRef<ReportableMutant> for ReportableMutant {
    fn as_ref(&self) -> &ReportableMutant {
        self
    }
}

pub fn accumulate_outcomes<E: AsRef<ReportableMutant>>(
    executed_mutants: &[E],
) -> AccumulatedOutcomes {
    let (alive, timeout, killed, error, skipped) =
        executed_mutants.iter().map(|e| e.as_ref()).fold(
            (0, 0, 0, 0, 0),
            |(alive, timeout, killed, error, skipped), outcome| match outcome.outcome {
                MutationOutcome::Alive => (alive + 1, timeout, killed, error, skipped),
                MutationOutcome::Killed => (alive, timeout, killed + 1, error, skipped),
                MutationOutcome::Timeout => (alive, timeout + 1, killed, error, skipped),
                MutationOutcome::Error => (alive, timeout, killed, error + 1, skipped),
                MutationOutcome::Skipped => (alive, timeout, killed, error, skipped + 1),
            },
        );
    let mutation_score = 100f32 * (timeout + killed + error) as f32
        / (alive + timeout + killed + error + skipped) as f32;

    AccumulatedOutcomes {
        total: executed_mutants.len() as i32,
        alive,
        timeout,
        killed,
        error,
        skipped,
        mutation_score,
    }
}

pub fn accumulate_outcomes_for_file(mutants: &LineNumberMutantMap) -> AccumulatedOutcomes {
    let mut all_outcomes: Vec<&ReportableMutant> = Vec::new();

    for mutants in mutants.values() {
        all_outcomes.extend(mutants.iter());
    }

    accumulate_outcomes(&all_outcomes)
}

struct SyntectContext {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntectContext {
    fn new(theme_name: &str) -> Self {
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes[theme_name].clone();

        let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();

        Self { syntax_set, theme }
    }

    fn file_context<P: AsRef<Path>>(&self, file: P) -> Result<SyntectFileContext<'_>> {
        Ok(SyntectFileContext {
            context: self,
            syntax: create_syntax_reference(&self.syntax_set, file)?,
        })
    }
}

impl Default for SyntectContext {
    fn default() -> Self {
        Self::new("InspiredGitHub")
    }
}

fn create_syntax_reference<P: AsRef<Path>>(
    syntax_set: &SyntaxSet,
    file: P,
) -> Result<&syntect::parsing::SyntaxReference> {
    let syntax = if let Some(extension) = file.as_ref().extension() {
        let file_extension = extension
            .to_os_string()
            .into_string()
            .ok()
            .context("Could not convert OsString to String")?;
        syntax_set
            .find_syntax_by_extension(&file_extension)
            // If the extension is unknown, we just use plain text
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    } else {
        // If we don't have a file extension, we just just the plain text
        // "highlighting"
        syntax_set.find_syntax_plain_text()
    };
    Ok(syntax)
}

struct SyntectFileContext<'a> {
    context: &'a SyntectContext,
    syntax: &'a SyntaxReference,
}

impl<'a> SyntectFileContext<'a> {
    fn terminal_string(&self, line: &str) -> Result<String> {
        let mut highlight = HighlightLines::new(self.syntax, &self.context.theme);
        let regions = highlight.highlight_line(line, &self.context.syntax_set)?;
        Ok(syntect::util::as_24_bit_terminal_escaped(
            &regions[..],
            false,
        ))
    }
}

#[cfg(test)]
mod tests {
    use wasmut_wasm::elements::Instruction;

    use crate::operator::ops::BinaryOperatorAddToSub;

    use super::*;

    #[test]
    fn unknown_extension() -> Result<()> {
        let ctx = SyntectContext::default();
        assert_eq!(&ctx.file_context("test.abc")?.syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn no_extension() -> Result<()> {
        let ctx = SyntectContext::default();
        assert_eq!(&ctx.file_context("test")?.syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn prepare_results_empty_lists() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        assert_eq!(prepare_results(&module, vec![]).unwrap().len(), 0);
        Ok(())
    }

    #[test]
    fn prepare_results_correct() {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm").unwrap();

        let executed_mutants = vec![
            ExecutedMutant {
                offset: 34,
                result: ExecutionResult::ProcessExit {
                    exit_code: 0,
                    execution_cost: 1337,
                },
                mutation_operator: Box::new(
                    BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap(),
                ),
            },
            ExecutedMutant {
                offset: 34,
                result: ExecutionResult::ProcessExit {
                    exit_code: 1,
                    execution_cost: 1337,
                },
                mutation_operator: Box::new(
                    BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap(),
                ),
            },
            ExecutedMutant {
                offset: 34,
                result: ExecutionResult::Timeout,
                mutation_operator: Box::new(
                    BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap(),
                ),
            },
            ExecutedMutant {
                offset: 34,
                result: ExecutionResult::Error,
                mutation_operator: Box::new(
                    BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap(),
                ),
            },
            ExecutedMutant {
                offset: 34,
                result: ExecutionResult::Skipped,
                mutation_operator: Box::new(
                    BinaryOperatorAddToSub::new(&Instruction::I32Add).unwrap(),
                ),
            },
        ];

        let results = prepare_results(&module, executed_mutants).unwrap();

        dbg!(&results);
        assert_eq!(results.len(), 5);

        assert!(results[0]
            .location
            .file
            .as_ref()
            .unwrap()
            .contains("testdata/simple_add/simple_add.c"));
        assert!(*results[0].location.line.as_ref().unwrap() == 3);
        assert!(*results[0].location.column.as_ref().unwrap() == 14);

        assert!(results[0].outcome == MutationOutcome::Alive);
        assert!(results[1].outcome == MutationOutcome::Killed);
        assert!(results[2].outcome == MutationOutcome::Timeout);
        assert!(results[3].outcome == MutationOutcome::Error);
        assert!(results[4].outcome == MutationOutcome::Skipped);
    }
}
