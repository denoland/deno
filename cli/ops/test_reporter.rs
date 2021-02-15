use crate::tools::test::TestPlan;
use crate::tools::test::TestReporter;
use crate::tools::test::TestResult;
use crate::tools::test::TestSummary;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_report_test_plan", op_report_test_plan);
  super::reg_json_sync(rt, "op_report_test_result", op_report_test_result);
  super::reg_json_sync(rt, "op_report_test_summary", op_report_test_summary);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTestPlanArgs {
  count: usize,
}

fn op_report_test_plan(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ReportTestPlanArgs = serde_json::from_value(args)?;

  let reporter_mutex = state.borrow::<Arc<Mutex<TestReporter>>>().clone();
  if let Ok(mut reporter) = reporter_mutex.lock() {
    reporter.visit_plan(TestPlan { count: args.count });
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTestResultArgs {
  name: String,
  ignore: bool,
  error: Option<String>,
}

fn op_report_test_result(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ReportTestResultArgs = serde_json::from_value(args)?;

  let reporter_mutex = state.borrow::<Arc<Mutex<TestReporter>>>().clone();
  if let Ok(mut reporter) = reporter_mutex.lock() {
    reporter.visit_result(TestResult {
      name: args.name,
      ignore: args.ignore,
      error: args.error,
    });
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTestSummaryArgs {
  failed: usize,
  filtered: usize,
  ignored: usize,
  measured: usize,
  passed: usize,
}

fn op_report_test_summary(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ReportTestSummaryArgs = serde_json::from_value(args)?;

  let reporter_mutex = state.borrow::<Arc<Mutex<TestReporter>>>().clone();

  if let Ok(mut reporter) = reporter_mutex.lock() {
    reporter.visit_summary(TestSummary {
      failed: args.failed,
      filtered: args.filtered,
      ignored: args.ignored,
      measured: args.measured,
      passed: args.passed,
    });
  }

  Ok(json!({}))
}
