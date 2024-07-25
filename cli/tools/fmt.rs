// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! This module provides file formatting utilities using
//! [`dprint-plugin-typescript`](https://github.com/dprint/dprint-plugin-typescript).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::FmtFlags;
use crate::args::FmtOptions;
use crate::args::FmtOptionsConfig;
use crate::args::ProseWrap;
use crate::cache::Caches;
use crate::colors;
use crate::factory::CliFactory;
use crate::util::diff::diff;
use crate::util::file_watcher;
use crate::util::fs::canonicalize_path;
use crate::util::path::get_extension;
use async_trait::async_trait;
use deno_ast::ParsedSource;
use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use log::debug;
use log::info;
use log::warn;
use std::fs;
use std::io::stdin;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::cache::IncrementalCache;

/// Format JavaScript/TypeScript files.
pub async fn format(
  flags: Arc<Flags>,
  fmt_flags: FmtFlags,
) -> Result<(), AnyError> {
  if fmt_flags.is_stdin() {
    let cli_options = CliOptions::from_flags(flags)?;
    let start_dir = &cli_options.start_dir;
    let fmt_config = start_dir
      .to_fmt_config(FilePatterns::new_with_base(start_dir.dir_path()))?;
    let fmt_options = FmtOptions::resolve(fmt_config, &fmt_flags);
    return format_stdin(
      &fmt_flags,
      fmt_options,
      cli_options
        .ext_flag()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("ts"),
    );
  }

  if let Some(watch_flags) = &fmt_flags.watch {
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Fmt", !watch_flags.no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let fmt_flags = fmt_flags.clone();
        Ok(async move {
          let factory = CliFactory::from_flags(flags);
          let cli_options = factory.cli_options()?;
          let caches = factory.caches()?;
          let mut paths_with_options_batches =
            resolve_paths_with_options_batches(cli_options, &fmt_flags)?;

          for paths_with_options in &mut paths_with_options_batches {
            let _ = watcher_communicator
              .watch_paths(paths_with_options.paths.clone());
            let files = std::mem::take(&mut paths_with_options.paths);
            paths_with_options.paths = if let Some(paths) = &changed_paths {
              if fmt_flags.check {
                // check all files on any changed (https://github.com/denoland/deno/issues/12446)
                files
                  .iter()
                  .any(|path| {
                    canonicalize_path(path)
                      .map(|path| paths.contains(&path))
                      .unwrap_or(false)
                  })
                  .then_some(files)
                  .unwrap_or_else(|| [].to_vec())
              } else {
                files
                  .into_iter()
                  .filter(|path| {
                    canonicalize_path(path)
                      .map(|path| paths.contains(&path))
                      .unwrap_or(false)
                  })
                  .collect::<Vec<_>>()
              }
            } else {
              files
            };
          }

          format_files(caches, &fmt_flags, paths_with_options_batches).await?;

          Ok(())
        })
      },
    )
    .await?;
  } else {
    let factory = CliFactory::from_flags(flags);
    let cli_options = factory.cli_options()?;
    let caches = factory.caches()?;
    let paths_with_options_batches =
      resolve_paths_with_options_batches(cli_options, &fmt_flags)?;
    format_files(caches, &fmt_flags, paths_with_options_batches).await?;
  }

  Ok(())
}

struct PathsWithOptions {
  base: PathBuf,
  paths: Vec<PathBuf>,
  options: FmtOptions,
}

fn resolve_paths_with_options_batches(
  cli_options: &CliOptions,
  fmt_flags: &FmtFlags,
) -> Result<Vec<PathsWithOptions>, AnyError> {
  let members_fmt_options =
    cli_options.resolve_fmt_options_for_members(fmt_flags)?;
  let mut paths_with_options_batches =
    Vec::with_capacity(members_fmt_options.len());
  for (_ctx, member_fmt_options) in members_fmt_options {
    let files =
      collect_fmt_files(cli_options, member_fmt_options.files.clone())?;
    if !files.is_empty() {
      paths_with_options_batches.push(PathsWithOptions {
        base: member_fmt_options.files.base.clone(),
        paths: files,
        options: member_fmt_options,
      });
    }
  }
  if paths_with_options_batches.is_empty() {
    return Err(generic_error("No target files found."));
  }
  Ok(paths_with_options_batches)
}

async fn format_files(
  caches: &Arc<Caches>,
  fmt_flags: &FmtFlags,
  paths_with_options_batches: Vec<PathsWithOptions>,
) -> Result<(), AnyError> {
  let formatter: Box<dyn Formatter> = if fmt_flags.check {
    Box::new(CheckFormatter::default())
  } else {
    Box::new(RealFormatter::default())
  };
  for paths_with_options in paths_with_options_batches {
    log::debug!(
      "Formatting {} file(s) in {}",
      paths_with_options.paths.len(),
      paths_with_options.base.display()
    );
    let fmt_options = paths_with_options.options;
    let paths = paths_with_options.paths;
    let incremental_cache = Arc::new(IncrementalCache::new(
      caches.fmt_incremental_cache_db(),
      &fmt_options.options,
      &paths,
    ));
    formatter
      .handle_files(paths, fmt_options.options, incremental_cache.clone())
      .await?;
    incremental_cache.wait_completion().await;
  }

  formatter.finish()
}

fn collect_fmt_files(
  cli_options: &CliOptions,
  files: FilePatterns,
) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(|e| is_supported_ext_fmt(e.path))
    .ignore_git_folder()
    .ignore_node_modules()
    .set_vendor_folder(cli_options.vendor_dir_path().map(ToOwned::to_owned))
    .collect_file_patterns(&deno_config::fs::RealDenoConfigFs, files)
}

/// Formats markdown (using <https://github.com/dprint/dprint-plugin-markdown>) and its code blocks
/// (ts/tsx, js/jsx).
fn format_markdown(
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let markdown_config = get_resolved_markdown_config(fmt_options);
  dprint_plugin_markdown::format_text(
    file_text,
    &markdown_config,
    move |tag, text, line_width| {
      let tag = tag.to_lowercase();
      if matches!(
        tag.as_str(),
        "ts"
          | "tsx"
          | "js"
          | "jsx"
          | "cjs"
          | "cts"
          | "mjs"
          | "mts"
          | "javascript"
          | "typescript"
          | "json"
          | "jsonc"
          | "yml"
          | "yaml"
      ) {
        // It's important to tell dprint proper file extension, otherwise
        // it might parse the file twice.
        let extension = match tag.as_str() {
          "javascript" => "js",
          "typescript" => "ts",
          rest => rest,
        };

        let fake_filename =
          PathBuf::from(format!("deno_fmt_stdin.{extension}"));
        match extension {
          "json" | "jsonc" => {
            let mut json_config = get_resolved_json_config(fmt_options);
            json_config.line_width = line_width;
            dprint_plugin_json::format_text(&fake_filename, text, &json_config)
          }
          "yml" | "yaml" => pretty_yaml::format_text(
            text,
            &get_resolved_yaml_config(fmt_options),
          )
          .map(Some)
          .map_err(AnyError::from),
          _ => {
            let mut codeblock_config =
              get_resolved_typescript_config(fmt_options);
            codeblock_config.line_width = line_width;
            dprint_plugin_typescript::format_text(
              &fake_filename,
              text.to_string(),
              &codeblock_config,
            )
          }
        }
      } else {
        Ok(None)
      }
    },
  )
}

/// Formats JSON and JSONC using the rules provided by .deno()
/// of configuration builder of <https://github.com/dprint/dprint-plugin-json>.
/// See <https://github.com/dprint/dprint-plugin-json/blob/cfa1052dbfa0b54eb3d814318034cdc514c813d7/src/configuration/builder.rs#L87> for configuration.
pub fn format_json(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let config = get_resolved_json_config(fmt_options);
  dprint_plugin_json::format_text(file_path, file_text, &config)
}

/// Formats a single TS, TSX, JS, JSX, JSONC, JSON, MD, or IPYNB file.
pub fn format_file(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let ext = get_extension(file_path).unwrap_or_default();

  match ext.as_str() {
    "md" | "mkd" | "mkdn" | "mdwn" | "mdown" | "markdown" => {
      format_markdown(file_text, fmt_options)
    }
    "json" | "jsonc" => format_json(file_path, file_text, fmt_options),
    "yml" | "yaml" => pretty_yaml::format_text(
      file_text,
      &get_resolved_yaml_config(fmt_options),
    )
    .map(Some)
    .map_err(AnyError::from),
    "ipynb" => dprint_plugin_jupyter::format_text(
      file_text,
      |file_path: &Path, file_text: String| {
        format_file(file_path, &file_text, fmt_options)
      },
    ),
    _ => {
      let config = get_resolved_typescript_config(fmt_options);
      dprint_plugin_typescript::format_text(
        file_path,
        file_text.to_string(),
        &config,
      )
    }
  }
}

pub fn format_parsed_source(
  parsed_source: &ParsedSource,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  dprint_plugin_typescript::format_parsed_source(
    parsed_source,
    &get_resolved_typescript_config(fmt_options),
  )
}

#[async_trait]
trait Formatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    incremental_cache: Arc<IncrementalCache>,
  ) -> Result<(), AnyError>;

  fn finish(&self) -> Result<(), AnyError>;
}

#[derive(Default)]
struct CheckFormatter {
  not_formatted_files_count: Arc<AtomicUsize>,
  checked_files_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Formatter for CheckFormatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    incremental_cache: Arc<IncrementalCache>,
  ) -> Result<(), AnyError> {
    // prevent threads outputting at the same time
    let output_lock = Arc::new(Mutex::new(0));

    run_parallelized(paths, {
      let not_formatted_files_count = self.not_formatted_files_count.clone();
      let checked_files_count = self.checked_files_count.clone();
      move |file_path| {
        checked_files_count.fetch_add(1, Ordering::Relaxed);
        let file_text = read_file_contents(&file_path)?.text;

        // skip checking the file if we know it's formatted
        if incremental_cache.is_file_same(&file_path, &file_text) {
          return Ok(());
        }

        match format_file(&file_path, &file_text, &fmt_options) {
          Ok(Some(formatted_text)) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            let diff = diff(&file_text, &formatted_text);
            info!("");
            info!("{} {}:", colors::bold("from"), file_path.display());
            info!("{}", diff);
          }
          Ok(None) => {
            // When checking formatting, only update the incremental cache when
            // the file is the same since we don't bother checking for stable
            // formatting here. Additionally, ensure this is done during check
            // so that CIs that cache the DENO_DIR will get the benefit of
            // incremental formatting
            incremental_cache.update_file(&file_path, &file_text);
          }
          Err(e) => {
            not_formatted_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            warn!("Error checking: {}", file_path.to_string_lossy());
            warn!(
              "{}",
              format!("{e}")
                .split('\n')
                .map(|l| {
                  if l.trim().is_empty() {
                    String::new()
                  } else {
                    format!("  {l}")
                  }
                })
                .collect::<Vec<_>>()
                .join("\n")
            );
          }
        }
        Ok(())
      }
    })
    .await?;

    Ok(())
  }

  fn finish(&self) -> Result<(), AnyError> {
    let not_formatted_files_count =
      self.not_formatted_files_count.load(Ordering::Relaxed);
    let checked_files_count = self.checked_files_count.load(Ordering::Relaxed);
    let checked_files_str =
      format!("{} {}", checked_files_count, files_str(checked_files_count));
    if not_formatted_files_count == 0 {
      info!("Checked {}", checked_files_str);
      Ok(())
    } else {
      let not_formatted_files_str = files_str(not_formatted_files_count);
      Err(generic_error(format!(
        "Found {not_formatted_files_count} not formatted {not_formatted_files_str} in {checked_files_str}",
      )))
    }
  }
}

#[derive(Default)]
struct RealFormatter {
  formatted_files_count: Arc<AtomicUsize>,
  checked_files_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Formatter for RealFormatter {
  async fn handle_files(
    &self,
    paths: Vec<PathBuf>,
    fmt_options: FmtOptionsConfig,
    incremental_cache: Arc<IncrementalCache>,
  ) -> Result<(), AnyError> {
    let output_lock = Arc::new(Mutex::new(0)); // prevent threads outputting at the same time

    run_parallelized(paths, {
      let formatted_files_count = self.formatted_files_count.clone();
      let checked_files_count = self.checked_files_count.clone();
      move |file_path| {
        checked_files_count.fetch_add(1, Ordering::Relaxed);
        let file_contents = read_file_contents(&file_path)?;

        // skip formatting the file if we know it's formatted
        if incremental_cache.is_file_same(&file_path, &file_contents.text) {
          return Ok(());
        }

        match format_ensure_stable(
          &file_path,
          &file_contents.text,
          &fmt_options,
          format_file,
        ) {
          Ok(Some(formatted_text)) => {
            incremental_cache.update_file(&file_path, &formatted_text);
            write_file_contents(
              &file_path,
              FileContents {
                had_bom: file_contents.had_bom,
                text: formatted_text,
              },
            )?;
            formatted_files_count.fetch_add(1, Ordering::Relaxed);
            let _g = output_lock.lock();
            info!("{}", file_path.to_string_lossy());
          }
          Ok(None) => {
            incremental_cache.update_file(&file_path, &file_contents.text);
          }
          Err(e) => {
            let _g = output_lock.lock();
            log::error!("Error formatting: {}", file_path.to_string_lossy());
            log::error!("   {e}");
          }
        }
        Ok(())
      }
    })
    .await?;
    Ok(())
  }

  fn finish(&self) -> Result<(), AnyError> {
    let formatted_files_count =
      self.formatted_files_count.load(Ordering::Relaxed);
    debug!(
      "Formatted {} {}",
      formatted_files_count,
      files_str(formatted_files_count),
    );

    let checked_files_count = self.checked_files_count.load(Ordering::Relaxed);
    info!(
      "Checked {} {}",
      checked_files_count,
      files_str(checked_files_count)
    );
    Ok(())
  }
}

/// When storing any formatted text in the incremental cache, we want
/// to ensure that anything stored when formatted will have itself as
/// the output as well. This is to prevent "double format" issues where
/// a user formats their code locally and it fails on the CI afterwards.
fn format_ensure_stable(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
  fmt_func: impl Fn(
    &Path,
    &str,
    &FmtOptionsConfig,
  ) -> Result<Option<String>, AnyError>,
) -> Result<Option<String>, AnyError> {
  let formatted_text = fmt_func(file_path, file_text, fmt_options)?;

  match formatted_text {
    Some(mut current_text) => {
      let mut count = 0;
      loop {
        match fmt_func(file_path, &current_text, fmt_options) {
          Ok(Some(next_pass_text)) => {
            // just in case
            if next_pass_text == current_text {
              return Ok(Some(next_pass_text));
            }
            current_text = next_pass_text;
          }
          Ok(None) => {
            return Ok(Some(current_text));
          }
          Err(err) => {
            panic!(
              concat!(
                "Formatting succeeded initially, but failed when ensuring a ",
                "stable format. This indicates a bug in the formatter where ",
                "the text it produces is not syntactically correct. As a temporary ",
                "workaround you can ignore this file ({}).\n\n{:#}"
              ),
              file_path.display(),
              err,
            )
          }
        }
        count += 1;
        if count == 5 {
          panic!(
            concat!(
              "Formatting not stable. Bailed after {} tries. This indicates a bug ",
              "in the formatter where it formats the file ({}) differently each time. As a ",
              "temporary workaround you can ignore this file."
            ),
            count,
            file_path.display(),
          )
        }
      }
    }
    None => Ok(None),
  }
}

/// Format stdin and write result to stdout.
/// Treats input as set by `--ext` flag.
/// Compatible with `--check` flag.
fn format_stdin(
  fmt_flags: &FmtFlags,
  fmt_options: FmtOptions,
  ext: &str,
) -> Result<(), AnyError> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    bail!("Failed to read from stdin");
  }
  let file_path = PathBuf::from(format!("_stdin.{ext}"));
  let formatted_text = format_file(&file_path, &source, &fmt_options.options)?;
  if fmt_flags.check {
    #[allow(clippy::print_stdout)]
    if formatted_text.is_some() {
      println!("Not formatted stdin");
    }
  } else {
    stdout().write_all(formatted_text.unwrap_or(source).as_bytes())?;
  }
  Ok(())
}

fn files_str(len: usize) -> &'static str {
  if len <= 1 {
    "file"
  } else {
    "files"
  }
}

fn get_resolved_typescript_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_typescript::configuration::Configuration {
  let mut builder =
    dprint_plugin_typescript::configuration::ConfigurationBuilder::new();
  builder.deno();

  if let Some(use_tabs) = options.use_tabs {
    builder.use_tabs(use_tabs);
  }

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(indent_width) = options.indent_width {
    builder.indent_width(indent_width);
  }

  if let Some(single_quote) = options.single_quote {
    if single_quote {
      builder.quote_style(
        dprint_plugin_typescript::configuration::QuoteStyle::PreferSingle,
      );
    }
  }

  if let Some(semi_colons) = options.semi_colons {
    builder.semi_colons(match semi_colons {
      true => dprint_plugin_typescript::configuration::SemiColons::Prefer,
      false => dprint_plugin_typescript::configuration::SemiColons::Asi,
    });
  }

  builder.build()
}

fn get_resolved_markdown_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_markdown::configuration::Configuration {
  let mut builder =
    dprint_plugin_markdown::configuration::ConfigurationBuilder::new();

  builder.deno();

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(prose_wrap) = options.prose_wrap {
    builder.text_wrap(match prose_wrap {
      ProseWrap::Always => {
        dprint_plugin_markdown::configuration::TextWrap::Always
      }
      ProseWrap::Never => {
        dprint_plugin_markdown::configuration::TextWrap::Never
      }
      ProseWrap::Preserve => {
        dprint_plugin_markdown::configuration::TextWrap::Maintain
      }
    });
  }

  builder.build()
}

fn get_resolved_json_config(
  options: &FmtOptionsConfig,
) -> dprint_plugin_json::configuration::Configuration {
  let mut builder =
    dprint_plugin_json::configuration::ConfigurationBuilder::new();

  builder.deno();

  if let Some(use_tabs) = options.use_tabs {
    builder.use_tabs(use_tabs);
  }

  if let Some(line_width) = options.line_width {
    builder.line_width(line_width);
  }

  if let Some(indent_width) = options.indent_width {
    builder.indent_width(indent_width);
  }

  builder.build()
}

fn get_resolved_yaml_config(
  options: &FmtOptionsConfig,
) -> pretty_yaml::config::FormatOptions {
  use pretty_yaml::config::*;

  let mut layout_options = LayoutOptions::default();
  if let Some(use_tabs) = options.use_tabs {
    layout_options.use_tabs = use_tabs;
  }
  if let Some(line_width) = options.line_width {
    layout_options.print_width = line_width as usize;
  }
  if let Some(indent_width) = options.indent_width {
    layout_options.indent_width = indent_width as usize;
  }

  let mut language_options = LanguageOptions::default();
  match options.single_quote {
    Some(true) => language_options.quotes = Quotes::PreferSingle,
    Some(false) => language_options.quotes = Quotes::PreferDouble,
    None => {}
  }

  FormatOptions {
    layout: layout_options,
    language: language_options,
  }
}

struct FileContents {
  text: String,
  had_bom: bool,
}

fn read_file_contents(file_path: &Path) -> Result<FileContents, AnyError> {
  let file_bytes = fs::read(file_path)
    .with_context(|| format!("Error reading {}", file_path.display()))?;
  let had_bom = file_bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
  // will have the BOM stripped
  let text = deno_graph::source::decode_owned_file_source(file_bytes)
    .with_context(|| {
      anyhow!("{} is not a valid UTF-8 file", file_path.display())
    })?;

  Ok(FileContents { text, had_bom })
}

fn write_file_contents(
  file_path: &Path,
  mut file_contents: FileContents,
) -> Result<(), AnyError> {
  let file_text = if file_contents.had_bom {
    // add back the BOM
    file_contents.text.insert(0, '\u{FEFF}');
    file_contents.text
  } else {
    file_contents.text
  };

  Ok(fs::write(file_path, file_text)?)
}

pub async fn run_parallelized<F>(
  file_paths: Vec<PathBuf>,
  f: F,
) -> Result<(), AnyError>
where
  F: FnOnce(PathBuf) -> Result<(), AnyError> + Send + 'static + Clone,
{
  let handles = file_paths.iter().map(|file_path| {
    let f = f.clone();
    let file_path = file_path.clone();
    spawn_blocking(move || f(file_path))
  });
  let join_results = futures::future::join_all(handles).await;

  // find the tasks that panicked and let the user know which files
  let panic_file_paths = join_results
    .iter()
    .enumerate()
    .filter_map(|(i, join_result)| {
      join_result
        .as_ref()
        .err()
        .map(|_| file_paths[i].to_string_lossy())
    })
    .collect::<Vec<_>>();
  if !panic_file_paths.is_empty() {
    panic!("Panic formatting: {}", panic_file_paths.join(", "))
  }

  // check for any errors and if so return the first one
  let mut errors = join_results.into_iter().filter_map(|join_result| {
    join_result
      .ok()
      .and_then(|handle_result| handle_result.err())
  });

  if let Some(e) = errors.next() {
    Err(e)
  } else {
    Ok(())
  }
}

/// This function is similar to is_supported_ext but adds additional extensions
/// supported by `deno fmt`.
fn is_supported_ext_fmt(path: &Path) -> bool {
  get_extension(path).is_some_and(|ext| {
    matches!(
      ext.as_str(),
      "ts"
        | "tsx"
        | "js"
        | "jsx"
        | "cjs"
        | "cts"
        | "mjs"
        | "mts"
        | "json"
        | "jsonc"
        | "md"
        | "mkd"
        | "mkdn"
        | "mdwn"
        | "mdown"
        | "markdown"
        | "yml"
        | "yaml"
        | "ipynb"
    )
  })
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_is_supported_ext_fmt() {
    assert!(!is_supported_ext_fmt(Path::new("tests/subdir/redirects")));
    assert!(is_supported_ext_fmt(Path::new("README.md")));
    assert!(is_supported_ext_fmt(Path::new("readme.MD")));
    assert!(is_supported_ext_fmt(Path::new("readme.mkd")));
    assert!(is_supported_ext_fmt(Path::new("readme.mkdn")));
    assert!(is_supported_ext_fmt(Path::new("readme.mdwn")));
    assert!(is_supported_ext_fmt(Path::new("readme.mdown")));
    assert!(is_supported_ext_fmt(Path::new("readme.markdown")));
    assert!(is_supported_ext_fmt(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_ext_fmt(Path::new("testdata/run/001_hello.js")));
    assert!(is_supported_ext_fmt(Path::new("testdata/run/002_hello.ts")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.tsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.TS")));
    assert!(is_supported_ext_fmt(Path::new("foo.TSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.JS")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.mjs")));
    assert!(!is_supported_ext_fmt(Path::new("foo.mjsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsonc")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSONC")));
    assert!(is_supported_ext_fmt(Path::new("foo.json")));
    assert!(is_supported_ext_fmt(Path::new("foo.JsON")));
    assert!(is_supported_ext_fmt(Path::new("foo.yml")));
    assert!(is_supported_ext_fmt(Path::new("foo.Yml")));
    assert!(is_supported_ext_fmt(Path::new("foo.yaml")));
    assert!(is_supported_ext_fmt(Path::new("foo.YaML")));
    assert!(is_supported_ext_fmt(Path::new("foo.ipynb")));
  }

  #[test]
  #[should_panic(expected = "Formatting not stable. Bailed after 5 tries.")]
  fn test_format_ensure_stable_unstable_format() {
    format_ensure_stable(
      &PathBuf::from("mod.ts"),
      "1",
      &Default::default(),
      |_, file_text, _| Ok(Some(format!("1{file_text}"))),
    )
    .unwrap();
  }

  #[test]
  fn test_format_ensure_stable_error_first() {
    let err = format_ensure_stable(
      &PathBuf::from("mod.ts"),
      "1",
      &Default::default(),
      |_, _, _| bail!("Error formatting."),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Error formatting.");
  }

  #[test]
  #[should_panic(expected = "Formatting succeeded initially, but failed when")]
  fn test_format_ensure_stable_error_second() {
    format_ensure_stable(
      &PathBuf::from("mod.ts"),
      "1",
      &Default::default(),
      |_, file_text, _| {
        if file_text == "1" {
          Ok(Some("11".to_string()))
        } else {
          bail!("Error formatting.")
        }
      },
    )
    .unwrap();
  }

  #[test]
  fn test_format_stable_after_two() {
    let result = format_ensure_stable(
      &PathBuf::from("mod.ts"),
      "1",
      &Default::default(),
      |_, file_text, _| {
        if file_text == "1" {
          Ok(Some("11".to_string()))
        } else if file_text == "11" {
          Ok(None)
        } else {
          unreachable!();
        }
      },
    )
    .unwrap();

    assert_eq!(result, Some("11".to_string()));
  }

  #[test]
  fn test_single_quote_true_prefers_single_quote() {
    let file_text = format_file(
      &PathBuf::from("test.ts"),
      "console.log(\"there's\");\nconsole.log('hi');\nconsole.log(\"bye\")\n",
      &FmtOptionsConfig {
        single_quote: Some(true),
        ..Default::default()
      },
    )
    .unwrap()
    .unwrap();
    assert_eq!(
      file_text,
      // should use double quotes for the string with a single quote
      "console.log(\"there's\");\nconsole.log('hi');\nconsole.log('bye');\n",
    );
  }
}
