// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceRangedForSpanned as _;
use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::sourcemap::SourceMap;
use deno_core::url::Url;
use deno_resolver::npm::DenoInNpmPackageChecker;
use node_resolver::InNpmPackageChecker;
use regex::Regex;
use reporter::CoverageReporter;
use text_lines::TextLines;

use self::ignore_directives::has_file_ignore_directive;
use self::ignore_directives::lex_comments;
use self::ignore_directives::parse_next_ignore_directives;
use self::ignore_directives::parse_range_ignore_directives;
use crate::args::CliOptions;
use crate::args::FileFlags;
use crate::args::Flags;
use crate::cdp;
use crate::factory::CliFactory;
use crate::file_fetcher::TextDecodedFile;
use crate::sys::CliSys;
use crate::tools::test::is_supported_test_path;
use crate::util::text_encoding::source_map_from_code;

mod ignore_directives;
mod merge;
mod range_tree;
pub mod reporter;
mod util;
use merge::ProcessCoverage;

#[derive(Debug, Clone)]
struct BranchCoverageItem {
  line_index: usize,
  block_number: usize,
  branch_number: usize,
  taken: Option<i64>,
  is_hit: bool,
}

#[derive(Debug, Clone)]
struct FunctionCoverageItem {
  name: String,
  line_index: usize,
  execution_count: i64,
}

#[derive(Debug, Clone)]
pub struct CoverageReport {
  url: ModuleSpecifier,
  named_functions: Vec<FunctionCoverageItem>,
  branches: Vec<BranchCoverageItem>,
  /// (line_index, number_of_hits)
  found_lines: Vec<(usize, i64)>,
  output: Option<PathBuf>,
}

/// A file's media type, original source, the source actually executed at
/// runtime (transpiled for TS), and its source map.
type LoadedSource = (MediaType, String, String, Option<Vec<u8>>);

struct GenerateCoverageReportOptions<'a> {
  script_module_specifier: Url,
  script_media_type: MediaType,
  script_coverage: &'a cdp::ScriptCoverage,
  script_original_source: String,
  script_runtime_source: String,
  maybe_source_map: &'a Option<Vec<u8>>,
  output: &'a Option<PathBuf>,
}

fn generate_coverage_report(
  options: GenerateCoverageReportOptions,
) -> Result<CoverageReport, AnyError> {
  let original_comments =
    lex_comments(&options.script_original_source, options.script_media_type);
  let url = Url::parse(&options.script_coverage.url).unwrap();

  if has_file_ignore_directive(&original_comments) {
    return Ok(CoverageReport {
      url,
      named_functions: Vec::new(),
      branches: Vec::new(),
      found_lines: Vec::new(),
      output: options.output.clone(),
    });
  }

  let maybe_source_map = options
    .maybe_source_map
    .as_ref()
    .map(|source_map| SourceMap::from_slice(source_map).unwrap());
  let mut coverage_report = CoverageReport {
    url,
    named_functions: Vec::with_capacity(
      options
        .script_coverage
        .functions
        .iter()
        .filter(|f| !f.function_name.is_empty())
        .count(),
    ),
    branches: Vec::new(),
    found_lines: Vec::new(),
    output: options.output.clone(),
  };

  let original_text_lines = TextLines::new(&options.script_original_source);
  let coverage_ignore_next_directives = parse_next_ignore_directives(
    &original_comments.comments,
    &original_text_lines,
  );
  let coverage_ignore_range_directives = parse_range_ignore_directives(
    &options.script_module_specifier,
    &original_comments.comments,
    &original_text_lines,
  );

  let runtime_comments =
    lex_comments(&options.script_runtime_source, MediaType::JavaScript);
  let runtime_text_lines = TextLines::new(&options.script_runtime_source);
  for function in &options.script_coverage.functions {
    if function.function_name.is_empty() {
      continue;
    }

    // Skip functions that the source map can't trace back to the original
    // source — these are injected by the transformer (e.g. SWC's decorator
    // runtime helpers) and shouldn't show up in user-visible coverage.
    let Some(line_index) = range_to_src_line_index(
      &function.ranges[0],
      &runtime_text_lines,
      &maybe_source_map,
    ) else {
      continue;
    };

    if line_index > 0
      && coverage_ignore_next_directives.contains(&(line_index - 1_usize))
    {
      continue;
    }

    if coverage_ignore_range_directives.iter().any(|range| {
      range.start_line_index <= line_index
        && range.stop_line_index >= line_index
    }) {
      continue;
    }

    coverage_report.named_functions.push(FunctionCoverageItem {
      name: function.function_name.clone(),
      line_index,
      execution_count: function.ranges[0].count,
    });
  }

  for (block_number, function) in
    options.script_coverage.functions.iter().enumerate()
  {
    let block_hits = function.ranges[0].count;

    // For each sub-range, find the parent count: the count of the smallest
    // enclosing range. V8 ranges are nested (ranges[0] is the function body,
    // inner ranges are blocks/branches/loops). The parent count is needed to
    // correctly compute complement branches — e.g. for an if/else inside a
    // loop, the parent is the loop body, not the function.
    let mut parent_counts: Vec<i64> =
      Vec::with_capacity(function.ranges.len().saturating_sub(1));
    for range in &function.ranges[1..] {
      let mut parent_count = block_hits;
      let mut parent_size = usize::MAX;
      for candidate in &function.ranges {
        if std::ptr::eq(candidate, range) {
          continue;
        }
        if candidate.start_char_offset <= range.start_char_offset
          && candidate.end_char_offset >= range.end_char_offset
        {
          let size = candidate.end_char_offset - candidate.start_char_offset;
          if size < parent_size {
            parent_size = size;
            parent_count = candidate.count;
          }
        }
      }
      parent_counts.push(parent_count);
    }

    // Group sub-ranges by their source line to detect branch points.
    // When multiple ranges map to the same source line, they represent
    // different arms of the same branch (e.g. if/else).
    let mut branches_by_line: std::collections::BTreeMap<
      usize,
      Vec<(usize, &cdp::CoverageRange)>,
    > = std::collections::BTreeMap::new();
    for (idx, range) in function.ranges[1..].iter().enumerate() {
      // Same rationale as above: drop sub-ranges with no source mapping —
      // they belong to transformer-injected helper code, not the user's
      // source.
      let Some(line_index) =
        range_to_src_line_index(range, &runtime_text_lines, &maybe_source_map)
      else {
        continue;
      };
      branches_by_line
        .entry(line_index)
        .or_default()
        .push((idx, range));
    }

    for (line_index, ranges) in &branches_by_line {
      if *line_index > 0
        && coverage_ignore_next_directives.contains(&(line_index - 1_usize))
      {
        continue;
      }

      if coverage_ignore_range_directives.iter().any(|range| {
        range.start_line_index <= *line_index
          && range.stop_line_index >= *line_index
      }) {
        continue;
      }

      if ranges.len() == 1 {
        let (idx, range) = &ranges[0];
        let enclosing_count = parent_counts[*idx];

        if range.count > enclosing_count {
          // Range executes more often than its enclosing scope — this is a
          // loop body, not a branch arm. Report it without a complement.
          coverage_report.branches.push(BranchCoverageItem {
            line_index: *line_index,
            block_number,
            branch_number: 0,
            taken: if enclosing_count > 0 {
              Some(range.count)
            } else {
              None
            },
            is_hit: range.count > 0,
          });
        } else {
          // Single range at this line: one arm of a branch. The complement
          // (e.g. the else for an if) is implicit with count equal to the
          // enclosing range's count minus this range's count. Using the
          // enclosing (parent) range rather than the function-level count
          // gives correct complements for branches inside loops.
          let taken = if enclosing_count > 0 {
            Some(range.count)
          } else {
            None
          };
          let complement_taken = if enclosing_count > 0 {
            Some(enclosing_count - range.count)
          } else {
            None
          };
          coverage_report.branches.push(BranchCoverageItem {
            line_index: *line_index,
            block_number,
            branch_number: 0,
            taken,
            is_hit: range.count > 0,
          });
          coverage_report.branches.push(BranchCoverageItem {
            line_index: *line_index,
            block_number,
            branch_number: 1,
            taken: complement_taken,
            is_hit: complement_taken.is_some_and(|c| c > 0),
          });
        }
      } else {
        // Multiple ranges at the same line: these are explicit branch arms
        // (e.g. both if and else blocks reported by V8).
        for (branch_number, (_, range)) in ranges.iter().enumerate() {
          let taken = if block_hits > 0 {
            Some(range.count)
          } else {
            None
          };
          coverage_report.branches.push(BranchCoverageItem {
            line_index: *line_index,
            block_number,
            branch_number,
            taken,
            is_hit: range.count > 0,
          });
        }
      }
    }
  }

  // TODO(caspervonb): collect uncovered ranges on the lines so that we can highlight specific
  // parts of a line in color (word diff style) instead of the entire line.
  let mut line_counts = Vec::with_capacity(runtime_text_lines.lines_count());
  for line_index in 0..runtime_text_lines.lines_count() {
    let (line_start_byte_offset, line_end_byte_offset) =
      runtime_text_lines.line_range(line_index);
    let line_start_char_offset =
      runtime_text_lines.char_index(line_start_byte_offset);
    let line_end_char_offset =
      runtime_text_lines.char_index(line_end_byte_offset);
    let ignore = runtime_comments.comments.iter().any(|comment| {
      comment.range.start <= line_start_byte_offset
        && comment.range.end >= line_end_byte_offset
    }) || options.script_runtime_source
      [line_start_byte_offset..line_end_byte_offset]
      .trim()
      .is_empty();
    let mut count = 0;

    if ignore {
      count = 1;
    } else {
      // Find the count from the most specific (smallest/innermost) range that
      // fully covers this line. V8 coverage ranges are nested: ranges[0] is the
      // function body and ranges[1..] are inner blocks/branches. The innermost
      // range that covers a line gives the correct execution count for that line.
      let mut best_range_size = usize::MAX;
      for function in &options.script_coverage.functions {
        for range in &function.ranges {
          if range.start_char_offset <= line_start_char_offset
            && range.end_char_offset >= line_end_char_offset
          {
            let range_size = range.end_char_offset - range.start_char_offset;
            if range_size < best_range_size {
              best_range_size = range_size;
              count = range.count;
            }
          }
        }
      }

      // Reset the count if a zero-count range overlaps the line and reaches
      // at least one edge (start or end) of the line. A zero-count range
      // floating in the middle of a line (not reaching either edge) is
      // typically just a tiny gap between blocks (e.g. the unreachable path
      // between catch's return and finally) and should not zero out the line.
      for function in &options.script_coverage.functions {
        for range in &function.ranges {
          if range.count > 0 {
            continue;
          }

          let overlaps = range.start_char_offset < line_end_char_offset
            && range.end_char_offset > line_start_char_offset;
          let reaches_edge = range.start_char_offset <= line_start_char_offset
            || range.end_char_offset >= line_end_char_offset;
          if overlaps && reaches_edge {
            count = 0;
          }
        }
      }
    }

    line_counts.push(count);
  }

  let found_lines_coverage_filter = |(line, _): &(usize, i64)| -> bool {
    if coverage_ignore_range_directives.iter().any(|range| {
      range.start_line_index <= *line && range.stop_line_index >= *line
    }) {
      return false;
    }

    if coverage_ignore_next_directives.contains(line) {
      return false;
    }

    if *line == 0_usize {
      return true;
    }

    if coverage_ignore_next_directives.contains(&(line - 1_usize)) {
      return false;
    }

    true
  };

  coverage_report.found_lines =
    if let Some(source_map) = maybe_source_map.as_ref() {
      let script_runtime_source_lines =
        options.script_runtime_source.lines().collect::<Vec<_>>();
      let mut found_lines = line_counts
        .iter()
        .enumerate()
        .flat_map(|(index, count)| {
          // get all the mappings from this destination line to a different src line
          let mut results = source_map
            .tokens()
            .filter(|token| {
              let dst_line = token.get_dst_line() as usize;
              dst_line == index && {
                let dst_col = token.get_dst_col() as usize;
                let content = script_runtime_source_lines
                  .get(dst_line)
                  .and_then(|line| {
                    line.get(dst_col..std::cmp::min(dst_col + 2, line.len()))
                  })
                  .unwrap_or("");

                !content.is_empty()
                  && content != "/*"
                  && content != "*/"
                  && content != "//"
              }
            })
            .map(move |token| (token.get_src_line() as usize, *count))
            .collect::<Vec<_>>();
          // only keep the results that point at different src lines
          results.sort_unstable_by_key(|(index, _)| *index);
          results.dedup_by_key(|(index, _)| *index);
          results.into_iter()
        })
        .filter(found_lines_coverage_filter)
        .collect::<Vec<(usize, i64)>>();

      found_lines.sort_unstable_by_key(|(index, _)| *index);
      // combine duplicated lines - when multiple compiled JS lines map to the
      // same source line, use the maximum count rather than summing, since
      // hitting the same source line from different compiled lines doesn't mean
      // it was executed more times.
      for i in (1..found_lines.len()).rev() {
        if found_lines[i].0 == found_lines[i - 1].0 {
          found_lines[i - 1].1 =
            std::cmp::max(found_lines[i - 1].1, found_lines[i].1);
          found_lines.remove(i);
        }
      }
      found_lines
    } else {
      line_counts
        .into_iter()
        .enumerate()
        .filter(found_lines_coverage_filter)
        .collect::<Vec<(usize, i64)>>()
    };

  Ok(coverage_report)
}

/// Maps a runtime coverage range back to a line in the original source.
///
/// Returns `None` when the source map exists but has no mapping for this
/// position. That happens for code injected by the transformer (e.g. SWC's
/// decorator runtime helpers like `applyDecs2203R`) — the bytes exist in
/// the runtime JS but didn't come from the user's source, so callers
/// should drop these coverage entries rather than reporting them as if
/// they were the user's own code.
fn range_to_src_line_index(
  range: &cdp::CoverageRange,
  text_lines: &TextLines,
  maybe_source_map: &Option<SourceMap>,
) -> Option<usize> {
  let source_lc = text_lines.line_and_column_index(
    text_lines.byte_index_from_char_index(range.start_char_offset),
  );
  if let Some(source_map) = maybe_source_map.as_ref() {
    source_map
      .lookup_token(source_lc.line_index as u32, source_lc.column_index as u32)
      .map(|token| token.get_src_line() as usize)
  } else {
    Some(source_lc.line_index)
  }
}

fn collect_coverages(
  cli_options: &CliOptions,
  files: FileFlags,
  initial_cwd: &Path,
) -> Result<Vec<cdp::ScriptCoverage>, AnyError> {
  let mut coverages: Vec<cdp::ScriptCoverage> = Vec::new();
  let file_patterns = FilePatterns {
    base: initial_cwd.to_path_buf(),
    include: Some({
      if files.include.is_empty() {
        PathOrPatternSet::new(vec![PathOrPattern::Path(
          initial_cwd.to_path_buf(),
        )])
      } else {
        PathOrPatternSet::from_include_relative_path_or_patterns(
          initial_cwd,
          &files.include,
        )?
      }
    }),
    exclude: PathOrPatternSet::new(vec![]),
  };
  let file_paths = FileCollector::new(|e| {
    e.path.extension().map(|ext| ext == "json").unwrap_or(false)
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&CliSys::default(), &file_patterns);

  let coverage_patterns = FilePatterns {
    base: initial_cwd.to_path_buf(),
    include: None,
    exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
      initial_cwd,
      &files.ignore,
    )
    .context("Invalid ignore pattern.")?,
  };

  for file_path in file_paths {
    let new_coverage = fs::read_to_string(file_path.as_path())
      .map_err(AnyError::from)
      .and_then(|json| {
        serde_json::from_str::<cdp::ScriptCoverage>(&json)
          .map_err(AnyError::from)
      })
      .with_context(|| format!("Failed reading '{}'", file_path.display()))?;
    let url = Url::parse(&new_coverage.url)?;
    if coverage_patterns.matches_specifier(&url) {
      coverages.push(new_coverage);
    }
  }

  coverages.sort_by_key(|k| k.url.clone());

  Ok(coverages)
}

/// Walks the file system for source files matching the `--include` globs that
/// did not produce a coverage record (i.e. were never loaded during the run).
/// These are reported as 0% covered.
fn collect_uncovered_files(
  include_patterns: &FilePatterns,
  cli_options: &CliOptions,
  initial_cwd: &Path,
  covered_specifiers: &HashSet<ModuleSpecifier>,
) -> Vec<ModuleSpecifier> {
  let paths = FileCollector::new(|e| {
    matches!(
      e.path.extension().and_then(|ext| ext.to_str()),
      Some("ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs")
    )
  })
  .ignore_git_folder()
  .ignore_node_modules()
  .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
  .collect_file_patterns(&CliSys::default(), include_patterns);

  paths
    .into_iter()
    // Resolve through the same function used to build `covered_specifiers`
    // (see `cover_files`) so the two sides share one canonical form; otherwise
    // a loaded file could slip past the dedup below and be re-synthesized as a
    // 0% duplicate.
    .filter_map(|path| {
      deno_path_util::resolve_url_or_path(&path.to_string_lossy(), initial_cwd)
        .ok()
    })
    .filter(|url| !covered_specifiers.contains(url))
    .collect()
}

fn filter_coverages(
  coverages: Vec<cdp::ScriptCoverage>,
  include_patterns: Option<&FilePatterns>,
  exclude: Vec<String>,
  in_npm_pkg_checker: &DenoInNpmPackageChecker,
  link_dir_urls: &[String],
) -> Result<Vec<cdp::ScriptCoverage>, AnyError> {
  let exclude: Vec<Regex> = exclude
    .iter()
    .map(|e| {
      Regex::new(e)
        .with_context(|| format!("Invalid --exclude regular expression: {e}"))
    })
    .collect::<Result<_, _>>()?;

  // Matches virtual file paths for doc testing
  // e.g. file:///path/to/mod.ts$23-29.ts
  let doc_test_re =
    Regex::new(r"\$\d+-\d+\.(js|mjs|cjs|jsx|ts|mts|cts|tsx)$").unwrap();

  let coverages = coverages
    .into_iter()
    .filter(|e| {
      let is_internal = e.url.starts_with("ext:")
        || e.url.starts_with("data:")
        || e.url.starts_with("blob:")
        || e.url.ends_with("__anonymous__")
        || e.url.ends_with("$deno$test.mjs")
        || e.url.contains("/$deno$stdin.")
        || e.url.ends_with(".snap")
        || is_supported_test_path(Path::new(e.url.as_str()))
        || doc_test_re.is_match(e.url.as_str())
        // Exclude packages brought in via the "links" (formerly "patch")
        // feature. These are third-party dependencies replacing a registry
        // version, so they shouldn't show up in the user's coverage report,
        // matching how npm packages are excluded above.
        || link_dir_urls.iter().any(|dir| e.url.starts_with(dir))
        || Url::parse(&e.url)
          .ok()
          .map(|url| in_npm_pkg_checker.in_npm_package(&url))
          .unwrap_or(false);

      // When `--include` globs are provided, keep only files matching them.
      // Otherwise preserve the historical default of reporting file: urls
      // (remote modules were excluded by the old `^file:` default include).
      let is_included = match include_patterns {
        Some(patterns) => Url::parse(&e.url)
          .ok()
          .map(|url| patterns.matches_specifier(&url))
          .unwrap_or(false),
        None => e.url.starts_with("file:"),
      };
      let is_excluded = exclude.iter().any(|p| p.is_match(&e.url));

      is_included && !is_excluded && !is_internal
    })
    .collect::<Vec<cdp::ScriptCoverage>>();

  Ok(coverages)
}

/// Builds a coverage report for a source file that was never loaded during the
/// run. Line coverage is produced by running an empty V8 coverage through the
/// normal pipeline (so every executable line is reported as missed, reusing the
/// same comment/blank-line and source-map handling as covered files), while
/// functions and branches are counted from the source AST and marked uncovered.
fn synthesize_uncovered_report(
  module_specifier: &ModuleSpecifier,
  media_type: MediaType,
  original_source: &str,
  runtime_source: String,
  maybe_source_map: &Option<Vec<u8>>,
  output: &Option<PathBuf>,
) -> Result<Option<CoverageReport>, AnyError> {
  let empty_coverage = cdp::ScriptCoverage {
    script_id: String::new(),
    url: module_specifier.to_string(),
    functions: Vec::new(),
  };
  let mut coverage_report =
    generate_coverage_report(GenerateCoverageReportOptions {
      script_module_specifier: module_specifier.clone(),
      script_media_type: media_type,
      script_coverage: &empty_coverage,
      script_original_source: original_source.to_string(),
      script_runtime_source: runtime_source,
      maybe_source_map,
      output,
    })?;

  // The file was never loaded, so nothing executed: every executable line is
  // missed (count 0). In the no-source-map (plain JS) path the line pipeline
  // marks comment/blank lines as count 1 (ignored), which would otherwise count
  // them as covered and inflate the line % of a file that never ran. Drop them
  // so only executable lines remain. For the source-map (TS) path these lines
  // were already excluded, so this is a no-op there.
  coverage_report.found_lines.retain(|(_, count)| *count == 0);

  // Nothing executable (all comments/blank, or a coverage-ignore-file
  // directive): skip the file entirely, matching the covered path.
  if coverage_report.found_lines.is_empty() {
    return Ok(None);
  }

  // V8 produced no function/branch data because the file was never loaded, so
  // recover them from the AST and mark everything uncovered. A parse failure
  // just leaves them empty (line coverage is still reported).
  if let Ok(parsed) = deno_ast::parse_program(deno_ast::ParseParams {
    specifier: module_specifier.clone(),
    text: original_source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) {
    let mut collector = UncoveredCollector {
      text_info: parsed.text_info_lazy(),
      named_functions: Vec::new(),
      branches: Vec::new(),
      block_number: 0,
    };
    parsed.program_ref().visit_with(&mut collector);
    coverage_report.named_functions = collector.named_functions;
    coverage_report.branches = collector.branches;
  }

  Ok(Some(coverage_report))
}

fn prop_name_string(key: &ast::PropName) -> Option<String> {
  match key {
    ast::PropName::Ident(i) => Some(i.sym.to_string()),
    ast::PropName::Str(s) => Some(s.value.to_atom_lossy().to_string()),
    ast::PropName::Num(n) => Some(n.value.to_string()),
    ast::PropName::BigInt(b) => Some(b.value.to_string()),
    ast::PropName::Computed(_) => None,
  }
}

/// Collects named functions and branch points from a source AST, marking them
/// all uncovered. Used to give a never-loaded file accurate Function % and
/// Branch % denominators (the percentages are 0% regardless of the exact
/// counts, since nothing executed).
///
/// This is an approximation of the function/branch set V8 would have reported
/// had the file actually loaded, not an exact reproduction of it. The per-file
/// result is always 0% so the divergence is harmless there, but it does shift
/// the aggregate "All files" counts. Known gaps versus V8: functions assigned
/// to object-literal properties or array elements, computed method/property
/// names, constructors, and default-exported anonymous functions are not
/// counted. Treat a mismatch with V8's counts as expected, not a bug.
struct UncoveredCollector<'a> {
  text_info: &'a deno_ast::SourceTextInfo,
  named_functions: Vec<FunctionCoverageItem>,
  branches: Vec<BranchCoverageItem>,
  block_number: usize,
}

impl UncoveredCollector<'_> {
  fn add_function(&mut self, name: String, pos: deno_ast::SourcePos) {
    self.named_functions.push(FunctionCoverageItem {
      name,
      line_index: self.text_info.line_index(pos),
      execution_count: 0,
    });
  }

  fn add_branch(&mut self, pos: deno_ast::SourcePos, arms: usize) {
    let line_index = self.text_info.line_index(pos);
    let block_number = self.block_number;
    self.block_number += 1;
    for branch_number in 0..arms {
      self.branches.push(BranchCoverageItem {
        line_index,
        block_number,
        branch_number,
        taken: Some(0),
        is_hit: false,
      });
    }
  }
}

impl Visit for UncoveredCollector<'_> {
  fn visit_fn_decl(&mut self, n: &ast::FnDecl) {
    self.add_function(n.ident.sym.to_string(), n.ident.start());
    n.visit_children_with(self);
  }

  fn visit_class_method(&mut self, n: &ast::ClassMethod) {
    if let Some(name) = prop_name_string(&n.key) {
      self.add_function(name, n.key.start());
    }
    n.visit_children_with(self);
  }

  fn visit_method_prop(&mut self, n: &ast::MethodProp) {
    if let Some(name) = prop_name_string(&n.key) {
      self.add_function(name, n.key.start());
    }
    n.visit_children_with(self);
  }

  fn visit_getter_prop(&mut self, n: &ast::GetterProp) {
    if let Some(name) = prop_name_string(&n.key) {
      self.add_function(name, n.key.start());
    }
    n.visit_children_with(self);
  }

  fn visit_setter_prop(&mut self, n: &ast::SetterProp) {
    if let Some(name) = prop_name_string(&n.key) {
      self.add_function(name, n.key.start());
    }
    n.visit_children_with(self);
  }

  fn visit_var_declarator(&mut self, n: &ast::VarDeclarator) {
    // `const foo = () => {}` / `const foo = function () {}` are reported by V8
    // under the inferred name `foo`.
    if let (ast::Pat::Ident(ident), Some(init)) = (&n.name, n.init.as_deref())
      && matches!(init, ast::Expr::Arrow(_) | ast::Expr::Fn(_))
    {
      self.add_function(ident.id.sym.to_string(), ident.id.start());
    }
    n.visit_children_with(self);
  }

  fn visit_if_stmt(&mut self, n: &ast::IfStmt) {
    self.add_branch(n.start(), 2);
    n.visit_children_with(self);
  }

  fn visit_cond_expr(&mut self, n: &ast::CondExpr) {
    self.add_branch(n.start(), 2);
    n.visit_children_with(self);
  }

  fn visit_bin_expr(&mut self, n: &ast::BinExpr) {
    if matches!(
      n.op,
      ast::BinaryOp::LogicalAnd
        | ast::BinaryOp::LogicalOr
        | ast::BinaryOp::NullishCoalescing
    ) {
      self.add_branch(n.start(), 2);
    }
    n.visit_children_with(self);
  }

  fn visit_switch_stmt(&mut self, n: &ast::SwitchStmt) {
    if !n.cases.is_empty() {
      self.add_branch(n.start(), n.cases.len());
    }
    n.visit_children_with(self);
  }
}

pub fn cover_files(
  flags: Arc<Flags>,
  files_include: Vec<String>,
  files_ignore: Vec<String>,
  include: Vec<String>,
  exclude: Vec<String>,
  output: Option<String>,
  reporters: &[&dyn CoverageReporter],
) -> Result<(), AnyError> {
  if files_include.is_empty() {
    return Err(anyhow!("No matching coverage profiles found"));
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let in_npm_pkg_checker = factory.in_npm_pkg_checker()?;
  let file_fetcher = factory.file_fetcher()?;
  let emitter = factory.emitter()?;
  let cjs_tracker = factory.cjs_tracker()?;

  let initial_cwd = cli_options.initial_cwd();
  // Use the first include path as the default output path.
  let coverage_root = initial_cwd.join(&files_include[0]);
  let script_coverages = collect_coverages(
    cli_options,
    FileFlags {
      include: files_include,
      ignore: files_ignore.clone(),
    },
    initial_cwd,
  )?;
  if script_coverages.is_empty() {
    return Err(anyhow!("No coverage files found"));
  }
  let link_dir_urls = cli_options
    .workspace()
    .link_folders()
    .keys()
    .map(|url| url.as_str().to_string())
    .collect::<Vec<_>>();

  // When `--include` globs are provided they select the set of source files to
  // report (matched against file paths), and any matching file that was never
  // loaded during the run is reported as 0% covered. When omitted, only files
  // loaded during the run are reported.
  let include_patterns = if include.is_empty() {
    None
  } else {
    Some(FilePatterns {
      base: initial_cwd.to_path_buf(),
      include: Some(PathOrPatternSet::from_include_relative_path_or_patterns(
        initial_cwd,
        &include,
      )?),
      exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
        initial_cwd,
        &files_ignore,
      )
      .context("Invalid ignore pattern.")?,
    })
  };

  let script_coverages = filter_coverages(
    script_coverages,
    include_patterns.as_ref(),
    exclude.clone(),
    in_npm_pkg_checker,
    &link_dir_urls,
  )?;
  if script_coverages.is_empty() {
    return Err(anyhow!("No covered files included in the report"));
  }

  let proc_coverages: Vec<_> = script_coverages
    .into_iter()
    .map(|cov| ProcessCoverage { result: vec![cov] })
    .collect();

  let script_coverages = if let Some(c) = merge::merge_processes(proc_coverages)
  {
    c.result
  } else {
    vec![]
  };

  let out_mode = match output {
    Some(ref path) => match File::create(path) {
      Ok(_) => Some(PathBuf::from(path)),
      Err(e) => {
        return Err(anyhow!("Failed to create output file: {}", e));
      }
    },
    None => None,
  };
  let get_message = |specifier: &ModuleSpecifier| -> String {
    format!(
      "Source not found for \"{}\" (was it deleted after coverage was collected?). Skipping.",
      specifier,
    )
  };

  // Loads a file's original source plus the source actually executed at
  // runtime (transpiled for TS) and its source map. Returns `None` when the
  // file can't be loaded so the caller can skip it.
  //
  // `never_loaded` distinguishes the two callers: a file that produced a
  // coverage record was loaded during the run, so its transpiled emit is in
  // the cache and a cache miss means the source changed after coverage was
  // collected (stale cache) — that file is skipped. A never-loaded file
  // (synthesized as uncovered) has no cached emit by definition, so it is
  // transpiled on demand instead.
  let load_file = |module_specifier: &ModuleSpecifier,
                   never_loaded: bool|
   -> Result<Option<LoadedSource>, AnyError> {
    let file = match file_fetcher.get_cached_source_or_local(module_specifier) {
      Ok(Some(file)) => TextDecodedFile::decode(file)?,
      Ok(None) => {
        log::warn!("{}", get_message(module_specifier));
        return Ok(None);
      }
      Err(err) => {
        log::warn!("{}: {}", get_message(module_specifier), err);
        return Ok(None);
      }
    };
    let original_source = file.source.to_string();
    // Check if file was transpiled
    let transpiled_code = match file.media_type {
      MediaType::JavaScript
      | MediaType::Unknown
      | MediaType::Css
      | MediaType::Html
      | MediaType::Markdown
      | MediaType::Sql
      | MediaType::Wasm
      | MediaType::Cjs
      | MediaType::Mjs
      | MediaType::Json
      | MediaType::Jsonc
      | MediaType::Json5 => None,
      MediaType::Dts | MediaType::Dmts | MediaType::Dcts => Some(String::new()),
      MediaType::TypeScript
      | MediaType::Jsx
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Tsx => {
        let module_kind = ModuleKind::from_is_cjs(
          cjs_tracker.is_maybe_cjs(&file.specifier, file.media_type)?,
        );
        if never_loaded {
          // No cached emit exists for a file that was never loaded, so
          // transpile it on demand.
          Some(
            emitter
              .maybe_emit_source_sync(
                &file.specifier,
                file.media_type,
                module_kind,
                &file.source,
              )?
              .to_string(),
          )
        } else {
          // The file was loaded during the run, so its transpiled emit should
          // be cached. A miss means the source was modified after coverage was
          // collected; skip it rather than mapping the recorded coverage onto a
          // freshly transpiled file that no longer matches.
          match emitter.maybe_cached_emit(
            &file.specifier,
            module_kind,
            &file.source,
          )? {
            Some(code) => Some(code),
            None => {
              log::warn!(
                "Missing transpiled source code for: \"{}\" (was it deleted after coverage was collected?). Skipping.",
                file.specifier,
              );
              return Ok(None);
            }
          }
        }
      }
      MediaType::SourceMap => {
        unreachable!()
      }
    };
    let runtime_code = match transpiled_code {
      Some(code) => code,
      None => original_source.clone(),
    };
    let source_map = source_map_from_code(runtime_code.as_bytes());
    Ok(Some((
      file.media_type,
      original_source,
      runtime_code,
      source_map,
    )))
  };

  let mut file_reports = Vec::with_capacity(script_coverages.len());
  // Resolved specifiers of files that produced a coverage record, so that
  // synthesized uncovered entries don't duplicate a file that was loaded.
  let mut covered_specifiers: HashSet<ModuleSpecifier> = HashSet::new();

  for script_coverage in script_coverages {
    let module_specifier =
      deno_path_util::resolve_url_or_path(&script_coverage.url, initial_cwd)?;
    covered_specifiers.insert(module_specifier.clone());

    let Some((media_type, original_source, runtime_code, source_map)) =
      load_file(&module_specifier, false)?
    else {
      continue;
    };

    let coverage_report =
      generate_coverage_report(GenerateCoverageReportOptions {
        script_module_specifier: module_specifier.clone(),
        script_media_type: media_type,
        script_coverage: &script_coverage,
        script_original_source: original_source.clone(),
        script_runtime_source: runtime_code,
        maybe_source_map: &source_map,
        output: &out_mode,
      })
      .with_context(|| {
        format!(
          "Failed to generate coverage report for file ({module_specifier})"
        )
      })?;

    if !coverage_report.found_lines.is_empty() {
      file_reports.push((coverage_report, original_source));
    }
  }

  // Synthesize 0% entries for source files matching `--include` that were
  // never loaded during the run (and so produced no V8 coverage record).
  if let Some(include_patterns) = include_patterns.as_ref() {
    let uncovered = collect_uncovered_files(
      include_patterns,
      cli_options,
      initial_cwd,
      &covered_specifiers,
    );
    // Reuse the regular filtering so excluded/test/npm files are dropped.
    let uncovered = filter_coverages(
      uncovered
        .into_iter()
        .map(|url| cdp::ScriptCoverage {
          script_id: String::new(),
          url: url.into(),
          functions: Vec::new(),
        })
        .collect(),
      Some(include_patterns),
      exclude,
      in_npm_pkg_checker,
      &link_dir_urls,
    )?;

    for stub in uncovered {
      let module_specifier =
        deno_path_util::resolve_url_or_path(&stub.url, initial_cwd)?;
      let Some((media_type, original_source, runtime_code, source_map)) =
        load_file(&module_specifier, true)?
      else {
        continue;
      };
      if let Some(coverage_report) = synthesize_uncovered_report(
        &module_specifier,
        media_type,
        &original_source,
        runtime_code,
        &source_map,
        &out_mode,
      )? {
        file_reports.push((coverage_report, original_source));
      }
    }
  }

  // All covered files, might have had ignore directive and we can end up
  // with no reports at this point.
  if file_reports.is_empty() {
    return Err(anyhow!("No covered files included in the report"));
  }

  for reporter in reporters {
    reporter.done(&coverage_root, &file_reports);
  }

  Ok(())
}
