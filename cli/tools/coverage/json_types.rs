// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
  /// Start byte index.
  pub start_offset: usize,
  /// End byte index.
  pub end_offset: usize,
  pub count: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPreciseCoverageParameters {
  pub call_count: bool,
  pub detailed: bool,
  pub allow_triggered_updates: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPreciseCoverageReturnObject {
  pub timestamp: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakePreciseCoverageReturnObject {
  pub result: Vec<ScriptCoverage>,
  pub timestamp: f64,
}

// TODO(bartlomieju): remove me
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCoverage {
  pub result: Vec<ScriptCoverage>,
}
