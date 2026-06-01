// Copyright 2018-2026 the Deno authors. MIT license.

use crate::args::Flags;
use crate::lsp::lsp_custom::TestIdentifier;
use crate::lsp::lsp_custom::TestRunProgressMessage;
use crate::lsp::lsp_custom::TestRunProgressParams;
use crate::tools::coverage::CoverageCollector;
use crate::tools::coverage::CoverageRange;
use crate::tools::coverage::Coverage;

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::ModuleSpecifier;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TestRunKind {
  Run,
  Debug,
  Coverage,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TestRunParams {
  pub id: i64,
  pub kind: TestRunKind,
  pub include: Option<Vec<TestIdentifier>>,
  pub exclude: Option<Vec<TestIdentifier>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatementCoverage {
  pub range: Range,
  pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
  pub uri: ModuleSpecifier,
  pub statement_coverage: Vec<StatementCoverage>,
}

// We extend the TestRunProgressMessage enum to include a Coverage variant
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TestRunProgressMessageExtended {
  // Existing variants...
  Coverage {
    coverage: Vec<FileCoverage>,
  },
}

// Implementation details for collecting and remapping coverage in the LSP server
// when kind == TestRunKind::Coverage.
