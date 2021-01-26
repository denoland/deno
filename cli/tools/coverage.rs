// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::TokenOrComment;
use crate::colors;
use crate::media_type::MediaType;
use crate::module_graph::TypeLib;
use crate::program_state::ProgramState;
use crate::source_maps::SourceMapGetter;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_runtime::inspector::InspectorSession;
use deno_runtime::permissions::Permissions;
use serde::Deserialize;
use serde::Serialize;
use sourcemap::SourceMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use swc_common::Span;
use uuid::Uuid;

pub struct CoverageCollector {
  pub dir: PathBuf,
  session: Box<InspectorSession>,
}

impl CoverageCollector {
  pub fn new(dir: PathBuf, session: Box<InspectorSession>) -> Self {
    Self { dir, session }
  }

  pub async fn start_collecting(&mut self) -> Result<(), AnyError> {
    self.session.post_message("Debugger.enable", None).await?;

    self.session.post_message("Profiler.enable", None).await?;

    self
      .session
      .post_message(
        "Profiler.startPreciseCoverage",
        Some(json!({"callCount": true, "detailed": true})),
      )
      .await?;

    Ok(())
  }

  pub async fn stop_collecting(&mut self) -> Result<(), AnyError> {
    let result = self
      .session
      .post_message("Profiler.takePreciseCoverage", None)
      .await?;

    let take_coverage_result: TakePreciseCoverageResult =
      serde_json::from_value(result)?;

    fs::create_dir_all(&self.dir)?;

    let script_coverages = take_coverage_result.result;
    for script_coverage in script_coverages {
      let filename = format!("{}.json", Uuid::new_v4());
      let json = serde_json::to_string(&script_coverage)?;
      fs::write(self.dir.join(filename), &json)?;
    }

    self
      .session
      .post_message("Profiler.stopPreciseCoverage", None)
      .await?;

    self.session.post_message("Profiler.disable", None).await?;
    self.session.post_message("Debugger.disable", None).await?;

    Ok(())
  }
}

// TODO(caspervonb) all of these structs can and should be made private, possibly moved to
// inspector::protocol.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
  pub start_offset: usize,
  pub end_offset: usize,
  pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageResult {
  result: Vec<ScriptCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetScriptSourceResult {
  pub script_source: String,
  pub bytecode: Option<String>,
}

pub struct PrettyCoverageReporter {
  quiet: bool,
}

// TODO(caspervonb) add support for lcov output (see geninfo(1) for format spec).
impl PrettyCoverageReporter {
  pub fn new(quiet: bool) -> PrettyCoverageReporter {
    PrettyCoverageReporter { quiet }
  }

  pub fn visit_coverage(
    &mut self,
    script_coverage: &ScriptCoverage,
    script_source: &str,
    maybe_source_map: Option<Vec<u8>>,
    maybe_original_source: Option<String>,
  ) {
    let maybe_source_map = if let Some(source_map) = maybe_source_map {
      Some(SourceMap::from_slice(&source_map).unwrap())
    } else {
      None
    };

    let mut ignored_spans: Vec<Span> = Vec::new();
    for item in ast::lex("", script_source, &MediaType::JavaScript) {
      if let TokenOrComment::Token(_) = item.inner {
        continue;
      }

      ignored_spans.push(item.span);
    }

    let lines = script_source.split('\n').collect::<Vec<_>>();
    let mut covered_lines: Vec<usize> = Vec::new();
    let mut uncovered_lines: Vec<usize> = Vec::new();

    let mut line_start_offset = 0;
    for (index, line) in lines.iter().enumerate() {
      let line_end_offset = line_start_offset + line.len();

      let mut count = 0;

      let ignore = ignored_spans.iter().any(|span| {
        (span.lo.0 as usize) <= line_start_offset
          && (span.hi.0 as usize) >= line_end_offset
      });

      if ignore {
        covered_lines.push(index);
        continue;
      }

      // Count the hits of ranges that include the entire line which will always be at-least one
      // as long as the code has been evaluated.
      for function in &script_coverage.functions {
        for range in &function.ranges {
          if range.start_offset <= line_start_offset
            && range.end_offset >= line_end_offset
          {
            count += range.count;
          }
        }
      }

      // Reset the count if any block intersects with the current line has a count of
      // zero.
      //
      // We check for intersection instead of inclusion here because a block may be anywhere
      // inside a line.
      for function in &script_coverage.functions {
        for range in &function.ranges {
          if range.count > 0 {
            continue;
          }

          if (range.start_offset < line_start_offset
            && range.end_offset > line_start_offset)
            || (range.start_offset < line_end_offset
              && range.end_offset > line_end_offset)
          {
            count = 0;
          }
        }
      }

      if count > 0 {
        covered_lines.push(index);
      } else {
        uncovered_lines.push(index);
      }

      line_start_offset += line.len() + 1;
    }

    if !self.quiet {
      print!("cover {} ... ", script_coverage.url);

      let line_coverage_ratio = covered_lines.len() as f32 / lines.len() as f32;
      let line_coverage = format!(
        "{:.3}% ({}/{})",
        line_coverage_ratio * 100.0,
        covered_lines.len(),
        lines.len()
      );

      if line_coverage_ratio >= 0.9 {
        println!("{}", colors::green(&line_coverage));
      } else if line_coverage_ratio >= 0.75 {
        println!("{}", colors::yellow(&line_coverage));
      } else {
        println!("{}", colors::red(&line_coverage));
      }

      let output_lines =
        if let Some(original_source) = maybe_original_source.as_ref() {
          original_source.split("\n").collect::<Vec<_>>()
        } else {
          lines
        };

      let mut last_line = None;
      for line_index in uncovered_lines {
        const WIDTH: usize = 4;
        const SEPERATOR: &str = "|";

        let line_index = if let Some(source_map) = maybe_source_map.as_ref() {
          source_map
            .lookup_token(line_index as u32, 0)
            .unwrap()
            .get_src_line() as usize
        } else {
          line_index
        };

        // Put a horizontal separator between disjoint runs of lines
        if let Some(last_line) = last_line {
          if last_line + 1 != line_index {
            let dash = colors::gray(&"-".repeat(WIDTH + 1));
            println!("{}{}{}", dash, colors::gray(SEPERATOR), dash);
          }
        }

        println!(
          "{:width$} {} {}",
          line_index + 1,
          colors::gray(SEPERATOR),
          colors::red(&output_lines[line_index]),
          width = WIDTH
        );

        last_line = Some(line_index);
      }
    }
  }
}

fn collect_coverages(dir: &PathBuf) -> Result<Vec<ScriptCoverage>, AnyError> {
  let mut coverages: Vec<ScriptCoverage> = Vec::new();

  let entries = fs::read_dir(dir)?;
  for entry in entries {
    let json = fs::read_to_string(entry.unwrap().path())?;
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

fn filter_coverages(
  coverages: Vec<ScriptCoverage>,
  exclude: Vec<Url>,
) -> Vec<ScriptCoverage> {
  coverages
    .into_iter()
    .filter(|e| {
      if let Ok(url) = Url::parse(&e.url) {
        if url.path().ends_with("__anonymous__") {
          return false;
        }

        for module_url in &exclude {
          if &url == module_url {
            return false;
          }
        }

        if let Ok(path) = url.to_file_path() {
          for module_url in &exclude {
            if let Ok(module_path) = module_url.to_file_path() {
              if path.starts_with(module_path.parent().unwrap()) {
                return true;
              }
            }
          }
        }
      }

      false
    })
    .collect::<Vec<ScriptCoverage>>()
}

pub async fn report_coverages(
  program_state: Arc<ProgramState>,
  dir: &PathBuf,
  quiet: bool,
  exclude: Vec<Url>,
) -> Result<(), AnyError> {
  let coverages = collect_coverages(dir)?;
  let coverages = filter_coverages(coverages, exclude);

  let mut coverage_reporter = PrettyCoverageReporter::new(quiet);
  for script_coverage in coverages {
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&script_coverage.url)?;
    program_state
      .prepare_module_load(
        module_specifier.clone(),
        TypeLib::UnstableDenoWindow,
        Permissions::allow_all(),
        false,
        program_state.maybe_import_map.clone(),
      )
      .await?;

    let module_source = program_state.load(module_specifier.clone(), None)?;
    let script_source = &module_source.code;

    let maybe_source_map = program_state.get_source_map(&script_coverage.url);
    let maybe_cached_source = program_state
      .file_fetcher
      .get_source(&module_specifier)
      .map(|f| f.source);

    coverage_reporter.visit_coverage(
      &script_coverage,
      &script_source,
      maybe_source_map,
      maybe_cached_source,
    );
  }

  Ok(())
}
