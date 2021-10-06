// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::flags::Flags;
use crate::fs_util::collect_files;
use crate::module_graph::TypeLib;
use crate::proc_state::ProcState;
use crate::source_maps::SourceMapGetter;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::LocalInspectorSession;
use deno_core::ModuleSpecifier;
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
  pub start_offset: usize,
  pub end_offset: usize,
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
  Lcov,
}

fn create_reporter(
  kind: CoverageReporterKind,
) -> Box<dyn CoverageReporter + Send> {
  match kind {
    CoverageReporterKind::Pretty => Box::new(PrettyCoverageReporter::new()),
    CoverageReporterKind::Lcov => Box::new(LcovCoverageReporter::new()),
  }
}

trait CoverageReporter {
  fn report_result(
    &mut self,
    specifier: &ModuleSpecifier,
    result: &CoverageResult,
    source: &str,
  );
}

pub struct LcovCoverageReporter {}

impl LcovCoverageReporter {
  pub fn new() -> LcovCoverageReporter {
    LcovCoverageReporter {}
  }
}

impl CoverageReporter for LcovCoverageReporter {
  fn report_result(
    &mut self,
    specifier: &ModuleSpecifier,
    result: &CoverageResult,
    source: &str,
  ) {
    println!("SF:{}", specifier.to_file_path().unwrap().to_str().unwrap());

    let named_functions = result
      .functions
      .iter()
      .filter(|block| !block.function_name.is_empty())
      .collect::<Vec<&FunctionCoverage>>();

    for block in &named_functions {
      let index = source[0..block.ranges[0].start_offset].split('\n').count();

      println!("FN:{},{}", index + 1, block.function_name);
    }

    let hit_functions = named_functions
      .iter()
      .filter(|block| block.ranges[0].count > 0)
      .cloned()
      .collect::<Vec<&FunctionCoverage>>();

    for block in &hit_functions {
      println!("FNDA:{},{}", block.ranges[0].count, block.function_name);
    }

    println!("FNF:{}", named_functions.len());
    println!("FNH:{}", hit_functions.len());

    let mut branches_found = 0;
    let mut branches_hit = 0;
    for (block_number, block) in result.functions.iter().enumerate() {
      let block_hits = block.ranges[0].count;
      for (branch_number, range) in block.ranges[1..].iter().enumerate() {
        let line_index = source[0..range.start_offset].split('\n').count();

        // From https://manpages.debian.org/unstable/lcov/geninfo.1.en.html:
        //
        // Block number and branch number are gcc internal IDs for the branch. Taken is either '-'
        // if the basic block containing the branch was never executed or a number indicating how
        // often that branch was taken.
        //
        // However with the data we get from v8 coverage profiles it seems we can't actually hit
        // this as appears it won't consider any nested branches it hasn't seen but its here for
        // the sake of accuracy.
        let taken = if block_hits > 0 {
          range.count.to_string()
        } else {
          "-".to_string()
        };

        println!(
          "BRDA:{},{},{},{}",
          line_index + 1,
          block_number,
          branch_number,
          taken
        );

        branches_found += 1;
        if range.count > 0 {
          branches_hit += 1;
        }
      }
    }

    println!("BRF:{}", branches_found);
    println!("BRH:{}", branches_hit);

    let enumerated_lines = result
      .lines
      .iter()
      .enumerate()
      .collect::<Vec<(usize, &LineCoverage)>>();

    for (index, line) in &enumerated_lines {
      if line.ranges.is_empty() {
        continue;
      }

      let mut count = 0;
      for range in &line.ranges {
        count += range.count;

        if range.count == 0 {
          count = 0;
          break;
        }
      }

      println!("DA:{},{}", index + 1, count);
    }

    let lines_found = enumerated_lines
      .iter()
      .filter(|(_, line)| !line.ranges.is_empty())
      .count();

    println!("LF:{}", lines_found);

    let lines_hit = enumerated_lines
      .iter()
      .filter(|(_, line)| {
        !line.ranges.is_empty()
          && !line.ranges.iter().any(|range| range.count == 0)
      })
      .count();

    println!("LH:{}", lines_hit);

    println!("end_of_record");
  }
}

pub struct PrettyCoverageReporter {}

impl PrettyCoverageReporter {
  pub fn new() -> PrettyCoverageReporter {
    PrettyCoverageReporter {}
  }
}

const PRETTY_LINE_WIDTH: usize = 4;
const PRETTY_LINE_SEPERATOR: &str = "|";
impl CoverageReporter for PrettyCoverageReporter {
  fn report_result(
    &mut self,
    specifier: &ModuleSpecifier,
    result: &CoverageResult,
    source: &str,
  ) {
    print!("cover {} ... ", specifier);

    let enumerated_lines = result
      .lines
      .iter()
      .enumerate()
      .collect::<Vec<(usize, &LineCoverage)>>();

    let found_lines = enumerated_lines
      .iter()
      .filter(|(_, coverage)| !coverage.ranges.is_empty())
      .cloned()
      .collect::<Vec<(usize, &LineCoverage)>>();

    let missed_lines = found_lines
      .iter()
      .filter(|(_, coverage)| {
        coverage.ranges.iter().any(|range| range.count == 0)
      })
      .cloned()
      .collect::<Vec<(usize, &LineCoverage)>>();

    let line_ratio = (found_lines.len() - missed_lines.len()) as f32
      / found_lines.len() as f32;
    let line_coverage = format!(
      "{:.3}% ({}/{})",
      line_ratio * 100.0,
      found_lines.len() - missed_lines.len(),
      found_lines.len()
    );

    if line_ratio >= 0.9 {
      println!("{}", colors::green(&line_coverage));
    } else if line_ratio >= 0.75 {
      println!("{}", colors::yellow(&line_coverage));
    } else {
      println!("{}", colors::red(&line_coverage));
    }

    let mut maybe_last_index = None;
    for (index, line) in missed_lines {
      if let Some(last_index) = maybe_last_index {
        if last_index + 1 != index {
          let dash = colors::gray("-".repeat(PRETTY_LINE_WIDTH + 1));
          println!("{}{}{}", dash, colors::gray(PRETTY_LINE_SEPERATOR), dash);
        }
      }

      let slice = &source[line.start_offset..line.end_offset];

      println!(
        "{:width$} {} {}",
        index + 1,
        colors::gray(PRETTY_LINE_SEPERATOR),
        colors::red(&slice),
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

fn offset_to_line_col(source: &str, offset: usize) -> Option<(u32, u32)> {
  let mut line = 0;
  let mut col = 0;

  if let Some(slice) = source.get(0..offset) {
    for ch in slice.bytes() {
      if ch == b'\n' {
        line += 1;
        col = 0;
      } else {
        col += 1;
      }
    }

    return Some((line, col));
  }

  None
}

fn line_col_to_offset(source: &str, line: u32, col: u32) -> Option<usize> {
  let mut current_col = 0;
  let mut current_line = 0;

  for (i, ch) in source.bytes().enumerate() {
    if current_line == line && current_col == col {
      return Some(i);
    }

    if ch == b'\n' {
      current_line += 1;
      current_col = 0;
    } else {
      current_col += 1;
    }
  }

  None
}

async fn cover_script(
  program_state: ProcState,
  script: ScriptCoverage,
) -> Result<CoverageResult, AnyError> {
  let module_specifier = resolve_url_or_path(&script.url)?;
  let file = program_state
    .file_fetcher
    .fetch(&module_specifier, &mut Permissions::allow_all())
    .await?;

  let source = file.source.as_str();

  let line_offsets = {
    let mut line_offsets: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0;

    for line in source.split('\n') {
      line_offsets.push((offset, offset + line.len()));
      offset += line.len() + 1;
    }

    line_offsets
  };

  program_state
    .prepare_module_load(
      module_specifier.clone(),
      TypeLib::UnstableDenoWindow,
      Permissions::allow_all(),
      Permissions::allow_all(),
      false,
      program_state.maybe_import_map.clone(),
    )
    .await?;

  let compiled_source =
    program_state.load(module_specifier.clone(), None)?.code;

  // TODO(caspervonb): source mapping is still a bit of a mess and we should try look into avoiding
  // doing any loads at this stage of execution but it'll do for now.
  let maybe_raw_source_map = program_state.get_source_map(&script.url);
  if let Some(raw_source_map) = maybe_raw_source_map {
    let source_map = SourceMap::from_slice(&raw_source_map)?;

    // To avoid false positives we base our line ranges on the ranges of the compiled lines
    let compiled_line_offsets = {
      let mut line_offsets: Vec<(usize, usize)> = Vec::new();
      let mut offset = 0;

      for line in compiled_source.split('\n') {
        line_offsets.push((offset, offset + line.len()));
        offset += line.len() + 1;
      }

      line_offsets
    };

    // First we get the adjusted ranges of these lines
    let compiled_line_ranges = compiled_line_offsets
      .iter()
      .filter_map(|(start_offset, end_offset)| {
        // We completely ignore empty lines, they just cause trouble and can't map to anything
        // meaningful.
        let line = &compiled_source[*start_offset..*end_offset];
        if line == "\n" {
          return None;
        }

        let ranges = script
          .functions
          .iter()
          .map(|function| {
            function.ranges.iter().filter_map(|function_range| {
              if &function_range.start_offset > end_offset {
                return None;
              }

              if &function_range.end_offset < start_offset {
                return None;
              }

              Some(CoverageRange {
                start_offset: cmp::max(
                  *start_offset,
                  function_range.start_offset,
                ),
                end_offset: cmp::min(*end_offset, function_range.end_offset),
                count: function_range.count,
              })
            })
          })
          .flatten()
          .collect::<Vec<CoverageRange>>();

        Some(ranges)
      })
      .flatten()
      .collect::<Vec<CoverageRange>>();

    // Then we map those adjusted ranges from their closest tokens to their source locations.
    let mapped_line_ranges = compiled_line_ranges
      .iter()
      .map(|line_range| {
        let (start_line, start_col) =
          offset_to_line_col(&compiled_source, line_range.start_offset)
            .unwrap();

        let start_token =
          source_map.lookup_token(start_line, start_col).unwrap();

        let (end_line, end_col) =
          offset_to_line_col(&compiled_source, line_range.end_offset).unwrap();

        let end_token = source_map.lookup_token(end_line, end_col).unwrap();

        let mapped_start_offset = line_col_to_offset(
          source,
          start_token.get_src_line(),
          start_token.get_src_col(),
        )
        .unwrap();

        let mapped_end_offset = line_col_to_offset(
          source,
          end_token.get_src_line(),
          end_token.get_src_col(),
        )
        .unwrap();

        CoverageRange {
          start_offset: mapped_start_offset,
          end_offset: mapped_end_offset,
          count: line_range.count,
        }
      })
      .collect::<Vec<CoverageRange>>();

    // Then we go through the source lines and grab any ranges that apply to any given line
    // adjusting them as we go.
    let lines = line_offsets
      .iter()
      .map(|(start_offset, end_offset)| {
        let ranges = mapped_line_ranges
          .iter()
          .filter_map(|line_range| {
            if &line_range.start_offset > end_offset {
              return None;
            }

            if &line_range.end_offset < start_offset {
              return None;
            }

            Some(CoverageRange {
              start_offset: cmp::max(*start_offset, line_range.start_offset),
              end_offset: cmp::min(*end_offset, line_range.end_offset),
              count: line_range.count,
            })
          })
          .collect();

        LineCoverage {
          start_offset: *start_offset,
          end_offset: *end_offset,
          ranges,
        }
      })
      .collect();

    let functions = script
      .functions
      .iter()
      .map(|function| {
        let ranges = function
          .ranges
          .iter()
          .map(|function_range| {
            let (start_line, start_col) =
              offset_to_line_col(&compiled_source, function_range.start_offset)
                .unwrap();

            let start_token =
              source_map.lookup_token(start_line, start_col).unwrap();

            let mapped_start_offset = line_col_to_offset(
              source,
              start_token.get_src_line(),
              start_token.get_src_col(),
            )
            .unwrap();

            let (end_line, end_col) =
              offset_to_line_col(&compiled_source, function_range.end_offset)
                .unwrap();

            let end_token = source_map.lookup_token(end_line, end_col).unwrap();

            let mapped_end_offset = line_col_to_offset(
              source,
              end_token.get_src_line(),
              end_token.get_src_col(),
            )
            .unwrap();

            CoverageRange {
              start_offset: mapped_start_offset,
              end_offset: mapped_end_offset,
              count: function_range.count,
            }
          })
          .collect();

        FunctionCoverage {
          ranges,
          is_block_coverage: function.is_block_coverage,
          function_name: function.function_name.clone(),
        }
      })
      .collect::<Vec<FunctionCoverage>>();

    return Ok(CoverageResult { lines, functions });
  }

  let functions = script.functions.clone();

  let lines = line_offsets
    .iter()
    .map(|(start_offset, end_offset)| {
      let line = &source[*start_offset..*end_offset];
      if line == "\n" {
        return LineCoverage {
          start_offset: *start_offset,
          end_offset: *end_offset,
          ranges: Vec::new(),
        };
      }

      let ranges = script
        .functions
        .iter()
        .map(|function| {
          function.ranges.iter().filter_map(|function_range| {
            if &function_range.start_offset > end_offset {
              return None;
            }

            if &function_range.end_offset < start_offset {
              return None;
            }

            Some(CoverageRange {
              start_offset: cmp::max(
                *start_offset,
                function_range.start_offset,
              ),
              end_offset: cmp::min(*end_offset, function_range.end_offset),
              count: function_range.count,
            })
          })
        })
        .flatten()
        .collect();

      LineCoverage {
        start_offset: *start_offset,
        end_offset: *end_offset,
        ranges,
      }
    })
    .collect();

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
  let ps = ProcState::build(flags).await?;

  let script_coverages = collect_script_coverages(files, ignore)?;
  let script_coverages =
    filter_script_coverages(script_coverages, include, exclude);

  let reporter_kind = if lcov {
    CoverageReporterKind::Lcov
  } else {
    CoverageReporterKind::Pretty
  };

  let mut reporter = create_reporter(reporter_kind);

  for script_coverage in script_coverages {
    let result = cover_script(ps.clone(), script_coverage.clone()).await?;

    let module_specifier = resolve_url_or_path(&script_coverage.url)?;
    let file = ps
      .file_fetcher
      .fetch(&module_specifier, &mut Permissions::allow_all())
      .await?;

    reporter.report_result(&module_specifier, &result, &file.source);
  }

  Ok(())
}
