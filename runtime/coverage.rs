// Copyright 2018-2025 the Deno authors. MIT license.

use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use deno_core::InspectorSessionKind;
use deno_core::JsRuntime;
use deno_core::JsRuntimeInspector;
use deno_core::LocalInspectorSession;
use deno_core::error::CoreError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use uuid::Uuid;

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);

fn next_msg_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug)]
pub struct CoverageCollectorInner {
  dir: PathBuf,
  coverage_msg_id: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct CoverageCollectorState(Arc<Mutex<CoverageCollectorInner>>);

impl CoverageCollectorState {
  pub fn new(dir: PathBuf) -> Self {
    Self(Arc::new(Mutex::new(CoverageCollectorInner {
      dir,
      coverage_msg_id: None,
    })))
  }

  pub fn callback(&self, msg: deno_core::InspectorMsg) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      return;
    };
    let maybe_coverage_msg_id = self.0.lock().coverage_msg_id.as_ref().cloned();

    if let Some(coverage_msg_id) = maybe_coverage_msg_id
      && coverage_msg_id == msg_id
    {
      let message: serde_json::Value =
        serde_json::from_str(&msg.content).unwrap();
      let coverages: cdp::TakePreciseCoverageResponse =
        serde_json::from_value(message["result"].clone()).unwrap();
      self.write_coverages(coverages.result);
    }
  }

  fn write_coverages(&self, script_coverages: Vec<cdp::ScriptCoverage>) {
    for script_coverage in script_coverages {
      // Filter out internal and http/https JS files, eval'd scripts,
      // and scripts with invalid urls from being included in coverage reports
      if script_coverage.url.is_empty()
        || script_coverage.url.starts_with("ext:")
        || script_coverage.url.starts_with("[ext:")
        || script_coverage.url.starts_with("http:")
        || script_coverage.url.starts_with("https:")
        || script_coverage.url.starts_with("node:")
        || Url::parse(&script_coverage.url).is_err()
      {
        continue;
      }

      let filename = format!("{}.json", Uuid::new_v4());
      let filepath = self.0.lock().dir.join(filename);

      let file = match File::create(&filepath) {
        Ok(f) => f,
        Err(err) => {
          log::error!(
            "Failed to create coverage file at {:?}, reason: {:?}",
            filepath,
            err
          );
          continue;
        }
      };
      let mut out = BufWriter::new(file);
      let coverage = serde_json::to_string_pretty(&script_coverage).unwrap();

      if let Err(err) = out.write_all(coverage.as_bytes()) {
        log::error!(
          "Failed to write coverage file at {:?}, reason: {:?}",
          filepath,
          err
        );
        continue;
      }
      if let Err(err) = out.flush() {
        log::error!(
          "Failed to flush coverage file at {:?}, reason: {:?}",
          filepath,
          err
        );
        continue;
      }
    }
  }
}

pub struct CoverageCollector {
  pub state: CoverageCollectorState,
  session: LocalInspectorSession,
}

impl CoverageCollector {
  pub fn new(js_runtime: &mut JsRuntime, coverage_dir: PathBuf) -> Self {
    let state = CoverageCollectorState::new(coverage_dir);

    js_runtime.maybe_init_inspector();
    let insp = js_runtime.inspector();

    let s = state.clone();
    let callback = Box::new(move |message| s.clone().callback(message));
    let session = JsRuntimeInspector::create_local_session(
      insp,
      callback,
      InspectorSessionKind::Blocking,
    );

    Self { state, session }
  }

  pub fn start_collecting(&mut self) {
    self
      .session
      .post_message::<()>(next_msg_id(), "Profiler.enable", None);
    self.session.post_message(
      next_msg_id(),
      "Profiler.startPreciseCoverage",
      Some(cdp::StartPreciseCoverageArgs {
        call_count: true,
        detailed: true,
        allow_triggered_updates: false,
      }),
    );
  }

  #[allow(clippy::disallowed_methods)]
  pub fn stop_collecting(&mut self) -> Result<(), CoreError> {
    fs::create_dir_all(&self.state.0.lock().dir)?;
    let msg_id = next_msg_id();
    self.state.0.lock().coverage_msg_id.replace(msg_id);

    self.session.post_message::<()>(
      msg_id,
      "Profiler.takePreciseCoverage",
      None,
    );
    Ok(())
  }
}

mod cdp {
  use serde::Deserialize;
  use serde::Serialize;

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#method-takePreciseCoverage>
  #[derive(Debug, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct TakePreciseCoverageResponse {
    pub result: Vec<ScriptCoverage>,
    pub timestamp: f64,
  }

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#type-CoverageRange>
  #[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct CoverageRange {
    /// Start character index.
    #[serde(rename = "startOffset")]
    pub start_char_offset: usize,
    /// End character index.
    #[serde(rename = "endOffset")]
    pub end_char_offset: usize,
    pub count: i64,
  }

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#type-FunctionCoverage>
  #[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct FunctionCoverage {
    pub function_name: String,
    pub ranges: Vec<CoverageRange>,
    pub is_block_coverage: bool,
  }

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#type-ScriptCoverage>
  #[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct ScriptCoverage {
    pub script_id: String,
    pub url: String,
    pub functions: Vec<FunctionCoverage>,
  }

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#method-startPreciseCoverage>
  #[derive(Debug, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct StartPreciseCoverageArgs {
    pub call_count: bool,
    pub detailed: bool,
    pub allow_triggered_updates: bool,
  }
}
