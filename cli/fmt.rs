// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides file formating utilities using
//! [`dprint`](https://github.com/dsherret/dprint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use crate::fs::files_in_subtree;
use crate::op_error::OpError;
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

fn is_supported(path: &Path) -> bool {
  if let Some(ext) = path.extension() {
    ext == "ts" || ext == "tsx" || ext == "js" || ext == "jsx"
  } else {
    false
  }
}

fn get_config() -> dprint::configuration::Configuration {
  use dprint::configuration::*;
  ConfigurationBuilder::new().deno().build()
}

async fn check_source_files(
  config: dprint::configuration::Configuration,
  paths: Vec<PathBuf>,
) -> Result<(), ErrBox> {
  let not_formatted_files_count = Arc::new(AtomicUsize::new(0));
  let formatter = Arc::new(dprint::Formatter::new(config));
  let output_lock = Arc::new(Mutex::new(0)); // prevent threads outputting at the same time

  run_parallelized(paths, {
    let not_formatted_files_count = not_formatted_files_count.clone();
    move |file_path| {
      let file_path_str = file_path.to_string_lossy();
      let file_contents = fs::read_to_string(&file_path)?;
      let r = formatter.format_text(&file_path_str, &file_contents);
      match r {
        Ok(formatted_text) => {
          if formatted_text != file_contents {
            not_formatted_files_count.fetch_add(1, Ordering::SeqCst);
          }
        }
        Err(e) => {
          let _g = output_lock.lock().unwrap();
          eprintln!("Error checking: {}", &file_path_str);
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

fn files_str(len: usize) -> &'static str {
  if len == 1 {
    "file"
  } else {
    "files"
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
      let file_path_str = file_path.to_string_lossy();
      let file_contents = fs::read_to_string(&file_path)?;
      let r = formatter.format_text(&file_path_str, &file_contents);
      match r {
        Ok(formatted_text) => {
          if formatted_text != file_contents {
            fs::write(&file_path, formatted_text)?;
            formatted_files_count.fetch_add(1, Ordering::SeqCst);
            let _g = output_lock.lock().unwrap();
            println!("{}", file_path_str);
          }
        }
        Err(e) => {
          let _g = output_lock.lock().unwrap();
          eprintln!("Error formatting: {}", &file_path_str);
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

/// Format JavaScript/TypeScript files.
///
/// First argument supports globs, and if it is `None`
/// then the current directory is recursively walked.
pub async fn format(args: Vec<String>, check: bool) -> Result<(), ErrBox> {
  if args.len() == 1 && args[0] == "-" {
    return format_stdin(check);
  }

  let mut target_files: Vec<PathBuf> = vec![];

  if args.is_empty() {
    target_files.extend(files_in_subtree(
      std::env::current_dir().unwrap(),
      is_supported,
    ));
  } else {
    for arg in args {
      let p = PathBuf::from(arg);
      if p.is_dir() {
        target_files.extend(files_in_subtree(p, is_supported));
      } else {
        target_files.push(p);
      };
    }
  }
  let config = get_config();
  if check {
    check_source_files(config, target_files).await?;
  } else {
    format_source_files(config, target_files).await?;
  }
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

  match formatter.format_text("_stdin.ts", &source) {
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

async fn run_parallelized<F>(
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
}

#[tokio::test]
async fn check_tests_dir() {
  // Because of cli/tests/error_syntax.js the following should fail but not
  // crash.
  let r = format(vec!["./tests".to_string()], true).await;
  assert!(r.is_err());
}
