// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::CoverageFlags;
use crate::args::FileFlags;
use crate::args::Flags;
use crate::cdp;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::tools::fmt::format_json;
use crate::tools::test::is_supported_test_path;
use crate::util::text_encoding::source_map_from_code;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::sourcemap::SourceMap;
use deno_core::url::Url;
use deno_core::LocalInspectorSession;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use text_lines::TextLines;
use uuid::Uuid;

mod merge;
mod range_tree;
mod reporter;
mod util;
use merge::ProcessCoverage;

pub struct CoverageCollector {
  pub dir: PathBuf,
  session: LocalInspectorSession,
}

#[async_trait::async_trait(?Send)]
impl crate::worker::CoverageCollector for CoverageCollector {
  async fn start_collecting(&mut self) -> Result<(), AnyError> {
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

  async fn stop_collecting(&mut self) -> Result<(), AnyError> {
    fs::create_dir_all(&self.dir)?;

    let script_coverages = self.take_precise_coverage().await?.result;
    for script_coverage in script_coverages {
      // Filter out internal and http/https JS files and eval'd scripts
      // from being included in coverage reports
      if script_coverage.url.starts_with("ext:")
        || script_coverage.url.starts_with("[ext:")
        || script_coverage.url.starts_with("http:")
        || script_coverage.url.starts_with("https:")
        || script_coverage.url.starts_with("node:")
        || script_coverage.url.is_empty()
      {
        continue;
      }

      let filename = format!("{}.json", Uuid::new_v4());
      let filepath = self.dir.join(filename);

      let mut out = BufWriter::new(File::create(&filepath)?);
      let coverage = serde_json::to_string(&script_coverage)?;
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

  async fn enable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.enable", None)
      .await?;
    Ok(())
  }

  async fn enable_profiler(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Profiler.enable", None)
      .await?;
    Ok(())
  }

  async fn disable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.disable", None)
      .await?;
    Ok(())
  }

  async fn disable_profiler(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Profiler.disable", None)
      .await?;
    Ok(())
  }

  async fn start_precise_coverage(
    &mut self,
    parameters: cdp::StartPreciseCoverageArgs,
  ) -> Result<cdp::StartPreciseCoverageResponse, AnyError> {
    let return_value = self
      .session
      .post_message("Profiler.startPreciseCoverage", Some(parameters))
      .await?;

    let return_object = serde_json::from_value(return_value)?;

    Ok(return_object)
  }

  async fn take_precise_coverage(
    &mut self,
  ) -> Result<cdp::TakePreciseCoverageResponse, AnyError> {
    let return_value = self
      .session
      .post_message::<()>("Profiler.takePreciseCoverage", None)
      .await?;

    let return_object = serde_json::from_value(return_value)?;

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

fn generate_coverage_report(
  script_coverage: &cdp::ScriptCoverage,
  script_source: String,
  maybe_source_map: &Option<Vec<u8>>,
  output: &Option<PathBuf>,
) -> CoverageReport {
  let maybe_source_map = maybe_source_map
    .as_ref()
    .map(|source_map| SourceMap::from_slice(source_map).unwrap());
  let text_lines = TextLines::new(&script_source);

  let comment_ranges = deno_ast::lex(&script_source, MediaType::JavaScript)
    .into_iter()
    .filter(|item| {
      matches!(item.inner, deno_ast::TokenOrComment::Comment { .. })
    })
    .map(|item| item.range)
    .collect::<Vec<_>>();

  let url = Url::parse(&script_coverage.url).unwrap();
  let mut coverage_report = CoverageReport {
    url,
    named_functions: Vec::with_capacity(
      script_coverage
        .functions
        .iter()
        .filter(|f| !f.function_name.is_empty())
        .count(),
    ),
    branches: Vec::new(),
    found_lines: Vec::new(),
    output: output.clone(),
  };

  for function in &script_coverage.functions {
    if function.function_name.is_empty() {
      continue;
    }

    let line_index = range_to_src_line_index(
      &function.ranges[0],
      &text_lines,
      &maybe_source_map,
    );
    coverage_report.named_functions.push(FunctionCoverageItem {
      name: function.function_name.clone(),
      line_index,
      execution_count: function.ranges[0].count,
    });
  }

  for (block_number, function) in script_coverage.functions.iter().enumerate() {
    let block_hits = function.ranges[0].count;
    for (branch_number, range) in function.ranges[1..].iter().enumerate() {
      let line_index =
        range_to_src_line_index(range, &text_lines, &maybe_source_map);

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
  let mut line_counts = Vec::with_capacity(text_lines.lines_count());
  for line_index in 0..text_lines.lines_count() {
    let line_start_byte_offset = text_lines.line_start(line_index);
    let line_start_char_offset = text_lines.char_index(line_start_byte_offset);
    let line_end_byte_offset = text_lines.line_end(line_index);
    let line_end_char_offset = text_lines.char_index(line_end_byte_offset);
    let ignore = comment_ranges.iter().any(|range| {
      range.start <= line_start_byte_offset && range.end >= line_end_byte_offset
    }) || script_source
      [line_start_byte_offset..line_end_byte_offset]
      .trim()
      .is_empty();
    let mut count = 0;

    if ignore {
      count = 1;
    } else {
      // Count the hits of ranges that include the entire line which will always be at-least one
      // as long as the code has been evaluated.
      for function in &script_coverage.functions {
        for range in &function.ranges {
          if range.start_char_offset <= line_start_char_offset
            && range.end_char_offset >= line_end_char_offset
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

  coverage_report.found_lines =
    if let Some(source_map) = maybe_source_map.as_ref() {
      let mut found_lines = line_counts
        .iter()
        .enumerate()
        .flat_map(|(index, count)| {
          // get all the mappings from this destination line to a different src line
          let mut results = source_map
            .tokens()
            .filter(move |token| token.get_dst_line() as usize == index)
            .map(move |token| (token.get_src_line() as usize, *count))
            .collect::<Vec<_>>();
          // only keep the results that point at different src lines
          results.sort_unstable_by_key(|(index, _)| *index);
          results.dedup_by_key(|(index, _)| *index);
          results.into_iter()
        })
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
        .collect::<Vec<(usize, i64)>>()
    };

  coverage_report
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
  .collect_file_patterns(&deno_config::fs::RealDenoConfigFs, file_patterns)?;

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
  npm_resolver: &dyn CliNpmResolver,
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
        || e.url.ends_with("$deno$test.js")
        || e.url.ends_with(".snap")
        || is_supported_test_path(Path::new(e.url.as_str()))
        || doc_test_re.is_match(e.url.as_str())
        || Url::parse(&e.url)
          .ok()
          .map(|url| npm_resolver.in_npm_package(&url))
          .unwrap_or(false);

      let is_included = include.iter().any(|p| p.is_match(&e.url));
      let is_excluded = exclude.iter().any(|p| p.is_match(&e.url));

      (include.is_empty() || is_included) && !is_excluded && !is_internal
    })
    .collect::<Vec<cdp::ScriptCoverage>>()
}

pub async fn cover_files(
  flags: Arc<Flags>,
  coverage_flags: CoverageFlags,
) -> Result<(), AnyError> {
  if coverage_flags.files.include.is_empty() {
    return Err(generic_error("No matching coverage profiles found"));
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let file_fetcher = factory.file_fetcher()?;
  let emitter = factory.emitter()?;

  assert!(!coverage_flags.files.include.is_empty());

  // Use the first include path as the default output path.
  let coverage_root = cli_options
    .initial_cwd()
    .join(&coverage_flags.files.include[0]);
  let script_coverages = collect_coverages(
    cli_options,
    coverage_flags.files,
    cli_options.initial_cwd(),
  )?;
  if script_coverages.is_empty() {
    return Err(generic_error("No coverage files found"));
  }
  let script_coverages = filter_coverages(
    script_coverages,
    coverage_flags.include,
    coverage_flags.exclude,
    npm_resolver.as_ref(),
  );
  if script_coverages.is_empty() {
    return Err(generic_error("No covered files included in the report"));
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

  let mut reporter = reporter::create(coverage_flags.r#type);

  let out_mode = match coverage_flags.output {
    Some(ref path) => match File::create(path) {
      Ok(_) => Some(PathBuf::from(path)),
      Err(e) => {
        return Err(anyhow!("Failed to create output file: {}", e));
      }
    },
    None => None,
  };

  for script_coverage in script_coverages {
    let module_specifier = deno_core::resolve_url_or_path(
      &script_coverage.url,
      cli_options.initial_cwd(),
    )?;

    let maybe_file = if module_specifier.scheme() == "file" {
      file_fetcher.get_source(&module_specifier)
    } else {
      file_fetcher
        .fetch_cached(&module_specifier, 10)
        .with_context(|| {
          format!("Failed to fetch \"{module_specifier}\" from cache.")
        })?
    };
    let file = maybe_file.ok_or_else(|| {
      anyhow!("Failed to fetch \"{}\" from cache.
          Before generating coverage report, run `deno test --coverage` to ensure consistent state.",
          module_specifier
        )
    })?.into_text_decoded()?;

    let original_source = file.source.clone();
    // Check if file was transpiled
    let transpiled_code = match file.media_type {
      MediaType::JavaScript
      | MediaType::Unknown
      | MediaType::Cjs
      | MediaType::Mjs
      | MediaType::Json => None,
      MediaType::Dts | MediaType::Dmts | MediaType::Dcts => Some(Vec::new()),
      MediaType::TypeScript
      | MediaType::Jsx
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Tsx => {
        Some(match emitter.maybe_cached_emit(&file.specifier, &file.source) {
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
      MediaType::Wasm | MediaType::TsBuildInfo | MediaType::SourceMap => {
        unreachable!()
      }
    };
    let runtime_code: String = match transpiled_code {
      Some(code) => String::from_utf8(code)
        .with_context(|| format!("Failed decoding {}", file.specifier))?,
      None => original_source.to_string(),
    };

    let source_map = source_map_from_code(runtime_code.as_bytes());
    let coverage_report = generate_coverage_report(
      &script_coverage,
      runtime_code.as_str().to_owned(),
      &source_map,
      &out_mode,
    );

    if !coverage_report.found_lines.is_empty() {
      reporter.report(&coverage_report, &original_source)?;
    }
  }

  reporter.done(&coverage_root);

  Ok(())
}
