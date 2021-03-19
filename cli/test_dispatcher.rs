use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResult {
  Ok,
  Ignored,
  Failed(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum TestMessage {
  Plan {
    pending: usize,
    filtered: usize,
    only: bool,
  },
  Wait {
    name: String,
  },
  Result {
    name: String,
    duration: usize,
    result: TestResult,
  },
}
