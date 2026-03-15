// Copyright 2018-2026 the Deno authors. MIT license.

mod cpuprof;
mod flamegraph;

use std::cell::RefCell;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::SystemTime;

/// Configuration for CPU profiling that can be passed to workers.
#[derive(Clone, Debug)]
pub struct CpuProfilerConfig {
  pub dir: PathBuf,
  pub name: Option<String>,
  pub interval: u32,
  pub md: bool,
  pub flamegraph: bool,
}

/// Generate a default CPU profile filename using timestamp and PID.
/// Optionally includes a suffix (e.g. worker ID) for uniqueness.
pub fn cpu_prof_default_filename(suffix: Option<&str>) -> String {
  let timestamp = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis();
  let pid = std::process::id();
  match suffix {
    Some(s) => format!("CPU.{}.{}.{}.cpuprofile", timestamp, pid, s),
    None => format!("CPU.{}.{}.cpuprofile", timestamp, pid),
  }
}

/// Generate a CPU profile filename from config, appending a suffix for workers
/// when a custom name is provided.
pub fn cpu_prof_filename(
  config: &CpuProfilerConfig,
  suffix: Option<&str>,
) -> String {
  match (&config.name, suffix) {
    (Some(name), Some(s)) => {
      // Always make worker filenames unique even with custom names
      let stem = name.strip_suffix(".cpuprofile").unwrap_or(name);
      format!("{}.{}.cpuprofile", stem, s)
    }
    (Some(name), None) => name.clone(),
    (None, _) => cpu_prof_default_filename(suffix),
  }
}

use deno_core::InspectorSessionKind;
use deno_core::JsRuntime;
use deno_core::JsRuntimeInspector;
use deno_core::LocalInspectorSession;
use deno_core::SourceMapApplication;
use deno_core::SourceMapper;
use deno_core::error::CoreError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);

fn next_msg_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug)]
pub struct CpuProfilerInner {
  dir: PathBuf,
  filename: String,
  interval: u32,
  generate_md: bool,
  generate_flamegraph: bool,
  /// Set by `stop_profiling` and checked by `callback`. This is safe because
  /// the inspector processes messages on a single thread, so `stop_profiling`
  /// always sets this before the callback fires for that message.
  profile_msg_id: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct CpuProfilerState(Arc<Mutex<CpuProfilerInner>>);

impl CpuProfilerState {
  pub fn new(
    dir: PathBuf,
    filename: String,
    interval: u32,
    generate_md: bool,
    generate_flamegraph: bool,
  ) -> Self {
    Self(Arc::new(Mutex::new(CpuProfilerInner {
      dir,
      filename,
      interval,
      generate_md,
      generate_flamegraph,
      profile_msg_id: None,
    })))
  }

  pub fn callback(
    &self,
    msg: deno_core::InspectorMsg,
    source_mapper: &Rc<RefCell<SourceMapper>>,
  ) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      return;
    };
    let maybe_profile_msg_id = self.0.lock().profile_msg_id.as_ref().cloned();

    if let Some(profile_msg_id) = maybe_profile_msg_id
      && profile_msg_id == msg_id
    {
      let mut message: serde_json::Value =
        match serde_json::from_str(&msg.content) {
          Ok(v) => v,
          Err(err) => {
            log::error!("Failed to parse CPU profiler response: {:?}", err);
            return;
          }
        };

      // Extract the profile from result.profile and apply source maps
      if let Some(result) = message.get_mut("result") {
        if let Some(profile) = result.get_mut("profile") {
          apply_source_maps(profile, &mut source_mapper.borrow_mut());
          self.write_profile(profile);
        } else {
          log::error!("No 'profile' field in CPU profiler response");
        }
      } else {
        log::error!("No 'result' field in CPU profiler response");
      }
    }
  }

  fn write_profile(&self, profile: &serde_json::Value) {
    let inner = self.0.lock();
    let filepath = inner.dir.join(&inner.filename);

    let file = match File::create(&filepath) {
      Ok(f) => f,
      Err(err) => {
        log::error!(
          "Failed to create CPU profile file at {:?}, reason: {:?}",
          filepath,
          err
        );
        return;
      }
    };

    let mut out = BufWriter::new(file);
    let profile_str = match serde_json::to_string_pretty(&profile) {
      Ok(s) => s,
      Err(err) => {
        log::error!("Failed to serialize CPU profile: {:?}", err);
        return;
      }
    };

    if let Err(err) = out.write_all(profile_str.as_bytes()) {
      log::error!(
        "Failed to write CPU profile file at {:?}, reason: {:?}",
        filepath,
        err
      );
      return;
    }

    if let Err(err) = out.flush() {
      log::error!(
        "Failed to flush CPU profile file at {:?}, reason: {:?}",
        filepath,
        err
      );
    }

    // Generate markdown report if requested
    if inner.generate_md {
      let md_filename = inner.filename.replace(".cpuprofile", ".md");
      let md_filepath = inner.dir.join(&md_filename);
      if let Err(err) = cpuprof::generate_markdown_report(
        profile,
        &md_filepath,
        inner.interval as i64,
      ) {
        log::error!(
          "Failed to generate markdown report at {:?}, reason: {:?}",
          md_filepath,
          err
        );
      }
    }

    // Generate flamegraph SVG if requested
    if inner.generate_flamegraph {
      let svg_filename = inner.filename.replace(".cpuprofile", ".svg");
      let svg_filepath = inner.dir.join(&svg_filename);
      if let Err(err) =
        flamegraph::generate_flamegraph_svg(profile, &svg_filepath)
      {
        log::error!(
          "Failed to generate flamegraph at {:?}, reason: {:?}",
          svg_filepath,
          err
        );
      }
    }
  }
}

pub struct CpuProfiler {
  pub state: CpuProfilerState,
  session: LocalInspectorSession,
  interval: u32,
}

impl CpuProfiler {
  pub fn new(
    js_runtime: &mut JsRuntime,
    cpu_prof_dir: PathBuf,
    filename: String,
    interval: u32,
    generate_md: bool,
    generate_flamegraph: bool,
  ) -> Self {
    let state = CpuProfilerState::new(
      cpu_prof_dir,
      filename,
      interval,
      generate_md,
      generate_flamegraph,
    );

    js_runtime.maybe_init_inspector();
    let insp = js_runtime.inspector();
    let source_mapper = js_runtime.source_mapper();

    let s = state.clone();
    let callback =
      Box::new(move |message| s.clone().callback(message, &source_mapper));
    let session = JsRuntimeInspector::create_local_session(
      insp,
      callback,
      InspectorSessionKind::NonBlocking {
        wait_for_disconnect: false,
      },
    );

    Self {
      state,
      session,
      interval,
    }
  }

  pub fn start_profiling(&mut self) {
    self
      .session
      .post_message::<()>(next_msg_id(), "Profiler.enable", None);

    // Note: Profiler.setSamplingInterval must be called before Profiler.start
    // but after Profiler.enable
    if self.interval != 1000 {
      self.session.post_message(
        next_msg_id(),
        "Profiler.setSamplingInterval",
        Some(cdp::SetSamplingIntervalArgs {
          interval: self.interval,
        }),
      );
    }

    self
      .session
      .post_message::<()>(next_msg_id(), "Profiler.start", None);

    log::debug!("CPU profiler started with interval: {}us", self.interval);
  }

  // fs::create_dir_all is on the Deno project's clippy disallowed list
  // (preferring the sys_traits abstraction), but the CPU profiler runs in the
  // runtime crate where using std::fs directly is acceptable.
  pub fn stop_profiling(&mut self) -> Result<(), CoreError> {
    #[allow(
      clippy::disallowed_methods,
      reason = "always using real fs with profiler"
    )]
    fs::create_dir_all(&self.state.0.lock().dir)?;

    let msg_id = next_msg_id();
    self.state.0.lock().profile_msg_id.replace(msg_id);

    self
      .session
      .post_message::<()>(msg_id, "Profiler.stop", None);

    log::debug!("CPU profiler stopped");

    Ok(())
  }
}

/// Apply source maps to all call frames in the CPU profile.
/// V8's profiler reports positions in transpiled JavaScript; this maps them
/// back to the original TypeScript (or other) source locations.
fn apply_source_maps(
  profile: &mut serde_json::Value,
  source_mapper: &mut SourceMapper,
) {
  let Some(nodes) = profile.get_mut("nodes").and_then(|n| n.as_array_mut())
  else {
    return;
  };

  for node in nodes {
    let Some(call_frame) = node.get_mut("callFrame") else {
      continue;
    };

    let Some(url) = call_frame.get("url").and_then(|u| u.as_str()) else {
      continue;
    };
    if url.is_empty() {
      continue;
    }

    // V8 profile line/column numbers are 0-based;
    // SourceMapper::apply_source_map expects 1-based.
    let line_number = call_frame
      .get("lineNumber")
      .and_then(|l| l.as_i64())
      .unwrap_or(-1);
    let column_number = call_frame
      .get("columnNumber")
      .and_then(|c| c.as_i64())
      .unwrap_or(-1);

    if line_number < 0 || column_number < 0 {
      continue;
    }

    let url_str = url.to_string();
    match source_mapper.apply_source_map(
      &url_str,
      (line_number + 1) as u32,
      (column_number + 1) as u32,
    ) {
      SourceMapApplication::LineAndColumn {
        line_number: new_line,
        column_number: new_col,
      } => {
        // Convert back to 0-based for the profile
        call_frame["lineNumber"] = serde_json::Value::from(new_line as i64 - 1);
        call_frame["columnNumber"] =
          serde_json::Value::from(new_col as i64 - 1);
      }
      SourceMapApplication::LineAndColumnAndFileName {
        file_name,
        line_number: new_line,
        column_number: new_col,
      } => {
        call_frame["url"] = serde_json::Value::from(file_name);
        call_frame["lineNumber"] = serde_json::Value::from(new_line as i64 - 1);
        call_frame["columnNumber"] =
          serde_json::Value::from(new_col as i64 - 1);
      }
      SourceMapApplication::Unchanged => {}
    }
  }
}

mod cdp {
  use serde::Serialize;

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#method-setSamplingInterval>
  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct SetSamplingIntervalArgs {
    pub interval: u32,
  }
}
