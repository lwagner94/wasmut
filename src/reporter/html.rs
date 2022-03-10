use std::{collections::BTreeMap, fs::File, io::BufWriter, path::Path};

use anyhow::{Context, Result};
use chrono::prelude::*;
use handlebars::{handlebars_helper, Handlebars};

use serde::Serialize;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::SyntaxSet,
};

use crate::{config::ReportConfig, templates};

use super::{
    rewriter::PathRewriter, AccumulatedOutcomes, LineNumberMutantMap, MutationOutcome,
    ReportableMutant, Reporter,
};

impl From<MutationOutcome> for String {
    /// Convert `MutationOutcome` to `String`
    fn from(m: MutationOutcome) -> Self {
        match m {
            MutationOutcome::Alive => "ALIVE".into(),
            MutationOutcome::Killed => "KILLED".into(),
            MutationOutcome::Timeout => "TIMEOUT".into(),
            MutationOutcome::Error => "ERROR".into(),
        }
    }
}

#[derive(PartialEq, Debug)]
enum BulmaClass {
    Success,
    Warning,
    Danger,
    Invalid,
}

impl BulmaClass {
    /// Deterine `BulmaClass` from mutation score
    fn from_mutation_score(score: f32) -> Self {
        match score {
            x if (0.0..50.0).contains(&x) => BulmaClass::Danger,
            x if (50.0..75.0).contains(&x) => BulmaClass::Warning,
            x if (75.0..=100.0).contains(&x) => BulmaClass::Success,
            _ => BulmaClass::Invalid,
        }
    }
}

impl From<BulmaClass> for String {
    /// Convert `BulmaClass` to concrete bulma class used in the `class` parameter
    fn from(b: BulmaClass) -> Self {
        match b {
            BulmaClass::Success => "is-success",
            BulmaClass::Warning => "is-warning",
            BulmaClass::Danger => "is-danger",
            BulmaClass::Invalid => "",
        }
        .into()
    }
}

impl From<AccumulatedOutcomes> for BulmaClass {
    /// Convert from `AccumulatedOutcomes` to `BulmaClass`
    fn from(a: AccumulatedOutcomes) -> Self {
        let total = a.alive + a.error + a.killed + a.timeout;

        if a.alive > 0 {
            // If any mutant is alive, show red
            BulmaClass::Danger
        } else if a.killed == total {
            // If all mutants were killed, green
            BulmaClass::Success
        } else {
            // Else, if some mutants were errors or timeouts, show yellow
            BulmaClass::Warning
        }
    }
}

pub struct HTMLReporter<'a> {
    output_directory: &'a Path,
    syntax_set: SyntaxSet,
    path_rewriter: Option<PathRewriter>,
}

impl<'a> Reporter for HTMLReporter<'a> {
    fn report(&self, executed_mutants: &[super::ReportableMutant]) -> Result<()> {
        // Prepare output directory
        self.create_output_directory()?;
        self.create_static_files()?;

        // Initialize template engine
        let template_engine = create_template_engine();

        // Create general report info (program version, date, etc.)
        let report_info = ReportInfo::new();

        // Render individual source files
        let source_files =
            self.render_source_files(executed_mutants, &report_info, &template_engine)?;

        // Render index.html
        self.render_index(
            executed_mutants,
            &source_files,
            &report_info,
            &template_engine,
        )?;

        Ok(())
    }
}

impl<'a> HTMLReporter<'a> {
    pub fn new(config: &ReportConfig, output_directory: &'a Path) -> Result<Self> {
        let path_rewriter = if let Some((regex, replacement)) = &config.path_rewrite() {
            Some(PathRewriter::new(regex, replacement)?)
        } else {
            None
        };

        Ok(Self {
            output_directory,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            path_rewriter,
        })
    }

    /// Instantiate Syntext HTML generator instance
    fn instantiate_html_generator(&self, file: &str) -> Result<ClassedHTMLGenerator> {
        let syntax = super::create_syntax_reference(&self.syntax_set, file)?;

        Ok(ClassedHTMLGenerator::new_with_class_style(
            syntax,
            &self.syntax_set,
            ClassStyle::Spaced,
        ))
    }

    /// Render all lines, with included InlineMutantDescriptions
    fn generate_source_lines(
        &self,
        file: &str,
        mapping: &LineNumberMutantMap,
    ) -> Result<Vec<SourceLine>> {
        let mut source_lines = Vec::new();

        for (line_nr, line) in super::read_lines(file)?.enumerate() {
            //  Iterator::enumerate is 0-based, line numbers start from 1
            let line_nr = line_nr as u64 + 1;
            let line = line?;

            let mutants_in_given_line = mapping
                .get(&line_nr)
                .map(|v| v.as_slice())
                .unwrap_or_else(|| &[]);

            let html_generator = self.instantiate_html_generator(file)?;

            source_lines.push(SourceLine::new(
                line_nr,
                &line,
                mutants_in_given_line,
                html_generator,
            ))
        }

        Ok(source_lines)
    }

    /// Create all static files needed for our HTML report
    fn create_static_files(&self) -> Result<()> {
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes["InspiredGitHub"].clone();
        let css = syntect::html::css_for_theme_with_class_style(&theme, ClassStyle::Spaced);
        std::fs::write(self.output_directory.join("syntax.css"), css)?;
        std::fs::write(self.output_directory.join("style.css"), templates::CSS)?;
        std::fs::write(
            self.output_directory.join("bulma.min.css"),
            templates::BULMA,
        )?;
        std::fs::write(
            self.output_directory.join("BULMA-LICENSE"),
            templates::BULMA_LICENSE,
        )?;

        Ok(())
    }

    /// Create the output directory
    fn create_output_directory(&self) -> Result<()> {
        std::fs::create_dir_all(&self.output_directory)?;
        Ok(())
    }

    /// Render individual source files
    fn render_source_files(
        &self,
        executed_mutants: &[ReportableMutant],
        report_info: &ReportInfo,
        template_engine: &Handlebars,
    ) -> Result<Vec<SourceFile>> {
        let mut source_files = Vec::new();
        let file_mapping =
            super::map_mutants_to_files(executed_mutants, self.path_rewriter.as_ref());
        for (file, line_number_map) in file_mapping {
            // line_number_map is map line_nr -> Vec<ExecutedMutants>

            let link = match self.generate_source_lines(&file, &line_number_map) {
                Ok(lines) => {
                    let html_filename = generate_html_filename(&file)?;

                    let writer =
                        BufWriter::new(File::create(self.output_directory.join(&html_filename))?);

                    let data = BTreeMap::from([
                        ("filename", handlebars::to_json(&file)),
                        ("lines", handlebars::to_json(lines)),
                        ("report_info", handlebars::to_json(report_info)),
                    ]);

                    template_engine.render_to_write("source_view", &data, writer)?;

                    Some(html_filename)
                }
                Err(e) => {
                    log::warn!("Could not render file {file}: {e:?} - skipping");
                    None
                }
            };

            let accumulated_outcomes = super::accumulate_outcomes_for_file(&line_number_map);

            source_files.push(SourceFile {
                name: file,
                link,
                accumulated_outcomes: accumulated_outcomes.clone(),
            });
        }
        Ok(source_files)
    }

    /// Render index file.
    fn render_index(
        &self,
        executed_mutants: &[ReportableMutant],
        source_files: &[SourceFile],
        report_info: &ReportInfo,
        template_engine: &Handlebars,
    ) -> Result<()> {
        let stats = super::accumulate_outcomes(executed_mutants);
        let data = BTreeMap::from([
            ("source_files", handlebars::to_json(source_files)),
            ("file", handlebars::to_json::<Option<String>>(None)),
            ("report_info", handlebars::to_json(&report_info)),
            ("stats", handlebars::to_json(&stats)),
        ]);
        let writer = BufWriter::new(File::create(self.output_directory.join("index.html"))?);
        template_engine
            .render_to_write("index", &data, writer)
            .unwrap();
        Ok(())
    }
}

/// Generate filename by taking the filename of a
/// given path and appending the hash of the full path.
fn generate_html_filename(file: &str) -> Result<String> {
    let file_name = Path::new(file)
        .file_name()
        .context("File has no filename")?;
    let file_name_as_str = file_name
        .to_str()
        .context("Could not convert OsStr to &str")?;

    let hash = md5::compute(file);
    Ok(format!("{file_name_as_str}-{hash:?}.html"))
}

handlebars_helper!(float_format: |x: f64| format!("{x:.1}"));
handlebars_helper!(score_to_class: |s: f64| {
    String::from(BulmaClass::from_mutation_score(s as f32))
});

fn create_template_engine() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();

    handlebars.set_strict_mode(true);
    handlebars
        .register_template_string("base", templates::BASE_TEMPLATE)
        .unwrap();
    handlebars
        .register_template_string("source_view", templates::SOURCE_VIEW)
        .unwrap();
    handlebars
        .register_template_string("index", templates::INDEX)
        .unwrap();

    handlebars.register_helper("float_format", Box::new(float_format));
    handlebars.register_helper("score_to_class", Box::new(score_to_class));

    handlebars
}

#[derive(Serialize)]
struct InlineMutantDescription {
    outcome: String,
    text: String,
}

#[derive(Serialize)]
struct SourceLine {
    line_number: u64,
    mutants: Vec<InlineMutantDescription>,
    code: String,
    mutant_tag_class: String,
    accumulated_outcomes: AccumulatedOutcomes,
}

impl SourceLine {
    fn new(
        line_nr: u64,
        line_content: &str,
        mutants: &[&ReportableMutant],
        mut html_generator: ClassedHTMLGenerator,
    ) -> Self {
        // Generate HTML code for a line of source code
        let line_including_newline = format!("{line_content}\n");
        html_generator.parse_html_for_line_which_includes_newline(&line_including_newline);
        let html = html_generator.finalize();

        // Accumulate mutants for the given line
        let accumulated_outcomes = super::accumulate_outcomes(mutants);

        // Generate inline mutant descriptions
        let inline_mutants = mutants
            .iter()
            .map(|mutant| InlineMutantDescription {
                outcome: mutant.outcome.clone().into(),
                text: mutant.operator.description(),
            })
            .collect();

        SourceLine {
            line_number: line_nr,
            code: html,
            mutants: inline_mutants,
            mutant_tag_class: BulmaClass::from(accumulated_outcomes.clone()).into(),
            accumulated_outcomes,
        }
    }
}

#[derive(Serialize)]
struct SourceFile {
    name: String,
    link: Option<String>,
    accumulated_outcomes: AccumulatedOutcomes,
}

#[derive(Serialize)]
struct ReportInfo {
    program_name: String,
    program_version: String,
    date: String,
    time: String,
}

impl ReportInfo {
    fn new() -> Self {
        let current_time = Local::now();

        ReportInfo {
            program_name: String::from(env!("CARGO_PKG_NAME")),
            program_version: String::from(env!("CARGO_PKG_VERSION")),
            date: format!("{}", current_time.format("%Y-%m-%d")),
            time: format!("{}", current_time.format("%H:%M:%S")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use quickcheck::quickcheck;
    use tempfile::tempdir;

    #[test]
    fn generate_source_lines_no_mutants() -> Result<()> {
        let output = tempdir()?;

        let reporter = HTMLReporter::new(&ReportConfig::default(), output.path())?;

        let result =
            reporter.generate_source_lines("testdata/simple_add/simple_add.c", &BTreeMap::new())?;
        assert_eq!(result.len(), 4);
        Ok(())
    }

    #[test]
    fn generate_source_lines_invalid_file() -> Result<()> {
        let output = tempdir()?;

        let reporter = HTMLReporter::new(&ReportConfig::default(), output.path())?;

        let result = reporter.generate_source_lines("testdata/invalid/invalid.c", &BTreeMap::new());
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn generate_filename_for_simple_add() -> Result<()> {
        let s =
            generate_html_filename("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c")?;
        assert_eq!(&s, "simple_add.c-ce4786400a5a428e3c19c99a8478f672.html");
        Ok(())
    }

    quickcheck! {
        fn test_bulma_class(mutation_score: f32) -> bool {
            let class: String = BulmaClass::from_mutation_score(mutation_score).into();
            if (0.0..50.0).contains(&mutation_score) {
                class == "is-danger"
            } else if (50.0..75.0).contains(&mutation_score) {
                class == "is-warning"
            } else if (75.0..=100.0).contains(&mutation_score) {
                class == "is-success"
            } else {
                class.is_empty()
            }
        }
    }

    #[test]
    fn test_bulma_class_boundaries() {
        assert_eq!(BulmaClass::from_mutation_score(0.0), BulmaClass::Danger);
        assert_eq!(BulmaClass::from_mutation_score(50.0), BulmaClass::Warning);
        assert_eq!(BulmaClass::from_mutation_score(75.0), BulmaClass::Success);
        assert_eq!(BulmaClass::from_mutation_score(100.0), BulmaClass::Success);
    }
}
