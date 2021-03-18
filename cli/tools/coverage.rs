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
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_runtime::inspector::InspectorSession;
use deno_runtime::permissions::Permissions;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use sourcemap::SourceMap;
use std::fs;
use std::path::PathBuf;
use swc_common::Span;
use uuid::Uuid;

pub struct CoverageCollector {
  pub dir: PathBuf,
  session: Box<InspectorSession>,
}

struct CoverageValues {
  hit: f32,
  found: f32,
}

pub struct TotalVisitCoverage {
  lines: Option<CoverageValues>,
  functions: Option<CoverageValues>,
  branches: Option<CoverageValues>,
}

pub struct TotalCoverages {
  lines: CoverageValues,
  functions: CoverageValues,
  branches: CoverageValues,
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

#[derive(Debug, Serialize, Deserialize)]
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

pub enum CoverageReporterKind {
  Pretty,
  Lcov,
}

fn create_reporter(
  kind: CoverageReporterKind,
) -> Box<dyn CoverageReporter + Send> {
  match kind {
    CoverageReporterKind::Lcov => Box::new(LcovCoverageReporter::new()),
    CoverageReporterKind::Pretty => Box::new(PrettyCoverageReporter::new()),
  }
}

pub trait CoverageReporter {
  fn visit_coverage(
    &mut self,
    script_coverage: &ScriptCoverage,
    script_source: &str,
    maybe_source_map: Option<Vec<u8>>,
    maybe_original_source: Option<String>,
  ) -> TotalVisitCoverage;

  fn done(&mut self);
}

pub struct LcovCoverageReporter {}

impl LcovCoverageReporter {
  pub fn new() -> LcovCoverageReporter {
    LcovCoverageReporter {}
  }
}

impl CoverageReporter for LcovCoverageReporter {
  fn visit_coverage(
    &mut self,
    script_coverage: &ScriptCoverage,
    script_source: &str,
    maybe_source_map: Option<Vec<u8>>,
    _maybe_original_source: Option<String>,
  ) -> TotalVisitCoverage {
    // TODO(caspervonb) cleanup and reduce duplication between reporters, pre-compute line coverage
    // elsewhere.
    let maybe_source_map = if let Some(source_map) = maybe_source_map {
      Some(SourceMap::from_slice(&source_map).unwrap())
    } else {
      None
    };

    let url = Url::parse(&script_coverage.url).unwrap();
    let file_path = url.to_file_path().unwrap();
    println!("SF:{}", file_path.to_str().unwrap());

    let mut functions_found = 0;
    for function in &script_coverage.functions {
      if function.function_name.is_empty() {
        continue;
      }

      let source_line = script_source[0..function.ranges[0].start_offset]
        .split('\n')
        .count();

      let line_index = if let Some(source_map) = maybe_source_map.as_ref() {
        source_map
          .tokens()
          .find(|token| token.get_dst_line() as usize == source_line)
          .map(|token| token.get_src_line() as usize)
          .unwrap_or(0)
      } else {
        source_line
      };

      let function_name = &function.function_name;

      println!("FN:{},{}", line_index + 1, function_name);

      functions_found += 1;
    }

    let mut functions_hit = 0;
    for function in &script_coverage.functions {
      if function.function_name.is_empty() {
        continue;
      }

      let execution_count = function.ranges[0].count;
      let function_name = &function.function_name;

      println!("FNDA:{},{}", execution_count, function_name);

      if execution_count != 0 {
        functions_hit += 1;
      }
    }

    println!("FNF:{}", functions_found);
    println!("FNH:{}", functions_hit);

    let mut branches_found = 0;
    let mut branches_hit = 0;
    for (block_number, function) in script_coverage.functions.iter().enumerate()
    {
      let block_hits = function.ranges[0].count;
      for (branch_number, range) in function.ranges[1..].iter().enumerate() {
        let source_line =
          script_source[0..range.start_offset].split('\n').count();

        let line_index = if let Some(source_map) = maybe_source_map.as_ref() {
          source_map
            .tokens()
            .find(|token| token.get_dst_line() as usize == source_line)
            .map(|token| token.get_src_line() as usize)
            .unwrap_or(0)
        } else {
          source_line
        };

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

    let lines = script_source.split('\n').collect::<Vec<_>>();
    let line_offsets = {
      let mut offsets: Vec<(usize, usize)> = Vec::new();
      let mut index = 0;

      for line in &lines {
        offsets.push((index, index + line.len() + 1));
        index += line.len() + 1;
      }

      offsets
    };

    let line_counts = line_offsets
      .iter()
      .map(|(line_start_offset, line_end_offset)| {
        let mut count = 0;

        // Count the hits of ranges that include the entire line which will always be at-least one
        // as long as the code has been evaluated.
        for function in &script_coverage.functions {
          for range in &function.ranges {
            if range.start_offset <= *line_start_offset
              && range.end_offset >= *line_end_offset
            {
              count += range.count;
            }
          }
        }

        // We reset the count if any block with a zero count overlaps with the line range.
        for function in &script_coverage.functions {
          for range in &function.ranges {
            if range.count > 0 {
              continue;
            }

            let overlaps = std::cmp::max(line_end_offset, &range.end_offset)
              - std::cmp::min(line_start_offset, &range.start_offset)
              < (line_end_offset - line_start_offset)
                + (range.end_offset - range.start_offset);

            if overlaps {
              count = 0;
            }
          }
        }

        count
      })
      .collect::<Vec<usize>>();

    let found_lines = if let Some(source_map) = maybe_source_map.as_ref() {
      let mut found_lines = line_counts
        .iter()
        .enumerate()
        .map(|(index, count)| {
          source_map
            .tokens()
            .filter(move |token| token.get_dst_line() as usize == index)
            .map(move |token| (token.get_src_line() as usize, *count))
        })
        .flatten()
        .collect::<Vec<(usize, usize)>>();

      found_lines.sort_unstable_by_key(|(index, _)| *index);
      found_lines.dedup_by_key(|(index, _)| *index);
      found_lines
    } else {
      line_counts
        .iter()
        .enumerate()
        .map(|(index, count)| (index, *count))
        .collect::<Vec<(usize, usize)>>()
    };

    for (index, count) in &found_lines {
      println!("DA:{},{}", index + 1, count);
    }

    let lines_hit = found_lines.iter().filter(|(_, count)| *count != 0).count();

    println!("LH:{}", lines_hit);

    let lines_found = found_lines.len();
    println!("LF:{}", lines_found);

    println!("end_of_record");

    return TotalVisitCoverage {
      lines: Some(CoverageValues {
        hit: lines_hit as f32,
        found: lines_found as f32,
      }),
      branches: Some(CoverageValues {
        hit: branches_hit as f32,
        found: branches_found as f32,
      }),
      functions: Some(CoverageValues {
        hit: functions_hit as f32,
        found: functions_found as f32,
      }),
    };
  }

  fn done(&mut self) {}
}

pub struct PrettyCoverageReporter {}

impl PrettyCoverageReporter {
  pub fn new() -> PrettyCoverageReporter {
    PrettyCoverageReporter {}
  }
}

impl CoverageReporter for PrettyCoverageReporter {
  fn visit_coverage(
    &mut self,
    script_coverage: &ScriptCoverage,
    script_source: &str,
    maybe_source_map: Option<Vec<u8>>,
    maybe_original_source: Option<String>,
  ) -> TotalVisitCoverage {
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

    let line_offsets = {
      let mut offsets: Vec<(usize, usize)> = Vec::new();
      let mut index = 0;

      for line in &lines {
        offsets.push((index, index + line.len() + 1));
        index += line.len() + 1;
      }

      offsets
    };

    // TODO(caspervonb): collect uncovered ranges on the lines so that we can highlight specific
    // parts of a line in color (word diff style) instead of the entire line.
    let line_counts = line_offsets
      .iter()
      .enumerate()
      .map(|(index, (line_start_offset, line_end_offset))| {
        let ignore = ignored_spans.iter().any(|span| {
          (span.lo.0 as usize) <= *line_start_offset
            && (span.hi.0 as usize) >= *line_end_offset
        });

        if ignore {
          return (index, 1);
        }

        let mut count = 0;

        // Count the hits of ranges that include the entire line which will always be at-least one
        // as long as the code has been evaluated.
        for function in &script_coverage.functions {
          for range in &function.ranges {
            if range.start_offset <= *line_start_offset
              && range.end_offset >= *line_end_offset
            {
              count += range.count;
            }
          }
        }

        // We reset the count if any block with a zero count overlaps with the line range.
        for function in &script_coverage.functions {
          for range in &function.ranges {
            if range.count > 0 {
              continue;
            }

            let overlaps = std::cmp::max(line_end_offset, &range.end_offset)
              - std::cmp::min(line_start_offset, &range.start_offset)
              < (line_end_offset - line_start_offset)
                + (range.end_offset - range.start_offset);

            if overlaps {
              count = 0;
            }
          }
        }

        (index, count)
      })
      .collect::<Vec<(usize, usize)>>();

    let lines = if let Some(original_source) = maybe_original_source.as_ref() {
      original_source.split('\n').collect::<Vec<_>>()
    } else {
      lines
    };

    let line_counts = if let Some(source_map) = maybe_source_map.as_ref() {
      let mut line_counts = line_counts
        .iter()
        .map(|(index, count)| {
          source_map
            .tokens()
            .filter(move |token| token.get_dst_line() as usize == *index)
            .map(move |token| (token.get_src_line() as usize, *count))
        })
        .flatten()
        .collect::<Vec<(usize, usize)>>();

      line_counts.sort_unstable_by_key(|(index, _)| *index);
      line_counts.dedup_by_key(|(index, _)| *index);

      line_counts
    } else {
      line_counts
    };

    print!("cover {} ... ", script_coverage.url);

    let hit_lines = line_counts
      .iter()
      .filter(|(_, count)| *count != 0)
      .map(|(index, _)| *index);

    let missed_lines = line_counts
      .iter()
      .filter(|(_, count)| *count == 0)
      .map(|(index, _)| *index);

    let lines_found = line_counts.len();
    let lines_hit = hit_lines.count();
    let line_ratio = lines_hit as f32 / lines_found as f32;

    let line_coverage =
      format!("{:.3}% ({}/{})", line_ratio * 100.0, lines_hit, lines_found,);

    if line_ratio >= 0.9 {
      println!("{}", colors::green(&line_coverage));
    } else if line_ratio >= 0.75 {
      println!("{}", colors::yellow(&line_coverage));
    } else {
      println!("{}", colors::red(&line_coverage));
    }

    let mut last_line = None;
    for line_index in missed_lines {
      const WIDTH: usize = 4;
      const SEPERATOR: &str = "|";

      // Put a horizontal separator between disjoint runs of lines
      if let Some(last_line) = last_line {
        if last_line + 1 != line_index {
          let dash = colors::gray("-".repeat(WIDTH + 1));
          println!("{}{}{}", dash, colors::gray(SEPERATOR), dash);
        }
      }

      println!(
        "{:width$} {} {}",
        line_index + 1,
        colors::gray(SEPERATOR),
        colors::red(&lines[line_index]),
        width = WIDTH
      );

      last_line = Some(line_index);
    }
    return TotalVisitCoverage {
      lines: Some(CoverageValues {
        hit: lines_hit as f32,
        found: lines_found as f32,
      }),
      branches: None,
      functions: None,
    };
  }

  fn done(&mut self) {}
}

fn collect_coverages(
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

fn filter_coverages(
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
        || e.url.ends_with("$deno$test.ts");

      let is_included = include.iter().any(|p| p.is_match(&e.url));
      let is_excluded = exclude.iter().any(|p| p.is_match(&e.url));

      (include.is_empty() || is_included) && !is_excluded && !is_internal
    })
    .collect::<Vec<ScriptCoverage>>()
}

fn coverage_threshold_error(coverage_type: &str, actual: f32, expected: f32) {
  let coverage = format!(
    "The coverage threshold for {} ({:.2}%) not met: {:.2}%",
    coverage_type, expected, actual
  );
  println!("{}", colors::red(&coverage));
}

pub async fn cover_files(
  flags: Flags,
  files: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  include: Vec<String>,
  exclude: Vec<String>,
  check: bool,
  lines: f32,
  functions: f32,
  branches: f32,
  lcov: bool,
) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags).await?;
  let mut total_coverages = TotalCoverages {
    lines: CoverageValues {
      hit: 0.0,
      found: 0.0,
    },
    functions: CoverageValues {
      hit: 0.0,
      found: 0.0,
    },
    branches: CoverageValues {
      hit: 0.0,
      found: 0.0,
    },
  };

  let script_coverages = collect_coverages(files, ignore)?;
  let script_coverages = filter_coverages(script_coverages, include, exclude);

  let reporter_kind = if lcov {
    CoverageReporterKind::Lcov
  } else {
    CoverageReporterKind::Pretty
  };

  let mut reporter = create_reporter(reporter_kind);

  for script_coverage in script_coverages {
    let module_specifier =
      deno_core::resolve_url_or_path(&script_coverage.url)?;
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

    let total_coverage = reporter.visit_coverage(
      &script_coverage,
      &script_source,
      maybe_source_map,
      maybe_cached_source,
    );

    if total_coverage.lines.is_some() {
      let coverage_lines = total_coverage.lines.unwrap();
      total_coverages.lines.hit += coverage_lines.hit;
      total_coverages.lines.found += coverage_lines.found;
    }

    if total_coverage.functions.is_some() {
      let coverage_functions = total_coverage.functions.unwrap();
      total_coverages.functions.hit += coverage_functions.hit;
      total_coverages.functions.found += coverage_functions.found;
    }

    if total_coverage.branches.is_some() {
      let coverage_branches = total_coverage.branches.unwrap();
      total_coverages.branches.hit += coverage_branches.hit;
      total_coverages.branches.found += coverage_branches.found;
    }
  }

  let total_coverage_lines = if total_coverages.lines.found != 0.0 {
    let lines = total_coverages.lines;
    Some((lines.hit * 100.0) / lines.found)
  } else {
    None
  };

  let total_coverage_functions = if total_coverages.functions.found != 0.0 {
    let functions = total_coverages.functions;
    Some((functions.hit * 100.0) / functions.found)
  } else {
    None
  };

  let total_coverage_branches = if total_coverages.branches.found != 0.0 {
    let branches = total_coverages.branches;
    Some((branches.hit * 100.0) / branches.found)
  } else {
    None
  };

  if check {
    let mut threshold_met = true;
    if total_coverage_lines.is_some() && total_coverage_lines.unwrap() < lines {
      coverage_threshold_error("lines", total_coverage_lines.unwrap(), lines);
      threshold_met = false;
    };

    if total_coverage_functions.is_some()
      && total_coverage_functions.unwrap() < functions
    {
      coverage_threshold_error(
        "functions",
        total_coverage_functions.unwrap(),
        functions,
      );
      threshold_met = false;
    };

    if total_coverage_branches.is_some()
      && total_coverage_branches.unwrap() < branches
    {
      coverage_threshold_error(
        "branches",
        total_coverage_branches.unwrap(),
        branches,
      );
      threshold_met = false;
    };

    if !threshold_met {
      return Err(custom_error(
        "CoverageThreshold",
        colors::red(format!("The coverage threshold is not met",)).to_string(),
      ));
    }
  }

  reporter.done();

  Ok(())
}
