// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module provides file formating utilities using
//! [`dprint`](https://github.com/dsherret/dprint).
//!
//! At the moment it is only consumed using CLI but in
//! the future it can be easily extended to provide
//! the same functions as ops available in JS runtime.

use deno_core::ErrBox;
use dprint_plugin_typescript as dprint;
use glob;
use std::fs;
use std::io::stdin;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

fn is_supported(path: &Path) -> bool {
  if let Some(ext) = path.extension() {
    if ext == "tsx" || ext == "js" || ext == "jsx" {
      true
    } else if ext == "ts" {
      // Currently dprint does not support d.ts files.
      // https://github.com/dsherret/dprint/issues/100
      !path.as_os_str().to_string_lossy().ends_with(".d.ts")
    } else {
      false
    }
  } else {
    false
  }
}

fn get_config() -> dprint::configuration::Configuration {
  dprint::configuration::ConfigurationBuilder::new()
    .line_width(80)
    .indent_width(2)
    .next_control_flow_position(
      dprint::configuration::NextControlFlowPosition::SameLine,
    )
    .binary_expression_operator_position(
      dprint::configuration::OperatorPosition::SameLine,
    )
    .build()
}

fn check_source_files(
  config: dprint::configuration::Configuration,
  paths: Vec<PathBuf>,
) -> Result<(), ErrBox> {
  let start = Instant::now();
  let mut not_formatted_files = vec![];

  for file_path in paths {
    let file_path_str = file_path.to_string_lossy();
    let file_contents = fs::read_to_string(&file_path).unwrap();
    match dprint::format_text(&file_path_str, &file_contents, &config) {
      Ok(None) => {
        // nothing to format, pass
      }
      Ok(Some(formatted_text)) => {
        if formatted_text != file_contents {
          not_formatted_files.push(file_path);
        }
      }
      Err(e) => {
        eprintln!("Error checking: {}", &file_path_str);
        eprintln!("   {}", e);
      }
    }
  }

  let duration = Instant::now() - start;

  if not_formatted_files.is_empty() {
    Ok(())
  } else {
    let f = if not_formatted_files.len() == 1 {
      "file"
    } else {
      "files"
    };
    Err(crate::deno_error::other_error(format!(
      "Found {} not formatted {} in {:?}",
      not_formatted_files.len(),
      f,
      duration
    )))
  }
}

fn format_source_files(
  config: dprint::configuration::Configuration,
  paths: Vec<PathBuf>,
) {
  let start = Instant::now();
  let mut not_formatted_files = vec![];

  for file_path in paths {
    let file_path_str = file_path.to_string_lossy();
    let file_contents = fs::read_to_string(&file_path).unwrap();
    match dprint::format_text(&file_path_str, &file_contents, &config) {
      Ok(None) => {
        // nothing to format, pass
      }
      Ok(Some(formatted_text)) => {
        if formatted_text != file_contents {
          println!("Formatting {}", file_path_str);
          fs::write(&file_path, formatted_text).unwrap();
          not_formatted_files.push(file_path);
        }
      }
      Err(e) => {
        eprintln!("Error formatting: {}", &file_path_str);
        eprintln!("   {}", e);
      }
    }
  }

  let duration = Instant::now() - start;
  let f = if not_formatted_files.len() == 1 {
    "file"
  } else {
    "files"
  };
  eprintln!(
    "Formatted {} {} in {:?}",
    not_formatted_files.len(),
    f,
    duration
  );
}

pub fn source_files_in_subtree(root: PathBuf) -> Vec<PathBuf> {
  assert!(root.is_dir());
  // TODO(ry) Use WalkDir instead of globs.
  let g = root.join("**/*");
  glob::glob(&g.into_os_string().into_string().unwrap())
    .expect("Failed to execute glob.")
    .filter_map(|result| {
      if let Ok(p) = result {
        if is_supported(&p) {
          Some(p)
        } else {
          None
        }
      } else {
        None
      }
    })
    .collect()
}

/// Format JavaScript/TypeScript files.
///
/// First argument supports globs, and if it is `None`
/// then the current directory is recursively walked.
pub fn format_files(args: Vec<String>, check: bool) -> Result<(), ErrBox> {
  if args.len() == 1 && args[0] == "-" {
    format_stdin(check);
    return Ok(());
  }

  let mut target_files: Vec<PathBuf> = vec![];

  if args.is_empty() {
    target_files
      .extend(source_files_in_subtree(std::env::current_dir().unwrap()));
  } else {
    for arg in args {
      let p = PathBuf::from(arg);
      if p.is_dir() {
        target_files.extend(source_files_in_subtree(p));
      } else {
        target_files.push(p);
      };
    }
  }
  let config = get_config();
  if check {
    check_source_files(config, target_files)?;
  } else {
    format_source_files(config, target_files);
  }
  Ok(())
}

/// Format stdin and write result to stdout.
/// Treats input as TypeScript.
/// Compatible with `--check` flag.
fn format_stdin(check: bool) {
  let mut source = String::new();
  if stdin().read_to_string(&mut source).is_err() {
    eprintln!("Failed to read from stdin");
  }
  let config = get_config();

  match dprint::format_text("_stdin.ts", &source, &config) {
    Ok(None) => unreachable!(),
    Ok(Some(formatted_text)) => {
      if check {
        if formatted_text != source {
          println!("Not formatted stdin");
        }
      } else {
        let _r = stdout().write_all(formatted_text.as_bytes());
        // TODO(ry) Only ignore SIGPIPE. Currently ignoring all errors.
      }
    }
    Err(e) => {
      eprintln!("Error formatting from stdin");
      eprintln!("   {}", e);
    }
  }
}

#[test]
fn test_is_supported() {
  assert!(!is_supported(Path::new("tests/subdir/redirects")));
  assert!(!is_supported(Path::new("README.md")));
  assert!(!is_supported(Path::new("lib/typescript.d.ts")));
  assert!(is_supported(Path::new("cli/tests/001_hello.js")));
  assert!(is_supported(Path::new("cli/tests/002_hello.ts")));
  assert!(is_supported(Path::new("foo.jsx")));
  assert!(is_supported(Path::new("foo.tsx")));
}

#[test]
fn check_tests_dir() {
  // Because of cli/tests/error_syntax.js the following should fail but not
  // crash.
  let r = format_files(vec!["./tests".to_string()], true);
  assert!(r.is_err());
}
