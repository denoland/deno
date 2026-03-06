

pub const COVERAGE_NOTIFICATION: &str = "deno/testCoverage";

/// Coverage data for a single file, sent from the LSP after a coverage test run.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
  /// The URI of the source file.
  pub uri: lsp::Uri,
  /// Line numbers (1-indexed) that were executed during the test run.
  pub covered_lines: Vec<u32>,
  /// Line numbers (1-indexed) that were not executed during the test run.
  pub uncovered_lines: Vec<u32>,
  /// The percentage of lines covered (0.0–100.0).
  pub coverage_percent: f64,
}

/// Parameters for the `deno/testCoverage` LSP notification.
///
/// Sent by the LSP server to the client after a `Coverage`-kind test run
/// completes, so the editor can display inline coverage decorations.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageNotificationParams {
  /// The test run ID this coverage belongs to.
  pub id: u32,
  /// Per-file coverage data for all workspace files exercised by the run.
  pub files: Vec<FileCoverage>,
}

pub enum CoverageNotification {}

impl lsp::notification::Notification for CoverageNotification {
  type Params = CoverageNotificationParams;

  const METHOD: &'static str = COVERAGE_NOTIFICATION;
}