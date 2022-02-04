mod cli;
mod rewriter;

pub use cli::CLIReporter;

use std::{
    collections::BTreeMap,
    fs::File,
    io::BufReader,
    io::{BufRead, BufWriter},
    path::Path,
};

use crate::{
    addressresolver::{AddressResolver, CodeLocation},
    config::Config,
    error::Result,
    mutation::Mutation,
    operator::InstructionReplacement,
    runtime::ExecutionResult,
    templates,
    wasmmodule::WasmModule,
};
use handlebars::{to_json, Handlebars};
use serde::Serialize;
use syntect::{
    easy::HighlightLines,
    highlighting::Theme,
    html::highlighted_html_for_string,
    parsing::{SyntaxReference, SyntaxSet},
};

#[derive(Debug, PartialEq, Clone)]
pub enum MutationOutcome {
    Alive,
    Killed,
    Timeout,
    Error,
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
        }
    }
}

#[derive(Debug)]
pub struct ExecutedMutant {
    location: CodeLocation,
    outcome: MutationOutcome,
    operator: Box<dyn InstructionReplacement>,
}

pub fn prepare_results(
    module: &WasmModule,
    mutations: Vec<Mutation>,
    results: Vec<ExecutionResult>,
) -> Vec<ExecutedMutant> {
    let resolver = AddressResolver::new(&module.bytes);

    if mutations.len() != results.len() {
        panic!("Mutation/Execution result length mismatch, this is a bug!");
    }

    mutations
        .into_iter()
        .zip(results)
        .map(|(mutation, result)| ExecutedMutant {
            location: resolver.lookup_address(mutation.offset).unwrap_or_default(),
            outcome: result.into(),
            operator: mutation.operator,
        })
        .collect()
}

pub trait Reporter {
    fn report(&self, executed_mutants: &[ExecutedMutant]) -> Result<()>;
}

type LineNumberMutantMap<'a> = BTreeMap<u64, Vec<&'a ExecutedMutant>>;
type FileMutantMap<'a> = BTreeMap<String, LineNumberMutantMap<'a>>;

#[derive(Serialize)]
struct SourceLine {
    mutations: usize,
    code: String,
}

#[derive(Serialize)]
struct MutatedFile {
    name: String,
    link: Option<String>,
}

pub fn generate_html(_config: &Config, executed_mutants: &[ExecutedMutant]) -> Result<()> {
    let file_mapping = map_mutants_to_files(executed_mutants);

    let _ = std::fs::remove_dir_all("report");
    std::fs::create_dir("report")?;

    let template_engine = init_template_engine();

    let mut mutated_files = Vec::new();
    let context = SyntectContext::new("InspiredGitHub");

    for (file, line_number_map) in file_mapping {
        let link = match generate_source_lines(&file, &line_number_map, &context) {
            Ok(lines) => {
                // TODO: Error handling?
                let output_file = generate_filename(&file);

                // TODO: report directory
                let writer = BufWriter::new(File::create(format!("report/{output_file}"))?);

                let data = BTreeMap::from([("file", to_json(&file)), ("lines", to_json(lines))]);

                // TODO: Refactor error
                template_engine
                    .render_to_write("source_view", &data, writer)
                    .unwrap();

                Some(output_file)
            }
            Err(_) => {
                log::warn!("Could not render file {file} - skipping");
                None
            }
        };

        mutated_files.push(MutatedFile { name: file, link });
    }

    let data = BTreeMap::from([("source_files", to_json(mutated_files))]);
    let writer = BufWriter::new(File::create("report/index.html")?);
    // TODO: Refactor error
    template_engine
        .render_to_write("index", &data, writer)
        .unwrap();

    Ok(())
}

fn generate_filename(file: &str) -> String {
    let s = Path::new(file).file_name().unwrap().to_str().unwrap();
    let hash = md5::compute(s);
    format!("{s}-{hash:?}.html")
}

fn init_template_engine() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    handlebars
        .register_template_string("base", templates::BASE_TEMPLATE)
        .unwrap();
    handlebars
        .register_template_string("source_view", templates::SOURCE_VIEW)
        .unwrap();
    handlebars
        .register_template_string("index", templates::INDEX)
        .unwrap();
    handlebars
}

fn map_mutants_to_files(executed_mutants: &[ExecutedMutant]) -> FileMutantMap {
    let mut file_mapping = BTreeMap::new();
    for mutant in executed_mutants {
        if let (Some(file), Some(line)) = (&mutant.location.file, mutant.location.line) {
            let entry = file_mapping
                .entry(file.clone())
                .or_insert_with(BTreeMap::new);
            let entry = entry.entry(line).or_insert_with(Vec::new);
            entry.push(mutant);
        }
    }
    file_mapping
}

fn read_lines<P>(filename: P) -> Result<std::io::Lines<std::io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}

struct SyntectContext {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntectContext {
    fn new(theme_name: &str) -> Self {
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes[theme_name].clone();

        let syntax_set = syntect::parsing::SyntaxSet::load_defaults_nonewlines();

        Self { syntax_set, theme }
    }

    fn file_context<P: AsRef<Path>>(&self, file: P) -> SyntectFileContext<'_> {
        let syntax = if let Some(extension) = file.as_ref().extension() {
            let e = extension.to_os_string().into_string().unwrap();
            self.syntax_set
                .find_syntax_by_extension(&e)
                // If the extension is unknown, we just use plain text
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
        } else {
            // If we don't have a file extension, we just just the plain text
            // "highlighting"
            self.syntax_set.find_syntax_plain_text()
        };

        SyntectFileContext {
            context: self,
            syntax,
        }
    }
}

impl Default for SyntectContext {
    fn default() -> Self {
        Self::new("InspiredGitHub")
    }
}

struct SyntectFileContext<'a> {
    context: &'a SyntectContext,
    syntax: &'a SyntaxReference,
}

impl<'a> SyntectFileContext<'a> {
    fn generate_html(&self, line: &str) -> String {
        highlighted_html_for_string(
            line,
            &self.context.syntax_set,
            self.syntax,
            &self.context.theme,
        )
    }

    fn terminal_string(&self, line: &str) -> String {
        let mut highlight = HighlightLines::new(self.syntax, &self.context.theme);
        let regions = highlight.highlight(line, &self.context.syntax_set);
        syntect::util::as_24_bit_terminal_escaped(&regions[..], false)
    }
}

fn generate_source_lines(
    file: &str,
    mapping: &LineNumberMutantMap,
    ctx: &SyntectContext,
) -> Result<Vec<SourceLine>> {
    let file_ctx = ctx.file_context(file);

    let mut lines = Vec::new();

    for (line_nr, line) in read_lines(file)?.enumerate() {
        let line_nr = (line_nr + 1) as u64;
        let line = line?;

        let a = mapping.get(&line_nr).map_or(0, |v| v.len());
        lines.push(SourceLine {
            mutations: a,
            code: file_ctx.generate_html(&line),
        })
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn generate_source_lines_no_mutants() -> Result<()> {
        let ctx = SyntectContext::default();
        let result =
            generate_source_lines("testdata/simple_add/simple_add.c", &BTreeMap::new(), &ctx)?;
        assert_eq!(result.len(), 4);
        Ok(())
    }

    #[test]
    fn generate_source_lines_invalid_file() -> Result<()> {
        let ctx = SyntectContext::default();
        assert!(generate_source_lines("testdata/invalid_file.c", &BTreeMap::new(), &ctx).is_err());
        Ok(())
    }

    #[test]
    fn unknown_extension() -> Result<()> {
        let ctx = SyntectContext::default();
        assert_eq!(&ctx.file_context("test.abc").syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn no_extension() -> Result<()> {
        let ctx = SyntectContext::default();
        assert_eq!(&ctx.file_context("test").syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn generate_filename_for_simple_add() -> Result<()> {
        let s = generate_filename("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c");
        assert_eq!(&s, "simple_add.c-fa92c051d002ff3e94998e6acfc1f707.html");
        Ok(())
    }

    #[test]
    fn prepare_results_empty_lists() -> Result<()> {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm")?;
        assert_eq!(prepare_results(&module, vec![], vec![]).len(), 0);
        Ok(())
    }

    #[test]
    #[should_panic]
    fn prepare_results_length_mismatch() {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm").unwrap();
        let _ = prepare_results(&module, vec![], vec![ExecutionResult::Timeout]);
    }

    #[test]
    fn prepare_results_correct() {
        let module = WasmModule::from_file("testdata/simple_add/test.wasm").unwrap();

        let mutation = Mutation {
            function_number: 1,
            statement_number: 2,
            offset: 34,
            operator: Box::new(crate::operator::ops::BinaryOperatorAddToSub(
                parity_wasm::elements::Instruction::I32Add,
                parity_wasm::elements::Instruction::I32Sub,
            )),
        };

        let results = prepare_results(&module, vec![mutation], vec![ExecutionResult::Timeout]);

        dbg!(&results);
        assert_eq!(results.len(), 1);

        assert!(results[0]
            .location
            .file
            .as_ref()
            .unwrap()
            .contains("testdata/simple_add/simple_add.c"));
        assert!(*results[0].location.line.as_ref().unwrap() == 3);
        assert!(*results[0].location.column.as_ref().unwrap() == 14);
    }
}
