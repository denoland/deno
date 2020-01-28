// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(unused)]

use dprint_plugin_typescript::{
  format_text, ResolvedTypeScriptConfiguration, TypeScriptConfiguration,
};
use glob;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

lazy_static! {
  static ref TYPESCRIPT: Regex = Regex::new(".tsx?$").unwrap();
  static ref JAVASCRIPT: Regex = Regex::new(".jsx?$").unwrap();
}

fn is_supported(path: &Path) -> bool {
  let path_str = path.to_string_lossy();
  TYPESCRIPT.is_match(&path_str) || JAVASCRIPT.is_match(&path_str)
}

fn get_config() -> ResolvedTypeScriptConfiguration {
  TypeScriptConfiguration::new()
    .line_width(80)
    .indent_width(2)
    .resolve()
}

fn get_supported_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
  let mut files_to_check = vec![];

  for path in paths {
    if is_supported(&path) {
      eprintln!("Found matching file: {:?}", path);
      files_to_check.push(path.to_owned());
    }
  }

  files_to_check
}

fn check_source_files(paths: Vec<PathBuf>) {
  let files_to_check = get_supported_files(paths);
  let config = get_config();
  let mut not_formatted_files = vec![];

  for file_path in files_to_check {
    let file_path_str = file_path.to_string_lossy();
    let file_contents = fs::read_to_string(&file_path).unwrap();
    match format_text(&file_path_str, &file_contents, &config) {
      Ok(None) => {
        // nothing to format, pass
      }
      Ok(Some(formatted_text)) => {
        if formatted_text != file_contents {
          not_formatted_files.push(file_path);
        }
      }
      Err(_) => {
        panic!("error during formatting");
      }
    }
  }

  eprintln!("Finished check {} not formatted", not_formatted_files.len());
}

fn format_source_files(paths: Vec<PathBuf>) {
  let files_to_format = get_supported_files(paths);
  let config = get_config();
  let mut not_formatted_files = vec![];

  for file_path in files_to_format {
    let file_path_str = file_path.to_string_lossy();
    let file_contents = fs::read_to_string(&file_path).unwrap();
    match format_text(&file_path_str, &file_contents, &config) {
      Ok(None) => {
        // nothing to format, pass
      }
      Ok(Some(formatted_text)) => {
        if formatted_text != file_contents {
          println!("Formatting {:?}", file_path);
          fs::write(&file_path, formatted_text).unwrap();
          not_formatted_files.push(file_path);
        }
      }
      Err(_) => {
        panic!("error during formatting");
      }
    }
  }

  eprintln!("Finished check {} not formatted", not_formatted_files.len());
}

fn get_target_files(include: Vec<&str>) -> Vec<PathBuf> {
  let mut target_files = Vec::with_capacity(128);

  for path in include {
    let files = glob::glob(path)
      .expect("Failed to execute glob.")
      .filter_map(Result::ok);
    target_files.extend(files);
  }

  target_files
}

pub fn format(check: bool) {
  let files = get_target_files(vec!["**/*"]);

  if check {
    check_source_files(files);
  } else {
    format_source_files(files);
  }
}
