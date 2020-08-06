// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides file formating utilities using
//! [`dprint`](https://github.com/dsherret/dprint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use crate::colors;
use crate::diff::diff;
use crate::fs::files_in_subtree;
use crate::op_error::OpError;
use crate::text_encoding;
use deno_core::ErrBox;
use dprint_plugin_typescript as dprint;
use std::fs;
use std::io::stdin;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const BOM_CHAR: char = '\u{FEFF}';

/// Format JavaScript/TypeScript files.
///
/// First argument and ignore supports globs, and if it is `None`
/// then the current directory is recursively walked.
pub async fn format(
  args: Vec<String>,
  check: bool,
  exclude: Vec<String>,
) -> Result<(), ErrBox> {
  if args.len() == 1 && args[0] == "-" {
    return format_stdin(check);
  }
  // collect all files provided.
  let mut target_files = collect_files(args)?;
  if !exclude.is_empty() {
    // collect all files to be ignored
    // and retain only files that should be formatted.
    let ignore_files = collect_files(exclude)?;
    target_files.retain(|f| !ignore_files.contains(&f));
  }
  let config = get_config();
  if check {
    check_source_files(config, target_files).await
  } else {
    format_source_files(config, target_files).await
  }
}

async fn check_source_files(
  config: dprint::configuration::Configuration,
  paths: Vec<PathBuf>,
) -> Result<(), ErrBox> {
  let not_formatted_files_count = Arc::new(AtomicUsize::new(0));
  let formatter = Arc::new(dprint::Formatter::new(config));

  // prevent threads outputting at the same time
  let output_lock = Arc::new(Mutex::new(0));

  run_parallelized(paths, {
    let not_formatted_files_count = not_formatted_files_count.clone();
    move |file_path| {
      let file_text = read_file_contents(&file_path)?.text;
      let r = formatter.format_text(&file_path, &file_text);
      match r {
        Ok(formatted_text) => {
          if formatted_text != file_text {
            not_formatted_files_count.fetch_add(1, Ordering::SeqCst);
            let _g = output_lock.lock().unwrap();
            match diff(&file_text, &formatted_text) {
              Ok(diff) => {
                println!();
                println!(
                  "{} {}:",
                  colors::bold("from"),
                  file_path.display().to_string()
                );
                println!("{}", diff);
              }
              Err(e) => {
                eprintln!(
                  "Error generating diff: {}",
                  file_path.to_string_lossy()
                );
                eprintln!("   {}", e);
              }
            }
          }
        }
        Err(e) => {
          let _g = output_lock.lock().unwrap();
          eprintln!("Error checking: {}", file_path.to_string_lossy());
          eprintln!("   {}", e);
        }
      }
      Ok(())
    }
  })
  .await?;

  let not_formatted_files_count =
    not_formatted_files_count.load(Ordering::SeqCst);
  if not_formatted_files_count == 0 {
    Ok(())
  } else {
    Err(
      OpError::other(format!(
        "Found {} not formatted {}",
        not_formatted_files_count,
        files_str(not_formatted_files_count),
      ))
      .into(),
    )
  }
}

async fn format_source_files(
  config: dprint::configuration::Configuration,
  paths: Vec<PathBuf>,
) -> Result<(), ErrBox> {
  let formatted_files_count = Arc::new(AtomicUsize::new(0));
  let formatter = Arc::new(dprint::Formatter::new(config));
  let output_lock = Arc::new(Mutex::new(0)); // prevent threads outputting at the same time

  run_parallelized(paths, {
    let formatted_files_count = formatted_files_count.clone();
    move |file_path| {
      let file_contents = read_file_contents(&file_path)?;
      let r = formatter.format_text(&file_path, &file_contents.text);
      match r {
        Ok(formatted_text) => {
          if formatted_text != file_contents.text {
            write_file_contents(
              &file_path,
              FileContents {
                had_bom: file_contents.had_bom,
                text: formatted_text,
              },
            )?;
            formatted_files_count.fetch_add(1, Ordering::SeqCst);
            let _g = output_lock.lock().unwrap();
            println!("{}", file_path.to_string_lossy());
          }
        }
        Err(e) => {
          let _g = output_lock.lock().unwrap();
          eprintln!("Error formatting: {}", file_path.to_string_lossy());
          eprintln!("   {}", e);
        }
      }
      Ok(())
    }
  })
  .await?;

  let formatted_files_count = formatted_files_count.load(Ordering::SeqCst);
  debug!(
    "Formatted {} {}",
    formatted_files_count,
    files_str(formatted_files_count),
  );
  Ok(())
}

/// Format stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--check` flag.
fn format_stdin(check: bool) -> Result<(), ErrBox> {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    return Err(OpError::other("Failed to read from stdin".to_string()).into());
  }
  let formatter = dprint::Formatter::new(get_config());

  // dprint will fallback to jsx parsing if parsing this as a .ts file doesn't work
  match formatter.format_text(&PathBuf::from("_stdin.ts"), &source) {
    Ok(formatted_text) => {
      if check {
        if formatted_text != source {
          println!("Not formatted stdin");
        }
      } else {
        stdout().write_all(formatted_text.as_bytes())?;
      }
    }
    Err(e) => {
      return Err(OpError::other(e).into());
    }
  }
  Ok(())
}

fn files_str(len: usize) -> &'static str {
  if len == 1 {
    "file"
  } else {
    "files"
  }
}

fn is_supported(path: &Path) -> bool {
  let lowercase_ext = path
    .extension()
    .and_then(|e| e.to_str())
    .map(|e| e.to_lowercase());
  if let Some(ext) = lowercase_ext {
    ext == "ts" || ext == "tsx" || ext == "js" || ext == "jsx" || ext == "mjs"
  } else {
    false
  }
}

pub fn collect_files(files: Vec<String>) -> Result<Vec<PathBuf>, ErrBox> {
  let mut target_files: Vec<PathBuf> = vec![];

  if files.is_empty() {
    target_files
      .extend(files_in_subtree(std::env::current_dir()?, is_supported));
  } else {
    for arg in files {
      let p = PathBuf::from(arg);
      if p.is_dir() {
        target_files.extend(files_in_subtree(p.canonicalize()?, is_supported));
      } else {
        target_files.push(p.canonicalize()?);
      };
    }
  }

  Ok(target_files)
}

fn get_config() -> dprint::configuration::Configuration {
  use dprint::configuration::*;
  ConfigurationBuilder::new().deno().build()
}

struct FileContents {
  text: String,
  had_bom: bool,
}

fn read_file_contents(file_path: &PathBuf) -> Result<FileContents, ErrBox> {
  let file_bytes = fs::read(&file_path)?;
  let charset = text_encoding::detect_charset(&file_bytes);
  let file_text = text_encoding::convert_to_utf8(&file_bytes, charset)?;
  let had_bom = file_text.starts_with(BOM_CHAR);
  let text = if had_bom {
    // remove the BOM
    String::from(&file_text[BOM_CHAR.len_utf8()..])
  } else {
    String::from(file_text)
  };

  Ok(FileContents { text, had_bom })
}

fn write_file_contents(
  file_path: &PathBuf,
  file_contents: FileContents,
) -> Result<(), ErrBox> {
  let file_text = if file_contents.had_bom {
    // add back the BOM
    format!("{}{}", BOM_CHAR, file_contents.text)
  } else {
    file_contents.text
  };

  Ok(fs::write(file_path, file_text)?)
}

pub async fn run_parallelized<F>(
  file_paths: Vec<PathBuf>,
  f: F,
) -> Result<(), ErrBox>
where
  F: FnOnce(PathBuf) -> Result<(), ErrBox> + Send + 'static + Clone,
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
      .map(|handle_result| handle_result.err())
      .flatten()
  });

  if let Some(e) = errors.next() {
    Err(e)
  } else {
    Ok(())
  }
}

#[test]
fn test_is_supported() {
  assert!(!is_supported(Path::new("tests/subdir/redirects")));
  assert!(!is_supported(Path::new("README.md")));
  assert!(is_supported(Path::new("lib/typescript.d.ts")));
  assert!(is_supported(Path::new("cli/tests/001_hello.js")));
  assert!(is_supported(Path::new("cli/tests/002_hello.ts")));
  assert!(is_supported(Path::new("foo.jsx")));
  assert!(is_supported(Path::new("foo.tsx")));
  assert!(is_supported(Path::new("foo.TS")));
  assert!(is_supported(Path::new("foo.TSX")));
  assert!(is_supported(Path::new("foo.JS")));
  assert!(is_supported(Path::new("foo.JSX")));
  assert!(is_supported(Path::new("foo.mjs")));
  assert!(!is_supported(Path::new("foo.mjsx")));
}
