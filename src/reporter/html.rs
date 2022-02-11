use std::{collections::BTreeMap, fs::File, io::BufWriter, path::Path};

use anyhow::Result;
use chrono::prelude::*;
use handlebars::Handlebars;

use serde::Serialize;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::SyntaxSet,
};

use crate::{config::ReportConfig, templates};

use super::{
    rewriter::PathRewriter, AccumulatedOutcomes, LineNumberMutantMap, MutationOutcome, Reporter,
};

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

pub fn bulma_class_from_mutation_score(score: f32) -> String {
    if score > 75.0 {
        "is-success".into()
    } else if score > 50.0 {
        "is-warning".into()
    } else {
        "is-danger".into()
    }
}

pub struct HTMLReporter {
    output_directory: String,
    syntax_set: SyntaxSet,
    path_rewriter: Option<PathRewriter>,
}

impl HTMLReporter {
    pub fn new(config: &ReportConfig, output_directory: &str) -> Result<Self> {
        let path_rewriter = if let Some((regex, replacement)) = &config.path_rewrite() {
            Some(PathRewriter::new(regex, replacement)?)
        } else {
            None
        };

        Ok(Self {
            output_directory: output_directory.into(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            path_rewriter,
        })
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
            let mut css_line_class = "is-white".into();
            let mut killed_mutants = 0;

            if let Some(mutants) = mapping.get(&line_nr) {
                if mutants.iter().any(|m| m.outcome == MutationOutcome::Alive) {
                    css_line_class = "is-danger".into();
                } else if mutants.iter().all(|m| m.outcome == MutationOutcome::Killed) {
                    css_line_class = "is-success".into();
                } else {
                    css_line_class = "is-warning".into();
                }

                killed_mutants = mutants.iter().fold(0usize, |acc, m| {
                    if m.outcome == MutationOutcome::Killed {
                        acc + 1
                    } else {
                        acc
                    }
                });

                for mutant in mutants {
                    html_mutants.push(HTMLMutant {
                        outcome: mutant.outcome.clone().into(),
                        text: mutant.operator.description(),
                    })
                }
            }

            let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
                syntax,
                &self.syntax_set,
                ClassStyle::Spaced,
            );

            html_generator.parse_html_for_line_which_includes_newline(&format!("{}\n", line));

            // let output_html = line;
            let output_html = html_generator.finalize();
            // dbg!(output_html);
            lines.push(SourceLine {
                line_number: line_nr,
                number_of_mutants: html_mutants.len(),
                number_of_killed_mutants: killed_mutants,
                code: output_html,
                mutants: html_mutants,
                css_line_class,
            })
        }
        Ok(lines)
    }

    fn accumulate_outcomes_for_file(&self, mutants: &LineNumberMutantMap) -> AccumulatedOutcomes {
        let mut all_outcomes = Vec::new();

        for mutants in mutants.values() {
            all_outcomes.extend(mutants.iter());
        }

        super::accumulate_outcomes_ref(&all_outcomes)
    }

    fn create_assets(&self) -> Result<()> {
        let output_dir = Path::new(&self.output_directory);

        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let theme = ts.themes["InspiredGitHub"].clone();
        let css = syntect::html::css_for_theme_with_class_style(&theme, ClassStyle::Spaced);
        std::fs::write(output_dir.join("syntax.css"), css)?;

        std::fs::write(output_dir.join("style.css"), templates::CSS)?;
        std::fs::write(output_dir.join("bulma.min.css"), templates::BULMA)?;
        std::fs::write(output_dir.join("BULMA-LICENSE"), templates::BULMA_LICENSE)?;

        Ok(())
    }
}

impl Reporter for HTMLReporter {
    fn report(&self, executed_mutants: &[super::ExecutedMutant]) -> Result<()> {
        let file_mapping = super::map_mutants_to_files(executed_mutants);

        let _ = std::fs::remove_dir_all(&self.output_directory);
        std::fs::create_dir(&self.output_directory)?;

        let template_engine = init_template_engine();

        let mut mutated_files = Vec::new();

        let info = GeneralInfo::new();

        for (file, line_number_map) in file_mapping {
            let file = if let Some(path_rewriter) = &self.path_rewriter {
                path_rewriter.rewrite(file)
            } else {
                file
            };

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
                        ("info", handlebars::to_json(&info)),
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

            let acc = self.accumulate_outcomes_for_file(&line_number_map);

            mutated_files.push(MutatedFile {
                name: file,
                link,
                mutation_score: format!("{:.1}", acc.mutation_score),
                alive: acc.alive,
                killed: acc.killed,
                timeout: acc.timeout,
                error: acc.error,
                // TODO: would it be nicer to move this to the template, using a helper?
                bulma_mutation_score_class: bulma_class_from_mutation_score(acc.mutation_score),
            });
        }

        let stats = super::accumulate_outcomes(executed_mutants);

        let total_mutation_score_class = bulma_class_from_mutation_score(stats.mutation_score);

        let data = BTreeMap::from([
            ("source_files", handlebars::to_json(mutated_files)),
            ("file", handlebars::to_json::<Option<String>>(None)),
            ("info", handlebars::to_json(&info)),
            ("stats", handlebars::to_json(&stats)),
            (
                "total_mutation_score",
                handlebars::to_json(format!("{:.1}", stats.mutation_score)),
            ),
            (
                "total_mutation_score_class",
                handlebars::to_json(total_mutation_score_class),
            ),
        ]);
        let writer = BufWriter::new(File::create(format!(
            "{}/index.html",
            &self.output_directory
        ))?);
        // TODO: Refactor error
        template_engine
            .render_to_write("index", &data, writer)
            .unwrap();

        self.create_assets()?;

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

#[derive(Serialize)]
struct HTMLMutant {
    outcome: String,
    text: String,
}

#[derive(Serialize)]
struct SourceLine {
    line_number: u64,
    number_of_killed_mutants: usize,
    number_of_mutants: usize,
    mutants: Vec<HTMLMutant>,
    code: String,
    css_line_class: String,
}

#[derive(Serialize)]
struct MutatedFile {
    name: String,
    link: Option<String>,
    mutation_score: String,
    alive: i32,
    killed: i32,
    error: i32,
    timeout: i32,
    bulma_mutation_score_class: String,
}

#[derive(Serialize)]
struct GeneralInfo {
    program_name: String,
    program_version: String,
    date: String,
    time: String,
}

impl GeneralInfo {
    fn new() -> Self {
        let current_time = Local::now();

        GeneralInfo {
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
