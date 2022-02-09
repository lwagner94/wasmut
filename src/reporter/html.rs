use std::{collections::BTreeMap, fs::File, io::BufWriter, path::Path};

use anyhow::Result;
use handlebars::Handlebars;
use serde::Serialize;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{
    config::{Config, ReportConfig},
    templates,
};

use super::{ExecutedMutant, LineNumberMutantMap, MutationOutcome, Reporter, SyntectContext};

impl From<MutationOutcome> for String {
    fn from(m: MutationOutcome) -> Self {
        match m {
            MutationOutcome::Alive => "ALIVE".into(),
            MutationOutcome::Killed => "KILLED".into(),
            MutationOutcome::Timeout => "TIMEOUT".into(),
            MutationOutcome::Error => "ERROR".into(),
        }
    }
}

pub struct HTMLReporter {
    output_directory: String,
    syntax_set: SyntaxSet,
}

impl HTMLReporter {
    pub fn new(_config: &ReportConfig, output_directory: &str) -> Self {
        Self {
            output_directory: output_directory.into(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
        }
    }

    fn generate_source_lines(
        &self,
        file: &str,
        mapping: &LineNumberMutantMap,
    ) -> Result<Vec<SourceLine>> {
        // let file_ctx = ctx.file_context(file);

        let syntax = if let Some(extension) = Path::new(file).extension() {
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

        let mut lines = Vec::new();

        for (line_nr, line) in super::read_lines(file)?.enumerate() {
            let line_nr = (line_nr + 1) as u64;
            let line = line?;

            let mut html_mutants = Vec::new();
            let mut css_line_class = "no-mutant".into();

            if let Some(mutants) = mapping.get(&line_nr) {
                if mutants.iter().any(|m| m.outcome == MutationOutcome::Alive) {
                    css_line_class = "alive".into();
                } else if mutants.iter().all(|m| m.outcome == MutationOutcome::Killed) {
                    css_line_class = "killed".into();
                } else {
                    css_line_class = "timeout-error".into();
                }

                for mutant in mutants {
                    html_mutants.push(HTMLMutant {
                        outcome: mutant.outcome.clone().into(),
                        text: mutant.operator.description(),
                    })
                }
            }

            let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
                &syntax,
                &self.syntax_set,
                ClassStyle::Spaced,
            );

            html_generator.parse_html_for_line_which_includes_newline(&format!("{}\n", line));

            let output_html = html_generator.finalize();
            // dbg!(output_html);
            lines.push(SourceLine {
                line_number: line_nr,
                number_of_mutations: html_mutants.len(),
                code: output_html,
                mutants: html_mutants,
                css_line_class,
            })
        }
        dbg!(&lines);
        Ok(lines)
    }
}

impl Reporter for HTMLReporter {
    fn report(&self, executed_mutants: &[super::ExecutedMutant]) -> Result<()> {
        let file_mapping = super::map_mutants_to_files(executed_mutants);

        let _ = std::fs::remove_dir_all(&self.output_directory);
        std::fs::create_dir(&self.output_directory)?;

        let template_engine = init_template_engine();

        let mut mutated_files = Vec::new();

        for (file, line_number_map) in file_mapping {
            let link = match self.generate_source_lines(&file, &line_number_map) {
                Ok(lines) => {
                    // TODO: Error handling?
                    let output_file = generate_filename(&file);

                    // TODO: report directory
                    let writer = BufWriter::new(File::create(format!(
                        "{}/{}",
                        self.output_directory, output_file
                    ))?);

                    let data = BTreeMap::from([
                        ("file", handlebars::to_json(&file)),
                        ("lines", handlebars::to_json(lines)),
                    ]);

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

        let data = BTreeMap::from([("source_files", handlebars::to_json(mutated_files))]);
        let writer = BufWriter::new(File::create(format!(
            "{}/index.html",
            &self.output_directory
        ))?);
        // TODO: Refactor error
        template_engine
            .render_to_write("index", &data, writer)
            .unwrap();

        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes["InspiredGitHub"].clone();

        let css = syntect::html::css_for_theme_with_class_style(&theme, ClassStyle::Spaced);

        std::fs::write(format!("{}/style.css", &self.output_directory), css).unwrap();

        Ok(())
    }
}

fn generate_filename(file: &str) -> String {
    let s = Path::new(file).file_name().unwrap().to_str().unwrap();
    let hash = md5::compute(s);
    format!("{s}-{hash:?}.html")
}

fn init_template_engine() -> Handlebars<'static> {
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
    handlebars
}

#[derive(Serialize, Debug)]
struct HTMLMutant {
    outcome: String,
    text: String,
}

#[derive(Serialize, Debug)]
struct SourceLine {
    line_number: u64,
    number_of_mutations: usize,
    mutants: Vec<HTMLMutant>,
    code: String,
    css_line_class: String,
}

#[derive(Serialize)]
struct MutatedFile {
    name: String,
    link: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    // #[test]
    // fn generate_source_lines_no_mutants() -> Result<()> {
    //     let ctx = SyntectContext::default();
    //     let result =
    //         generate_source_lines("testdata/simple_add/simple_add.c", &BTreeMap::new(), &ctx)?;
    //     assert_eq!(result.len(), 4);
    //     Ok(())
    // }

    // #[test]
    // fn generate_source_lines_invalid_file() -> Result<()> {
    //     let ctx = SyntectContext::default();
    //     assert!(generate_source_lines("testdata/invalid_file.c", &BTreeMap::new(), &ctx).is_err());
    //     Ok(())
    // }

    #[test]
    fn generate_filename_for_simple_add() -> Result<()> {
        let s = generate_filename("/home/lukas/Repos/wasmut/testdata/simple_add/simple_add.c");
        assert_eq!(&s, "simple_add.c-fa92c051d002ff3e94998e6acfc1f707.html");
        Ok(())
    }
}
