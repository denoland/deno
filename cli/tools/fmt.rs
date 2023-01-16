// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

//! This module provides file formatting utilities using
//! [`dprint-plugin-typescript`](https://github.com/dprint/dprint-plugin-typescript).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use crate::args::CliOptions;
use crate::args::FilesConfig;
use crate::args::FmtOptions;
use crate::args::FmtOptionsConfig;
use crate::args::ProseWrap;
use crate::colors;
use crate::util::diff::diff;
use crate::util::file_watcher;
use crate::util::file_watcher::ResolutionResult;
use crate::util::fs::FileCollector;
use crate::util::path::get_extension;
use crate::util::text_encoding;
use deno_ast::ParsedSource;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
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
  cli_options: CliOptions,
  fmt_options: FmtOptions,
) -> Result<(), AnyError> {
  if fmt_options.is_stdin {
    return format_stdin(fmt_options);
  }

  let files = fmt_options.files;
  let check = fmt_options.check;
  let fmt_config_options = fmt_options.options;

  let resolver = |changed: Option<Vec<PathBuf>>| {
    let files_changed = changed.is_some();

    let result = collect_fmt_files(&files).map(|files| {
      let refmt_files = if let Some(paths) = changed {
        if check {
          files
            .iter()
            .any(|path| paths.contains(path))
            .then_some(files)
            .unwrap_or_else(|| [].to_vec())
        } else {
          files
            .into_iter()
            .filter(|path| paths.contains(path))
            .collect::<Vec<_>>()
        }
      } else {
        files
      };
      (refmt_files, fmt_config_options.clone())
    });

    let paths_to_watch = files.include.clone();
    async move {
      if files_changed
        && matches!(result, Ok((ref files, _)) if files.is_empty())
      {
        ResolutionResult::Ignore
      } else {
        ResolutionResult::Restart {
          paths_to_watch,
          result,
        }
      }
    }
  };
  let deno_dir = &cli_options.resolve_deno_dir()?;
  let operation = |(paths, fmt_options): (Vec<PathBuf>, FmtOptionsConfig)| async move {
    let incremental_cache = Arc::new(IncrementalCache::new(
      &deno_dir.fmt_incremental_cache_db_file_path(),
      &fmt_options,
      &paths,
    ));
    if check {
      check_source_files(paths, fmt_options, incremental_cache.clone()).await?;
    } else {
      format_source_files(paths, fmt_options, incremental_cache.clone())
        .await?;
    }
    incremental_cache.wait_completion().await;
    Ok(())
  };

  if cli_options.watch_paths().is_some() {
    file_watcher::watch_func(
      resolver,
      operation,
      file_watcher::PrintConfig {
        job_name: "Fmt".to_string(),
        clear_screen: !cli_options.no_clear_screen(),
      },
    )
    .await?;
  } else {
    let files = collect_fmt_files(&files).and_then(|files| {
      if files.is_empty() {
        Err(generic_error("No target files found."))
      } else {
        Ok(files)
      }
    })?;
    operation((files, fmt_config_options)).await?;
  }

  Ok(())
}

fn collect_fmt_files(files: &FilesConfig) -> Result<Vec<PathBuf>, AnyError> {
  FileCollector::new(is_supported_ext_fmt)
    .ignore_git_folder()
    .ignore_node_modules()
    .add_ignore_paths(&files.exclude)
    .collect_files(&files.include)
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
      ) {
        // It's important to tell dprint proper file extension, otherwise
        // it might parse the file twice.
        let extension = match tag.as_str() {
          "javascript" => "js",
          "typescript" => "ts",
          rest => rest,
        };

        if matches!(extension, "json" | "jsonc") {
          let mut json_config = get_resolved_json_config(fmt_options);
          json_config.line_width = line_width;
          dprint_plugin_json::format_text(text, &json_config)
        } else {
          let fake_filename =
            PathBuf::from(format!("deno_fmt_stdin.{}", extension));
          let mut codeblock_config =
            get_resolved_typescript_config(fmt_options);
          codeblock_config.line_width = line_width;
          dprint_plugin_typescript::format_text(
            &fake_filename,
            text,
            &codeblock_config,
          )
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
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let config = get_resolved_json_config(fmt_options);
  dprint_plugin_json::format_text(file_text, &config)
}

/// Formats a single TS, TSX, JS, JSX, JSONC, JSON, or MD file.
pub fn format_file(
  file_path: &Path,
  file_text: &str,
  fmt_options: &FmtOptionsConfig,
) -> Result<Option<String>, AnyError> {
  let ext = get_extension(file_path).unwrap_or_default();
  if matches!(
    ext.as_str(),
    "md" | "mkd" | "mkdn" | "mdwn" | "mdown" | "markdown"
  ) {
    format_markdown(file_text, fmt_options)
  } else if matches!(ext.as_str(), "json" | "jsonc") {
    format_json(file_text, fmt_options)
  } else {
    let config = get_resolved_typescript_config(fmt_options);
    dprint_plugin_typescript::format_text(file_path, file_text, &config)
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

async fn check_source_files(
  paths: Vec<PathBuf>,
  fmt_options: FmtOptionsConfig,
  incremental_cache: Arc<IncrementalCache>,
) -> Result<(), AnyError> {
  let not_formatted_files_count = Arc::new(AtomicUsize::new(0));
  let checked_files_count = Arc::new(AtomicUsize::new(0));

  // prevent threads outputting at the same time
  let output_lock = Arc::new(Mutex::new(0));

  run_parallelized(paths, {
    let not_formatted_files_count = not_formatted_files_count.clone();
    let checked_files_count = checked_files_count.clone();
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
            format!("{}", e)
              .split('\n')
              .map(|l| {
                if l.trim().is_empty() {
                  String::new()
                } else {
                  format!("  {}", l)
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

  let not_formatted_files_count =
    not_formatted_files_count.load(Ordering::Relaxed);
  let checked_files_count = checked_files_count.load(Ordering::Relaxed);
  let checked_files_str =
    format!("{} {}", checked_files_count, files_str(checked_files_count));
  if not_formatted_files_count == 0 {
    info!("Checked {}", checked_files_str);
    Ok(())
  } else {
    let not_formatted_files_str = files_str(not_formatted_files_count);
    Err(generic_error(format!(
      "Found {} not formatted {} in {}",
      not_formatted_files_count, not_formatted_files_str, checked_files_str,
    )))
  }
}

async fn format_source_files(
  paths: Vec<PathBuf>,
  fmt_options: FmtOptionsConfig,
  incremental_cache: Arc<IncrementalCache>,
) -> Result<(), AnyError> {
  let formatted_files_count = Arc::new(AtomicUsize::new(0));
  let checked_files_count = Arc::new(AtomicUsize::new(0));
  let output_lock = Arc::new(Mutex::new(0)); // prevent threads outputting at the same time

  run_parallelized(paths, {
    let formatted_files_count = formatted_files_count.clone();
    let checked_files_count = checked_files_count.clone();
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
          eprintln!("Error formatting: {}", file_path.to_string_lossy());
          eprintln!("   {}", e);
        }
      }
      Ok(())
    }
  })
  .await?;

  let formatted_files_count = formatted_files_count.load(Ordering::Relaxed);
  debug!(
    "Formatted {} {}",
    formatted_files_count,
    files_str(formatted_files_count),
  );

  let checked_files_count = checked_files_count.load(Ordering::Relaxed);
  info!(
    "Checked {} {}",
    checked_files_count,
    files_str(checked_files_count)
  );

  Ok(())
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
                "the text it produces is not syntatically correct. As a temporary ",
                "workfaround you can ignore this file ({}).\n\n{:#}"
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
/// Treats input as TypeScript or as set by `--ext` flag.
/// Compatible with `--check` flag.
fn format_stdin(fmt_options: FmtOptions) -> Result<(), AnyError> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    bail!("Failed to read from stdin");
  }
  let file_path = PathBuf::from(format!("_stdin.{}", fmt_options.ext));
  let formatted_text = format_file(&file_path, &source, &fmt_options.options)?;
  if fmt_options.check {
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
        dprint_plugin_typescript::configuration::QuoteStyle::AlwaysSingle,
      );
    }
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

struct FileContents {
  text: String,
  had_bom: bool,
}

fn read_file_contents(file_path: &Path) -> Result<FileContents, AnyError> {
  let file_bytes = fs::read(file_path)
    .with_context(|| format!("Error reading {}", file_path.display()))?;
  let charset = text_encoding::detect_charset(&file_bytes);
  let file_text = text_encoding::convert_to_utf8(&file_bytes, charset)
    .map_err(|_| {
      anyhow!("{} is not a valid UTF-8 file", file_path.display())
    })?;
  let had_bom = file_text.starts_with(text_encoding::BOM_CHAR);
  let text = if had_bom {
    text_encoding::strip_bom(&file_text).to_string()
  } else {
    file_text.to_string()
  };

  Ok(FileContents { text, had_bom })
}

fn write_file_contents(
  file_path: &Path,
  file_contents: FileContents,
) -> Result<(), AnyError> {
  let file_text = if file_contents.had_bom {
    // add back the BOM
    format!("{}{}", text_encoding::BOM_CHAR, file_contents.text)
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
    tokio::task::spawn_blocking(move || f(file_path))
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
  if let Some(ext) = get_extension(path) {
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
    )
  } else {
    false
  }
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
  }

  #[test]
  #[should_panic(expected = "Formatting not stable. Bailed after 5 tries.")]
  fn test_format_ensure_stable_unstable_format() {
    format_ensure_stable(
      &PathBuf::from("mod.ts"),
      "1",
      &Default::default(),
      |_, file_text, _| Ok(Some(format!("1{}", file_text))),
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
}
