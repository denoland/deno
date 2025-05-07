// Copyright 2018-2025 the Deno authors. MIT license.

use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_ast::ModuleSpecifier;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::serde_json;
use deno_core::sourcemap::SourceMap;
use deno_core::url::Url;
use deno_core::LocalInspectorSession;
use deno_error::JsErrorBox;
use deno_resolver::npm::DenoInNpmPackageChecker;
use node_resolver::InNpmPackageChecker;
use regex::Regex;
use reporter::CoverageReporter;
use text_lines::TextLines;
use uuid::Uuid;

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
use crate::tools::fmt::format_json;
use crate::tools::test::is_supported_test_path;
use crate::util::text_encoding::source_map_from_code;

mod ignore_directives;
mod merge;
mod range_tree;
pub mod reporter;
mod util;
use merge::ProcessCoverage;

pub struct CoverageCollector {
  pub dir: PathBuf,
  session: LocalInspectorSession,
}

#[async_trait::async_trait(?Send)]
impl crate::worker::CoverageCollector for CoverageCollector {
  async fn start_collecting(&mut self) -> Result<(), CoreError> {
    self.enable_debugger().await?;
    self.enable_profiler().await?;
    self
      .start_precise_coverage(cdp::StartPreciseCoverageArgs {
        call_count: true,
        detailed: true,
        allow_triggered_updates: false,
      })
      .await?;

    Ok(())
  }

  async fn stop_collecting(&mut self) -> Result<(), CoreError> {
    fs::create_dir_all(&self.dir)?;

    let script_coverages = self.take_precise_coverage().await?.result;
    for script_coverage in script_coverages {
      // Filter out internal and http/https JS files, eval'd scripts,
      // and scripts with invalid urls from being included in coverage reports
      if script_coverage.url.is_empty()
        || script_coverage.url.starts_with("ext:")
        || script_coverage.url.starts_with("[ext:")
        || script_coverage.url.starts_with("http:")
        || script_coverage.url.starts_with("https:")
        || script_coverage.url.starts_with("node:")
        || Url::parse(&script_coverage.url).is_err()
      {
        continue;
      }

      let filename = format!("{}.json", Uuid::new_v4());
      let filepath = self.dir.join(filename);

      let mut out = BufWriter::new(File::create(&filepath)?);
      let coverage = serde_json::to_string(&script_coverage)
        .map_err(JsErrorBox::from_err)?;
      let formatted_coverage =
        format_json(&filepath, &coverage, &Default::default())
          .ok()
          .flatten()
          .unwrap_or(coverage);

      out.write_all(formatted_coverage.as_bytes())?;
      out.flush()?;
    }

    self.disable_debugger().await?;
    self.disable_profiler().await?;

    Ok(())
  }
}

impl CoverageCollector {
  pub fn new(dir: PathBuf, session: LocalInspectorSession) -> Self {
    Self { dir, session }
  }

  async fn enable_debugger(&mut self) -> Result<(), CoreError> {
    self
      .session
      .post_message::<()>("Debugger.enable", None)
      .await?;
    Ok(())
  }

  async fn enable_profiler(&mut self) -> Result<(), CoreError> {
    self
      .session
      .post_message::<()>("Profiler.enable", None)
      .await?;
    Ok(())
  }

  async fn disable_debugger(&mut self) -> Result<(), CoreError> {
    self
      .session
      .post_message::<()>("Debugger.disable", None)
      .await?;
    Ok(())
  }

  async fn disable_profiler(&mut self) -> Result<(), CoreError> {
    self
      .session
      .post_message::<()>("Profiler.disable", None)
      .await?;
    Ok(())
  }

  async fn start_precise_coverage(
    &mut self,
    parameters: cdp::StartPreciseCoverageArgs,
  ) -> Result<cdp::StartPreciseCoverageResponse, CoreError> {
    let return_value = self
      .session
      .post_message("Profiler.startPreciseCoverage", Some(parameters))
      .await?;

    let return_object =
      serde_json::from_value(return_value).map_err(JsErrorBox::from_err)?;

    Ok(return_object)
  }

  async fn take_precise_coverage(
    &mut self,
  ) -> Result<cdp::TakePreciseCoverageResponse, CoreError> {
    let return_value = self
      .session
      .post_message::<()>("Profiler.takePreciseCoverage", None)
      .await?;

    let return_object =
      serde_json::from_value(return_value).map_err(JsErrorBox::from_err)?;

    Ok(return_object)
  }
}

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

    let line_index = range_to_src_line_index(
      &function.ranges[0],
      &runtime_text_lines,
      &maybe_source_map,
    );

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
    for (branch_number, range) in function.ranges[1..].iter().enumerate() {
      let line_index =
        range_to_src_line_index(range, &runtime_text_lines, &maybe_source_map);

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
        Some(range.count)
      } else {
        None
      };

      coverage_report.branches.push(BranchCoverageItem {
        line_index,
        block_number,
        branch_number,
        taken,
        is_hit: range.count > 0,
      })
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
      // Count the hits of ranges that include the entire line which will always be at-least one
      // as long as the code has been evaluated.
      for function in &options.script_coverage.functions {
        for range in &function.ranges {
          if range.start_char_offset <= line_start_char_offset
            && range.end_char_offset >= line_end_char_offset
          {
            count += range.count;
          }
        }
      }

      // We reset the count if any block with a zero count overlaps with the line range.
      for function in &options.script_coverage.functions {
        for range in &function.ranges {
          if range.count > 0 {
            continue;
          }

          let overlaps = range.start_char_offset < line_end_char_offset
            && range.end_char_offset > line_start_char_offset;
          if overlaps {
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
      // combine duplicated lines
      for i in (1..found_lines.len()).rev() {
        if found_lines[i].0 == found_lines[i - 1].0 {
          found_lines[i - 1].1 += found_lines[i].1;
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

fn range_to_src_line_index(
  range: &cdp::CoverageRange,
  text_lines: &TextLines,
  maybe_source_map: &Option<SourceMap>,
) -> usize {
  let source_lc = text_lines.line_and_column_index(
    text_lines.byte_index_from_char_index(range.start_char_offset),
  );
  if let Some(source_map) = maybe_source_map.as_ref() {
    source_map
      .lookup_token(source_lc.line_index as u32, source_lc.column_index as u32)
      .map(|token| token.get_src_line() as usize)
      .unwrap_or(0)
  } else {
    source_lc.line_index
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
  .collect_file_patterns(&CliSys::default(), file_patterns);

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

fn filter_coverages(
  coverages: Vec<cdp::ScriptCoverage>,
  include: Vec<String>,
  exclude: Vec<String>,
  in_npm_pkg_checker: &DenoInNpmPackageChecker,
) -> Vec<cdp::ScriptCoverage> {
  let include: Vec<Regex> =
    include.iter().map(|e| Regex::new(e).unwrap()).collect();

  let exclude: Vec<Regex> =
    exclude.iter().map(|e| Regex::new(e).unwrap()).collect();

  // Matches virtual file paths for doc testing
  // e.g. file:///path/to/mod.ts$23-29.ts
  let doc_test_re =
    Regex::new(r"\$\d+-\d+\.(js|mjs|cjs|jsx|ts|mts|cts|tsx)$").unwrap();

  coverages
    .into_iter()
    .filter(|e| {
      let is_internal = e.url.starts_with("ext:")
        || e.url.ends_with("__anonymous__")
        || e.url.ends_with("$deno$test.mjs")
        || e.url.ends_with(".snap")
        || is_supported_test_path(Path::new(e.url.as_str()))
        || doc_test_re.is_match(e.url.as_str())
        || Url::parse(&e.url)
          .ok()
          .map(|url| in_npm_pkg_checker.in_npm_package(&url))
          .unwrap_or(false);

      let is_included = include.iter().any(|p| p.is_match(&e.url));
      let is_excluded = exclude.iter().any(|p| p.is_match(&e.url));

      (include.is_empty() || is_included) && !is_excluded && !is_internal
    })
    .collect::<Vec<cdp::ScriptCoverage>>()
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

  // Use the first include path as the default output path.
  let coverage_root = cli_options.initial_cwd().join(&files_include[0]);
  let script_coverages = collect_coverages(
    cli_options,
    FileFlags {
      include: files_include,
      ignore: files_ignore,
    },
    cli_options.initial_cwd(),
  )?;
  if script_coverages.is_empty() {
    return Err(anyhow!("No coverage files found"));
  }
  let script_coverages =
    filter_coverages(script_coverages, include, exclude, in_npm_pkg_checker);
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
      "Failed to fetch \"{}\" from cache. Before generating coverage report, run `deno test --coverage` to ensure consistent state.",
      specifier,
    )
  };

  let mut file_reports = Vec::with_capacity(script_coverages.len());

  for script_coverage in script_coverages {
    let module_specifier = deno_core::resolve_url_or_path(
      &script_coverage.url,
      cli_options.initial_cwd(),
    )?;

    let maybe_file_result =
      file_fetcher.get_cached_source_or_local(&module_specifier);
    let file = match maybe_file_result {
      Ok(Some(file)) => TextDecodedFile::decode(file)?,
      Ok(None) => return Err(anyhow!("{}", get_message(&module_specifier))),
      Err(err) => return Err(err).context(get_message(&module_specifier)),
    };

    let original_source = file.source.clone();
    // Check if file was transpiled
    let transpiled_code = match file.media_type {
      MediaType::JavaScript
      | MediaType::Unknown
      | MediaType::Css
      | MediaType::Html
      | MediaType::Sql
      | MediaType::Wasm
      | MediaType::Cjs
      | MediaType::Mjs
      | MediaType::Json => None,
      MediaType::Dts | MediaType::Dmts | MediaType::Dcts => Some(String::new()),
      MediaType::TypeScript
      | MediaType::Jsx
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Tsx => {
        let module_kind = ModuleKind::from_is_cjs(
          cjs_tracker.is_maybe_cjs(&file.specifier, file.media_type)?,
        );
        Some(match emitter.maybe_cached_emit(&file.specifier, module_kind, &file.source)? {
          Some(code) => code,
          None => {
            return Err(anyhow!(
              "Missing transpiled source code for: \"{}\".
              Before generating coverage report, run `deno test --coverage` to ensure consistent state.",
              file.specifier,
            ))
          }
        })
      }
      MediaType::SourceMap => {
        unreachable!()
      }
    };
    let runtime_code: String = match transpiled_code {
      Some(code) => code,
      None => original_source.to_string(),
    };

    let source_map = source_map_from_code(runtime_code.as_bytes());
    let coverage_report =
      generate_coverage_report(GenerateCoverageReportOptions {
        script_module_specifier: module_specifier.clone(),
        script_media_type: file.media_type,
        script_coverage: &script_coverage,
        script_original_source: original_source.to_string(),
        script_runtime_source: runtime_code.as_str().to_owned(),
        maybe_source_map: &source_map,
        output: &out_mode,
      })
      .with_context(|| {
        format!(
          "Failed to generate coverage report for file ({module_specifier})"
        )
      })?;

    if !coverage_report.found_lines.is_empty() {
      file_reports.push((coverage_report, original_source.to_string()));
    }
  }

  for reporter in reporters {
    reporter.done(&coverage_root, &file_reports);
  }

  Ok(())
}
