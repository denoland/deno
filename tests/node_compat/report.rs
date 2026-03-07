// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use test_util as util;

use crate::tests_path;

/// Report structures for generating report.json
#[derive(Debug, Serialize)]
struct TestReport {
  date: String,
  #[serde(rename = "denoVersion")]
  deno_version: String,
  os: String,
  arch: String,
  #[serde(rename = "nodeVersion")]
  node_version: String,
  #[serde(rename = "runId")]
  run_id: Option<String>,
  total: usize,
  pass: usize,
  ignore: usize,
  results: HashMap<String, TestResultEntry>,
}

// Result entry: [pass: bool | "IGNORE", error: Option<ErrorInfo>, info: ResultInfo]
type TestResultEntry = (Value, Option<ErrorInfo>, ResultInfo);

#[derive(Debug, Serialize, Clone)]
pub struct ErrorInfo {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub code: Option<i32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stderr: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub timeout: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
}

#[derive(Debug, Default, Serialize, Clone)]
struct ResultInfo {
  #[serde(rename = "usesNodeTest", skip_serializing_if = "Option::is_none")]
  uses_node_test: Option<u8>,
  #[serde(rename = "ignoreReason", skip_serializing_if = "Option::is_none")]
  ignore_reason: Option<String>,
}

/// Collected test result for report generation
#[derive(Debug, Clone)]
pub struct CollectedResult {
  pub passed: Option<bool>, // None means ignored
  pub error: Option<ErrorInfo>,
  pub uses_node_test: bool,
  pub ignore_reason: Option<String>,
}

pub fn generate_report(results: &HashMap<String, CollectedResult>) {
  let node_version = read_node_version();
  let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let deno_version = get_deno_version();
  let os = std::env::consts::OS.to_string();
  let arch = std::env::consts::ARCH.to_string();
  let run_id = std::env::var("GITHUB_RUN_ID").ok();

  let mut report_results: HashMap<String, TestResultEntry> = HashMap::new();
  let mut pass_count = 0;
  let mut ignore_count = 0;

  for (test_path, result) in results {
    let entry = match result.passed {
      Some(true) => {
        pass_count += 1;
        let info = ResultInfo {
          uses_node_test: if result.uses_node_test { Some(1) } else { None },
          ignore_reason: None,
        };
        (Value::Bool(true), None, info)
      }
      Some(false) => {
        let info = ResultInfo {
          uses_node_test: if result.uses_node_test { Some(1) } else { None },
          ignore_reason: None,
        };
        (Value::Bool(false), result.error.clone(), info)
      }
      None => {
        ignore_count += 1;
        let info = ResultInfo {
          uses_node_test: None,
          ignore_reason: result.ignore_reason.clone(),
        };
        (Value::String("IGNORE".to_string()), None, info)
      }
    };
    report_results.insert(test_path.clone(), entry);
  }

  let total = results.len() - ignore_count;
  let report = TestReport {
    date,
    deno_version,
    os,
    arch,
    node_version,
    run_id,
    total,
    pass: pass_count,
    ignore: ignore_count,
    results: report_results,
  };

  let report_path = tests_path().join("node_compat").join("report.json");
  let json = serde_json::to_string(&report).unwrap();
  report_path.write(json);
}

fn get_deno_version() -> String {
  let output = std::process::Command::new(util::deno_exe_path().as_path())
    .arg("-v")
    .output()
    .ok()
    .unwrap();

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Parse: "deno 2.x.x (...)"
  let line = stdout.lines().next().unwrap();
  let version = line.strip_prefix("deno ").unwrap();
  let version = version.split_whitespace().next().unwrap();
  version.to_string()
}

fn read_node_version() -> String {
  // Read from tests/node_compat/runner/suite/node_version.ts
  let version_file =
    tests_path().join("node_compat/runner/suite/node_version.ts");
  let content = version_file.read_to_string();

  // Parse: export const version = "24.2.0";
  let re = Regex::new(r#"export const version = "([^"]+)"#).unwrap();
  let captures = re.captures(&content).unwrap();
  captures.get(1).unwrap().as_str().to_string()
}
