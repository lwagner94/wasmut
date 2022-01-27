use std::{
    collections::BTreeMap,
    fs::File,
    io::BufReader,
    io::{BufRead, BufWriter},
    path::Path,
};

use crate::{
    addressresolver::AddressResolver, config::Config, error::Result, operator::Mutation,
    runtime::ExecutionResult, templates, wasmmodule::WasmModule,
};
use handlebars::{to_json, Handlebars};
use serde::Serialize;
use syntect::{
    highlighting::Theme,
    html::highlighted_html_for_string,
    parsing::{SyntaxReference, SyntaxSet},
};

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

type LineNumberMutantMap<'a> = BTreeMap<u32, Vec<(&'a Mutation, &'a ExecutionResult)>>;
type FileMutantMap<'a, 'b> = BTreeMap<&'a str, LineNumberMutantMap<'b>>;

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

pub fn generate_html(
    _config: &Config,
    module: &WasmModule,
    mutations: &[Mutation],
    results: &[ExecutionResult],
) -> Result<()> {
    let resolver = AddressResolver::new(&module.bytes);
    let file_mapping = map_mutants_to_files(mutations, results, &resolver);

    let _ = std::fs::remove_dir_all("report");
    std::fs::create_dir("report")?;

    let template_engine = init_template_engine();

    let mut mutated_files = Vec::new();
    let context = SyntectContext::new();

    for (file, line_number_map) in file_mapping {
        let link = match generate_source_lines(file, &line_number_map, &context) {
            Ok(lines) => {
                // TODO: Error handling?
                let output_file = generate_filename(file);

                // TODO: report directory
                let writer = BufWriter::new(File::create(format!("report/{output_file}"))?);

                let data = BTreeMap::from([("file", to_json(file)), ("lines", to_json(lines))]);

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

        mutated_files.push(MutatedFile {
            name: file.into(),
            link,
        });
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

fn map_mutants_to_files<'a, 'r>(
    mutations: &'a [Mutation],
    results: &'a [ExecutionResult],
    resolver: &'r AddressResolver,
) -> FileMutantMap<'r, 'a> {
    let mut file_mapping = BTreeMap::new();
    for (mutation, result) in mutations.iter().zip(results) {
        let location = resolver
            .lookup_address(mutation.offset)
            .expect("Lookup failed");

        if location.locations.is_empty() {
            continue;
        }

        if let Some(file) = location.locations[0].file {
            if let Some(line) = location.locations[0].line {
                let entry = file_mapping.entry(file).or_insert_with(BTreeMap::new);
                let entry = entry.entry(line).or_insert_with(Vec::new);
                entry.push((mutation, result));
            }
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
    fn new() -> Self {
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes["InspiredGitHub"].clone();

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
}

fn generate_source_lines(
    file: &str,
    mapping: &LineNumberMutantMap,
    ctx: &SyntectContext,
) -> Result<Vec<SourceLine>> {
    let file_ctx = ctx.file_context(file);

    let mut lines = Vec::new();

    for (line_nr, line) in read_lines(file)?.enumerate() {
        let line_nr = (line_nr + 1) as u32;
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
        let ctx = SyntectContext::new();
        let result =
            generate_source_lines("testdata/simple_add/simple_add.c", &BTreeMap::new(), &ctx)?;
        assert_eq!(result.len(), 4);
        Ok(())
    }

    #[test]
    fn generate_source_lines_invalid_file() -> Result<()> {
        let ctx = SyntectContext::new();
        assert!(generate_source_lines("testdata/invalid_file.c", &BTreeMap::new(), &ctx).is_err());
        Ok(())
    }

    #[test]
    fn unknown_extension() -> Result<()> {
        let ctx = SyntectContext::new();
        assert_eq!(&ctx.file_context("test.abc").syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn no_extension() -> Result<()> {
        let ctx = SyntectContext::new();
        assert_eq!(&ctx.file_context("test").syntax.name, "Plain Text");
        Ok(())
    }

    #[test]
    fn generate_filename_for_simple_add() -> Result<()> {
        let s = generate_filename("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c");
        assert_eq!(&s, "simple_add.c-fa92c051d002ff3e94998e6acfc1f707.html");
        Ok(())
    }
}
