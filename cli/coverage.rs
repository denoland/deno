// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::inspector::DenoInspector;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::v8;
use serde::Deserialize;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr;

pub struct CoverageCollector {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  response_map: HashMap<i32, oneshot::Sender<serde_json::Value>>,
  next_message_id: i32,
}

impl Deref for CoverageCollector {
  type Target = v8::inspector::V8InspectorSession;
  fn deref(&self) -> &Self::Target {
    &self.v8_session
  }
}

impl DerefMut for CoverageCollector {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.v8_session
  }
}

impl v8::inspector::ChannelImpl for CoverageCollector {
  fn base(&self) -> &v8::inspector::ChannelBase {
    &self.v8_channel
  }

  fn base_mut(&mut self) -> &mut v8::inspector::ChannelBase {
    &mut self.v8_channel
  }

  fn send_response(
    &mut self,
    call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let raw_message = message.unwrap().string().to_string();
    let message = serde_json::from_str(&raw_message).unwrap();
    self
      .response_map
      .remove(&call_id)
      .unwrap()
      .send(message)
      .unwrap();
  }

  fn send_notification(
    &mut self,
    _message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
  }

  fn flush_protocol_notifications(&mut self) {}
}

impl CoverageCollector {
  const CONTEXT_GROUP_ID: i32 = 1;

  pub fn new(inspector_ptr: *mut DenoInspector) -> Box<Self> {
    new_box_with(move |self_ptr| {
      let v8_channel = v8::inspector::ChannelBase::new::<Self>();
      let v8_session = unsafe { &mut *inspector_ptr }.connect(
        Self::CONTEXT_GROUP_ID,
        unsafe { &mut *self_ptr },
        v8::inspector::StringView::empty(),
      );

      let response_map = HashMap::new();
      let next_message_id = 0;

      Self {
        v8_channel,
        v8_session,
        response_map,
        next_message_id,
      }
    })
  }

  async fn post_message(
    &mut self,
    method: String,
    params: Option<serde_json::Value>,
  ) -> Result<serde_json::Value, AnyError> {
    let id = self.next_message_id;
    self.next_message_id += 1;

    let (sender, receiver) = oneshot::channel::<serde_json::Value>();
    self.response_map.insert(id, sender);

    let message = json!({
        "id": id,
        "method": method,
        "params": params,
    });

    let raw_message = serde_json::to_string(&message).unwrap();
    let raw_message = v8::inspector::StringView::from(raw_message.as_bytes());
    self.v8_session.dispatch_protocol_message(raw_message);

    let response = receiver.await.unwrap();
    if let Some(error) = response.get("error") {
      return Err(generic_error(format!("{}", error)));
    }

    let result = response.get("result").unwrap().clone();
    Ok(result)
  }

  pub async fn start_collecting(&mut self) -> Result<(), AnyError> {
    self
      .post_message("Debugger.enable".to_string(), None)
      .await?;

    self
      .post_message("Profiler.enable".to_string(), None)
      .await?;

    self
      .post_message(
        "Profiler.startPreciseCoverage".to_string(),
        Some(json!({"callCount": true, "detailed": true})),
      )
      .await?;

    Ok(())
  }

  pub async fn collect(&mut self) -> Result<Vec<Coverage>, AnyError> {
    let result = self
      .post_message("Profiler.takePreciseCoverage".to_string(), None)
      .await?;

    let take_coverage_result: TakePreciseCoverageResult =
      serde_json::from_value(result)?;

    let mut coverages: Vec<Coverage> = Vec::new();
    for script_coverage in take_coverage_result.result {
      let result = self
        .post_message(
          "Debugger.getScriptSource".to_string(),
          Some(json!({
              "scriptId": script_coverage.script_id,
          })),
        )
        .await?;

      let get_script_source_result: GetScriptSourceResult =
        serde_json::from_value(result)?;

      coverages.push(Coverage {
        script_coverage,
        script_source: get_script_source_result.script_source,
      })
    }

    Ok(coverages)
  }

  pub async fn stop_collecting(&mut self) -> Result<(), AnyError> {
    self
      .post_message("Profiler.stopPreciseCoverage".to_string(), None)
      .await?;
    self
      .post_message("Profiler.disable".to_string(), None)
      .await?;
    self
      .post_message("Debugger.disable".to_string(), None)
      .await?;

    Ok(())
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
  pub start_offset: usize,
  pub end_offset: usize,
  pub count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
  pub function_name: String,
  pub ranges: Vec<CoverageRange>,
  pub is_block_coverage: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
  pub script_id: String,
  pub url: String,
  pub functions: Vec<FunctionCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
  pub script_coverage: ScriptCoverage,
  pub script_source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageResult {
  result: Vec<ScriptCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetScriptSourceResult {
  pub script_source: String,
  pub bytecode: Option<String>,
}

pub struct PrettyCoverageReporter {
  quiet: bool,
}

// TODO(caspervonb) add support for lcov output (see geninfo(1) for format spec).
impl PrettyCoverageReporter {
  pub fn new(quiet: bool) -> PrettyCoverageReporter {
    PrettyCoverageReporter { quiet }
  }

  pub fn visit_coverage(&mut self, coverage: &Coverage) {
    let lines = coverage.script_source.lines().collect::<Vec<_>>();

    let mut covered_lines: Vec<usize> = Vec::new();
    let mut uncovered_lines: Vec<usize> = Vec::new();

    let mut line_start_offset = 0;
    for (index, line) in lines.iter().enumerate() {
      let line_end_offset = line_start_offset + line.len();

      let mut count = 0;
      for function in &coverage.script_coverage.functions {
        for range in &function.ranges {
          if range.start_offset <= line_start_offset
            && range.end_offset >= line_end_offset
          {
            if range.count == 0 {
              count = 0;
              break;
            }

            count += range.count;
          }
        }

        line_start_offset = line_end_offset;
      }
      if count > 0 {
        covered_lines.push(index);
      } else {
        uncovered_lines.push(index);
      }
    }

    if !self.quiet {
      print!("cover {} ... ", coverage.script_coverage.url);

      let line_coverage_ratio = covered_lines.len() as f32 / lines.len() as f32;
      let line_coverage = format!(
        "{:.3}% ({}/{})",
        line_coverage_ratio * 100.0,
        covered_lines.len(),
        lines.len()
      );

      if line_coverage_ratio >= 0.9 {
        println!("{}", colors::green(&line_coverage));
      } else if line_coverage_ratio >= 0.75 {
        println!("{}", colors::yellow(&line_coverage));
      } else {
        println!("{}", colors::red(&line_coverage));
      }

      for line_index in uncovered_lines {
        println!(
          "{:width$}{} {}",
          line_index + 1,
          colors::gray(" |"),
          colors::red(&lines[line_index]),
          width = 4
        );
      }
    }
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}

pub fn filter_script_coverages(
  coverages: Vec<Coverage>,
  test_file_url: Url,
  test_modules: Vec<Url>,
) -> Vec<Coverage> {
  coverages
    .into_iter()
    .filter(|e| {
      if let Ok(url) = Url::parse(&e.script_coverage.url) {
        if url.path().ends_with("__anonymous__") {
          return false;
        }

        if url == test_file_url {
          return false;
        }

        for test_module_url in &test_modules {
          if &url == test_module_url {
            return false;
          }
        }

        if let Ok(path) = url.to_file_path() {
          for test_module_url in &test_modules {
            if let Ok(test_module_path) = test_module_url.to_file_path() {
              if path.starts_with(test_module_path.parent().unwrap()) {
                return true;
              }
            }
          }
        }
      }

      false
    })
    .collect::<Vec<Coverage>>()
}
