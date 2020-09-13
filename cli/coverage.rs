// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::file_fetcher::SourceFile;
use crate::global_state::GlobalState;
use crate::inspector::DenoInspector;
use crate::permissions::Permissions;
use deno_core::v8;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde::Deserialize;
use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr;
use std::sync::Arc;
use url::Url;

pub struct CoverageCollector {
  v8_channel: v8::inspector::ChannelBase,
  v8_session: v8::UniqueRef<v8::inspector::V8InspectorSession>,
  response_queue: VecDeque<String>,
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
    _call_id: i32,
    message: v8::UniquePtr<v8::inspector::StringBuffer>,
  ) {
    let message = message.unwrap().string().to_string();
    self.response_queue.push_back(message);
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

      let response_queue = VecDeque::with_capacity(10);

      Self {
        v8_channel,
        v8_session,
        response_queue,
      }
    })
  }

  async fn dispatch(&mut self, message: String) -> Result<String, ErrBox> {
    let message = v8::inspector::StringView::from(message.as_bytes());
    self.v8_session.dispatch_protocol_message(message);

    let response = self.response_queue.pop_back();
    Ok(response.unwrap())
  }

  pub async fn start_collecting(&mut self) -> Result<(), ErrBox> {
    self
      .dispatch(r#"{"id":1,"method":"Runtime.enable"}"#.into())
      .await?;
    self
      .dispatch(r#"{"id":2,"method":"Profiler.enable"}"#.into())
      .await?;

    self
        .dispatch(r#"{"id":3,"method":"Profiler.startPreciseCoverage", "params": {"callCount": true, "detailed": true}}"#.into())
        .await?;

    Ok(())
  }

  pub async fn take_precise_coverage(
    &mut self,
  ) -> Result<Vec<ScriptCoverage>, ErrBox> {
    let response = self
      .dispatch(r#"{"id":4,"method":"Profiler.takePreciseCoverage" }"#.into())
      .await?;

    let coverage_result: TakePreciseCoverageResponse =
      serde_json::from_str(&response).unwrap();

    Ok(coverage_result.result.result)
  }

  pub async fn stop_collecting(&mut self) -> Result<(), ErrBox> {
    self
      .dispatch(r#"{"id":5,"method":"Profiler.stopPreciseCoverage"}"#.into())
      .await?;

    self
      .dispatch(r#"{"id":6,"method":"Profiler.disable"}"#.into())
      .await?;

    self
      .dispatch(r#"{"id":7,"method":"Runtime.disable"}"#.into())
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
struct TakePreciseCoverageResult {
  result: Vec<ScriptCoverage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakePreciseCoverageResponse {
  id: usize,
  result: TakePreciseCoverageResult,
}

pub struct PrettyCoverageReporter {
  coverages: Vec<ScriptCoverage>,
  global_state: Arc<GlobalState>,
}

// TODO(caspervonb) add support for lcov output (see geninfo(1) for format spec).
impl PrettyCoverageReporter {
  pub fn new(
    global_state: Arc<GlobalState>,
    coverages: Vec<ScriptCoverage>,
  ) -> PrettyCoverageReporter {
    PrettyCoverageReporter {
      global_state,
      coverages,
    }
  }

  pub fn get_report(&self) -> String {
    let mut report = String::from("test coverage:\n");

    for script_coverage in &self.coverages {
      if let Some(script_report) = self.get_script_report(script_coverage) {
        report.push_str(&format!("{}\n", script_report))
      }
    }

    report
  }

  fn get_source_file_for_script(
    &self,
    script_coverage: &ScriptCoverage,
  ) -> Option<SourceFile> {
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&script_coverage.url).ok()?;

    let maybe_source_file = self
      .global_state
      .ts_compiler
      .get_compiled_source_file(&module_specifier.as_url())
      .or_else(|_| {
        self
          .global_state
          .file_fetcher
          .fetch_cached_source_file(&module_specifier, Permissions::allow_all())
          .ok_or_else(|| ErrBox::error("unable to fetch source file"))
      })
      .ok();

    maybe_source_file
  }

  fn get_script_report(
    &self,
    script_coverage: &ScriptCoverage,
  ) -> Option<String> {
    let source_file = match self.get_source_file_for_script(script_coverage) {
      Some(sf) => sf,
      None => return None,
    };

    let mut total_lines = 0;
    let mut covered_lines = 0;

    let mut line_offset = 0;
    let source_string = source_file.source_code.to_string().unwrap();

    for line in source_string.lines() {
      let line_start_offset = line_offset;
      let line_end_offset = line_start_offset + line.len();

      let mut count = 0;
      for function in &script_coverage.functions {
        for range in &function.ranges {
          if range.start_offset <= line_start_offset
            && range.end_offset >= line_end_offset
          {
            count += range.count;
            if range.count == 0 {
              count = 0;
              break;
            }
          }
        }
      }

      if count > 0 {
        covered_lines += 1;
      }

      total_lines += 1;
      line_offset += line.len();
    }

    let line_ratio = covered_lines as f32 / total_lines as f32;
    let line_coverage = format!("{:.3}%", line_ratio * 100.0);

    let line = if line_ratio >= 0.9 {
      format!(
        "{} {}",
        source_file.url.to_string(),
        colors::green(&line_coverage)
      )
    } else if line_ratio >= 0.75 {
      format!(
        "{} {}",
        source_file.url.to_string(),
        colors::yellow(&line_coverage)
      )
    } else {
      format!(
        "{} {}",
        source_file.url.to_string(),
        colors::red(&line_coverage)
      )
    };

    Some(line)
  }
}

fn new_box_with<T>(new_fn: impl FnOnce(*mut T) -> T) -> Box<T> {
  let b = Box::new(MaybeUninit::<T>::uninit());
  let p = Box::into_raw(b) as *mut T;
  unsafe { ptr::write(p, new_fn(p)) };
  unsafe { Box::from_raw(p) }
}

pub fn filter_script_coverages(
  coverages: Vec<ScriptCoverage>,
  test_file_url: Url,
  test_modules: Vec<Url>,
) -> Vec<ScriptCoverage> {
  coverages
    .into_iter()
    .filter(|e| {
      if let Ok(url) = Url::parse(&e.url) {
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
    .collect::<Vec<ScriptCoverage>>()
}
