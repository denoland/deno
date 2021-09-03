// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::TokenOrComment;
use crate::colors;
use crate::flags::Flags;
use crate::fs_util::collect_files;
use crate::media_type::MediaType;
use crate::module_graph::TypeLib;
use crate::program_state::ProgramState;
use crate::source_maps::SourceMapGetter;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::LocalInspectorSession;
use deno_runtime::permissions::Permissions;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use sourcemap::SourceMap;
use std::cmp;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use swc_common::Span;
use uuid::Uuid;

// TODO(caspervonb) These structs are specific to the inspector protocol and should be refactored
// into a reusable module.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CoverageRange {
  pub start_offset: usize,
  pub end_offset: usize,
  pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FunctionCoverage {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LineCoverage {
  pub ranges: Vec<CoverageRange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScriptCoverage {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CoverageResult {
  pub lines: Vec<LineCoverage>,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPreciseCoverageParameters {
  pub call_count: bool,
  pub detailed: bool,
  pub allow_triggered_updates: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPreciseCoverageReturnObject {
  pub timestamp: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageReturnObject {
  pub result: Vec<ScriptCoverage>,
  pub timestamp: f64,
}

pub struct CoverageCollector {
  pub dir: PathBuf,
  session: LocalInspectorSession,
}

impl CoverageCollector {
  pub fn new(dir: PathBuf, session: LocalInspectorSession) -> Self {
    Self { dir, session }
  }

  async fn enable_debugger(&mut self) -> Result<(), AnyError> {
    self.session.post_message("Debugger.enable", None).await?;

    Ok(())
  }

  async fn enable_profiler(&mut self) -> Result<(), AnyError> {
    self.session.post_message("Profiler.enable", None).await?;

    Ok(())
  }

  async fn disable_debugger(&mut self) -> Result<(), AnyError> {
    self.session.post_message("Debugger.disable", None).await?;

    Ok(())
  }

  async fn disable_profiler(&mut self) -> Result<(), AnyError> {
    self.session.post_message("Profiler.disable", None).await?;

    Ok(())
  }

  async fn start_precise_coverage(
    &mut self,
    parameters: StartPreciseCoverageParameters,
  ) -> Result<StartPreciseCoverageReturnObject, AnyError> {
    let parameters_value = serde_json::to_value(parameters)?;
    let return_value = self
      .session
      .post_message("Profiler.startPreciseCoverage", Some(parameters_value))
      .await?;

    let return_object = serde_json::from_value(return_value)?;

    Ok(return_object)
  }

  async fn take_precise_coverage(
    &mut self,
  ) -> Result<TakePreciseCoverageReturnObject, AnyError> {
    let return_value = self
      .session
      .post_message("Profiler.takePreciseCoverage", None)
      .await?;

    let return_object = serde_json::from_value(return_value)?;

    Ok(return_object)
  }

  pub async fn start_collecting(&mut self) -> Result<(), AnyError> {
    self.enable_debugger().await?;
    self.enable_profiler().await?;
    self
      .start_precise_coverage(StartPreciseCoverageParameters {
        call_count: true,
        detailed: true,
        allow_triggered_updates: false,
      })
      .await?;

    Ok(())
  }

  pub async fn stop_collecting(&mut self) -> Result<(), AnyError> {
    fs::create_dir_all(&self.dir)?;

    let script_coverages = self.take_precise_coverage().await?.result;
    for script_coverage in script_coverages {
      let filename = format!("{}.json", Uuid::new_v4());
      let filepath = self.dir.join(filename);

      let mut out = BufWriter::new(File::create(filepath)?);
      serde_json::to_writer_pretty(&mut out, &script_coverage)?;
      out.write_all(b"\n")?;
      out.flush()?;
    }

    self.disable_debugger().await?;
    self.disable_profiler().await?;

    Ok(())
  }
}

enum CoverageReporterKind {
  Pretty,
}

fn create_reporter(
  kind: CoverageReporterKind,
) -> Box<dyn CoverageReporter + Send> {
  match kind {
    CoverageReporterKind::Pretty => Box::new(PrettyCoverageReporter::new()),
  }
}

trait CoverageReporter {
  fn report_result(&mut self, result: &CoverageResult, source: &str);
}

struct PrettyCoverageReporter {}

impl PrettyCoverageReporter {
  pub fn new() -> PrettyCoverageReporter {
    PrettyCoverageReporter {}
  }
}

const PRETTY_LINE_WIDTH: usize = 4;
const PRETTY_LINE_SEPERATOR: &str = "|";

impl CoverageReporter for PrettyCoverageReporter {
  fn report_result(&mut self, result: &CoverageResult, source: &str) {
    // Collect a vector of enumerated lines:
    // These are all the lines that were seen by the runtime during coverage collection.
    let enumerated_lines = result
      .lines
      .iter()
      .enumerate()
      .filter(|(_, coverage)| coverage.ranges.len() > 0)
      .collect::<Vec<(usize, &LineCoverage)>>();

    // Collect a vector of missed lines:
    // These are all the lines that were seen by the runtime but not executed
    // which is reported by a zero count in a range.
    let missed_lines = enumerated_lines
      .iter()
      .filter(|(_, coverage)| {
        coverage.ranges.iter().any(|range| range.count == 0)
      })
      .cloned()
      .collect::<Vec<(usize, &LineCoverage)>>();

    let line_ratio = (enumerated_lines.len() - missed_lines.len()) as f32 / enumerated_lines.len() as f32;
    let line_coverage = format!(
      "{:.3}% ({}/{})",
      line_ratio * 100.0,
      enumerated_lines.len() - missed_lines.len(),
      enumerated_lines.len()
    );

    if line_ratio >= 0.9 {
      println!("{}", colors::green(&line_coverage));
    } else if line_ratio >= 0.75 {
      println!("{}", colors::yellow(&line_coverage));
    } else {
      println!("{}", colors::red(&line_coverage));
    }

    let mut maybe_last_index = None;
    for (index, coverage) in missed_lines {
      if let Some(last_index) = maybe_last_index {
        if last_index + 1 != index {
          let dash = colors::gray("-".repeat(PRETTY_LINE_WIDTH + 1));
          println!("{}{}{}", dash, colors::gray(PRETTY_LINE_SEPERATOR), dash);
        }
      }

      println!("{} {:?}", index, coverage);
      let range = &coverage.ranges[0];
      let line = &source[range.start_offset..range.end_offset];

      println!(
        "{:width$} {} {}",
        index + 1,
        colors::gray(PRETTY_LINE_SEPERATOR),
        colors::red(&line),
        width = PRETTY_LINE_WIDTH,
      );

      maybe_last_index = Some(index);
    }
  }
}

fn collect_script_coverages(
  files: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
) -> Result<Vec<ScriptCoverage>, AnyError> {
  let mut coverages: Vec<ScriptCoverage> = Vec::new();
  let file_paths = collect_files(&files, &ignore, |file_path| {
    file_path.extension().map_or(false, |ext| ext == "json")
  })?;

  for file_path in file_paths {
    let json = fs::read_to_string(file_path.as_path())?;
    let new_coverage: ScriptCoverage = serde_json::from_str(&json)?;

    let existing_coverage =
      coverages.iter_mut().find(|x| x.url == new_coverage.url);

    if let Some(existing_coverage) = existing_coverage {
      for new_function in new_coverage.functions {
        let existing_function = existing_coverage
          .functions
          .iter_mut()
          .find(|x| x.function_name == new_function.function_name);

        if let Some(existing_function) = existing_function {
          for new_range in new_function.ranges {
            let existing_range =
              existing_function.ranges.iter_mut().find(|x| {
                x.start_offset == new_range.start_offset
                  && x.end_offset == new_range.end_offset
              });

            if let Some(existing_range) = existing_range {
              existing_range.count += new_range.count;
            } else {
              existing_function.ranges.push(new_range);
            }
          }
        } else {
          existing_coverage.functions.push(new_function);
        }
      }
    } else {
      coverages.push(new_coverage);
    }
  }

  coverages.sort_by_key(|k| k.url.clone());

  Ok(coverages)
}

fn filter_script_coverages(
  coverages: Vec<ScriptCoverage>,
  include: Vec<String>,
  exclude: Vec<String>,
) -> Vec<ScriptCoverage> {
  let include: Vec<Regex> =
    include.iter().map(|e| Regex::new(e).unwrap()).collect();

  let exclude: Vec<Regex> =
    exclude.iter().map(|e| Regex::new(e).unwrap()).collect();

  coverages
    .into_iter()
    .filter(|e| {
      let is_internal = e.url.starts_with("deno:")
        || e.url.ends_with("__anonymous__")
        || e.url.ends_with("$deno$test.js");

      let is_included = include.iter().any(|p| p.is_match(&e.url));
      let is_excluded = exclude.iter().any(|p| p.is_match(&e.url));

      (include.is_empty() || is_included) && !is_excluded && !is_internal
    })
    .collect::<Vec<ScriptCoverage>>()
}

async fn cover_script(
  program_state: Arc<ProgramState>,
  script: ScriptCoverage,
) -> Result<CoverageResult, AnyError> {
  let file = program_state
    .file_fetcher
    .fetch(
      &resolve_url_or_path(&script.url).unwrap(),
      &mut Permissions::allow_all(),
    )
    .await?;

  // TODO(caspervonb): remap input coverage
  let line_offsets = {
    let mut line_offsets: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0;

    for line in file.source.split_inclusive('\n') {
      line_offsets.push((offset, offset + line.len()));
      offset += line.len();
    }

    line_offsets
  };

  let lines = line_offsets
    .iter()
    .map(|(start_offset, end_offset)| {
      let ranges = script
        .functions
        .iter()
        .map(|function| {
          function.ranges.iter().filter_map(|function_range| {
            // If the line starts after the function ends:
            // there is no overlap and this range is applicable.
            if start_offset > &function_range.end_offset {
              return None;
            }

            Some(CoverageRange {
              start_offset: cmp::max(
                *start_offset,
                function_range.start_offset,
              ),
              end_offset: cmp::min(
                  *end_offset,
                  function_range.end_offset
              ),
              count: function_range.count,
            })
          })
        })
        .flatten()
        .collect();

      LineCoverage { ranges }
    })
    .collect();

  let functions = script.functions.clone();

  Ok(CoverageResult { lines, functions })
}

pub async fn cover_scripts(
  flags: Flags,
  files: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  include: Vec<String>,
  exclude: Vec<String>,
  lcov: bool,
) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags).await?;

  let script_coverages = collect_script_coverages(files, ignore)?;
  let script_coverages =
    filter_script_coverages(script_coverages, include, exclude);

  // TODO(caspervonb): reimplement lcov
  let reporter_kind = CoverageReporterKind::Pretty;
  let mut reporter = create_reporter(reporter_kind);

  for script_coverage in script_coverages {
    let result =
      cover_script(program_state.clone(), script_coverage.clone()).await?;
    let file = program_state
      .file_fetcher
      .fetch(
        &resolve_url_or_path(&script_coverage.url).unwrap(),
        &mut Permissions::allow_all(),
      )
      .await?;

    reporter.report_result(&result, &file.source);
  }

  Ok(())
}
