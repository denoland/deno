// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
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
use serde::Deserialize;

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);

fn next_msg_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug)]
pub struct CpuProfilerInner {
  dir: PathBuf,
  filename: String,
  interval: i32,
  generate_md: bool,
  profile_msg_id: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct CpuProfilerState(Arc<Mutex<CpuProfilerInner>>);

impl CpuProfilerState {
  pub fn new(
    dir: PathBuf,
    filename: String,
    interval: i32,
    generate_md: bool,
  ) -> Self {
    Self(Arc::new(Mutex::new(CpuProfilerInner {
      dir,
      filename,
      interval,
      generate_md,
      profile_msg_id: None,
    })))
  }

  pub fn callback(&self, msg: deno_core::InspectorMsg) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      return;
    };
    let maybe_profile_msg_id = self.0.lock().profile_msg_id.as_ref().cloned();

    if let Some(profile_msg_id) = maybe_profile_msg_id
      && profile_msg_id == msg_id
    {
      let message: serde_json::Value = match serde_json::from_str(&msg.content)
      {
        Ok(v) => v,
        Err(err) => {
          log::error!("Failed to parse CPU profiler response: {:?}", err);
          return;
        }
      };

      // Extract the profile from result.profile
      if let Some(result) = message.get("result") {
        if let Some(profile) = result.get("profile") {
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
      if let Err(err) =
        generate_markdown_report(profile, &md_filepath, inner.interval)
      {
        log::error!(
          "Failed to generate markdown report at {:?}, reason: {:?}",
          md_filepath,
          err
        );
      }
    }
  }
}

pub struct CpuProfiler {
  pub state: CpuProfilerState,
  session: LocalInspectorSession,
  interval: i32,
}

impl CpuProfiler {
  pub fn new(
    js_runtime: &mut JsRuntime,
    cpu_prof_dir: PathBuf,
    filename: String,
    interval: i32,
    generate_md: bool,
  ) -> Self {
    let state =
      CpuProfilerState::new(cpu_prof_dir, filename, interval, generate_md);

    js_runtime.maybe_init_inspector();
    let insp = js_runtime.inspector();

    let s = state.clone();
    let callback = Box::new(move |message| s.clone().callback(message));
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

  #[allow(clippy::disallowed_methods)]
  pub fn stop_profiling(&mut self) -> Result<(), CoreError> {
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

// V8 CPU Profile data structures
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CpuProfile {
  nodes: Vec<ProfileNode>,
  start_time: i64,
  end_time: i64,
  #[serde(default)]
  samples: Vec<i32>,
  #[serde(default)]
  time_deltas: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileNode {
  id: i32,
  call_frame: CallFrame,
  #[serde(default)]
  hit_count: i32,
  #[serde(default)]
  children: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallFrame {
  function_name: String,
  #[allow(dead_code)]
  script_id: String,
  url: String,
  line_number: i32,
  #[allow(dead_code)]
  column_number: i32,
}

#[derive(Debug, Clone)]
struct FunctionStats {
  function_name: String,
  url: String,
  line_number: i32,
  self_time: i64,
  total_time: i64,
  self_samples: i32,
  total_samples: i32,
}

fn generate_markdown_report(
  profile: &serde_json::Value,
  filepath: &PathBuf,
  interval: i32,
) -> std::io::Result<()> {
  let profile: CpuProfile = match serde_json::from_value(profile.clone()) {
    Ok(p) => p,
    Err(err) => {
      return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("Failed to parse profile: {}", err),
      ));
    }
  };

  let mut md = String::new();

  // Calculate stats
  let duration_us = profile.end_time - profile.start_time;
  let duration_ms = duration_us as f64 / 1000.0;
  let total_samples = profile.samples.len();
  let total_functions = profile.nodes.len();

  // Build node map
  let node_map: HashMap<i32, &ProfileNode> =
    profile.nodes.iter().map(|n| (n.id, n)).collect();

  // Calculate self and total times for each function
  let mut function_stats: HashMap<String, FunctionStats> = HashMap::new();

  // Calculate self time from samples
  let interval_us = interval as i64;
  for &sample_id in &profile.samples {
    if let Some(node) = node_map.get(&sample_id) {
      let key = format!(
        "{}:{}:{}",
        node.call_frame.function_name,
        node.call_frame.url,
        node.call_frame.line_number
      );
      let entry = function_stats.entry(key).or_insert_with(|| FunctionStats {
        function_name: node.call_frame.function_name.clone(),
        url: node.call_frame.url.clone(),
        line_number: node.call_frame.line_number,
        self_time: 0,
        total_time: 0,
        self_samples: 0,
        total_samples: 0,
      });
      entry.self_time += interval_us;
      entry.self_samples += 1;
    }
  }

  // Calculate total time (self time + time in children)
  // This is a simplified calculation - for accurate total time we'd need to walk the call tree
  for node in &profile.nodes {
    let key = format!(
      "{}:{}:{}",
      node.call_frame.function_name,
      node.call_frame.url,
      node.call_frame.line_number
    );
    if let Some(stats) = function_stats.get_mut(&key) {
      stats.total_time = stats.self_time;
      stats.total_samples = stats.self_samples;
    }
  }

  // Sort by self time
  let mut sorted_stats: Vec<_> = function_stats.values().cloned().collect();
  sorted_stats.sort_by(|a, b| b.self_time.cmp(&a.self_time));

  // Filter out idle/root
  let sorted_stats: Vec<_> = sorted_stats
    .into_iter()
    .filter(|s| {
      !s.function_name.is_empty()
        && s.function_name != "(idle)"
        && s.function_name != "(root)"
        && s.function_name != "(program)"
    })
    .collect();

  let total_self_time: i64 = sorted_stats.iter().map(|s| s.self_time).sum();

  // Header
  md.push_str("# CPU Profile\n\n");

  // Summary
  md.push_str(&format!("| Duration | Samples | Interval | Functions |\n"));
  md.push_str("| --- | --- | --- | --- |\n");
  md.push_str(&format!(
    "| {:.2}ms | {} | {}us | {} |\n\n",
    duration_ms, total_samples, interval, total_functions
  ));

  // Top 10 summary
  md.push_str("**Top 10:** ");
  let top10: Vec<_> = sorted_stats.iter().take(10).collect();
  let top10_strs: Vec<String> = top10
    .iter()
    .map(|s| {
      let pct = if total_self_time > 0 {
        (s.self_time as f64 / total_self_time as f64) * 100.0
      } else {
        0.0
      };
      format!("`{}` {:.1}%", display_function_name(&s.function_name), pct)
    })
    .collect();
  md.push_str(&top10_strs.join(", "));
  md.push_str("\n\n");

  // Hot Functions (Self Time)
  md.push_str("## Hot Functions (Self Time)\n\n");
  md.push_str("| Self% | Self | Total% | Total | Function | Location |\n");
  md.push_str("| ---: | ---: | ---: | ---: | --- | --- |\n");

  for stats in sorted_stats.iter().take(20) {
    let self_pct = if total_self_time > 0 {
      (stats.self_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let total_pct = self_pct; // Simplified - same as self for now
    let self_time_ms = stats.self_time as f64 / 1000.0;
    let total_time_ms = stats.total_time as f64 / 1000.0;

    let location = format_location(&stats.url, stats.line_number);
    let func_name = display_function_name(&stats.function_name);

    md.push_str(&format!(
      "| {:.1}% | {:.2}ms | {:.1}% | {:.2}ms | `{}` | {} |\n",
      self_pct, self_time_ms, total_pct, total_time_ms, func_name, location
    ));
  }
  md.push('\n');

  // Call Tree (simplified - show top-level calls)
  md.push_str("## Call Tree (Total Time)\n\n");
  md.push_str("| Total% | Total | Self% | Self | Function | Location |\n");
  md.push_str("| ---: | ---: | ---: | ---: | --- | --- |\n");

  // Find root nodes (nodes with no parents pointing to them)
  let mut has_parent: std::collections::HashSet<i32> =
    std::collections::HashSet::new();
  for node in &profile.nodes {
    for &child_id in &node.children {
      has_parent.insert(child_id);
    }
  }

  // Print call tree starting from root
  fn print_call_tree(
    md: &mut String,
    node_id: i32,
    node_map: &HashMap<i32, &ProfileNode>,
    depth: usize,
    total_self_time: i64,
    interval_us: i64,
    max_depth: usize,
  ) {
    if depth > max_depth {
      return;
    }
    let Some(node) = node_map.get(&node_id) else {
      return;
    };

    let func_name = &node.call_frame.function_name;
    if func_name == "(idle)"
      || func_name == "(root)"
      || func_name == "(program)"
    {
      // Skip root/idle but process children
      for &child_id in &node.children {
        print_call_tree(
          md,
          child_id,
          node_map,
          depth,
          total_self_time,
          interval_us,
          max_depth,
        );
      }
      return;
    }

    let self_time = node.hit_count as i64 * interval_us;
    let self_pct = if total_self_time > 0 {
      (self_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let self_time_ms = self_time as f64 / 1000.0;

    // For total time, we'd need to sum all descendants - simplified here
    let total_pct = self_pct;
    let total_time_ms = self_time_ms;

    let indent = "  ".repeat(depth);
    let location =
      format_location(&node.call_frame.url, node.call_frame.line_number);
    let func_display = display_function_name(func_name);

    md.push_str(&format!(
      "| {:.1}% | {:.2}ms | {:.1}% | {:.2}ms | {}`{}` | {} |\n",
      total_pct,
      total_time_ms,
      self_pct,
      self_time_ms,
      indent,
      func_display,
      location
    ));

    for &child_id in &node.children {
      print_call_tree(
        md,
        child_id,
        node_map,
        depth + 1,
        total_self_time,
        interval_us,
        max_depth,
      );
    }
  }

  // Find root and print tree
  for node in &profile.nodes {
    if !has_parent.contains(&node.id) {
      print_call_tree(
        &mut md,
        node.id,
        &node_map,
        0,
        total_self_time,
        interval_us,
        6, // max depth
      );
    }
  }
  md.push('\n');

  // Function Details
  md.push_str("## Function Details\n\n");

  for stats in sorted_stats.iter().take(10) {
    let self_pct = if total_self_time > 0 {
      (stats.self_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let self_time_ms = stats.self_time as f64 / 1000.0;
    let location = format_location(&stats.url, stats.line_number);

    md.push_str(&format!(
      "### `{}`\n",
      display_function_name(&stats.function_name)
    ));
    md.push_str(&format!(
      "{} | Self: {:.1}% ({:.2}ms) | Total: {:.1}% ({:.2}ms) | Samples: {}\n\n",
      location,
      self_pct,
      self_time_ms,
      self_pct,
      self_time_ms,
      stats.self_samples
    ));
  }

  // Write to file
  let file = File::create(filepath)?;
  let mut out = BufWriter::new(file);
  out.write_all(md.as_bytes())?;
  out.flush()?;

  Ok(())
}

fn display_function_name(name: &str) -> &str {
  if name.is_empty() { "(anonymous)" } else { name }
}

fn format_location(url: &str, line_number: i32) -> String {
  if url.is_empty() {
    "[native code]".to_string()
  } else {
    // Shorten the URL for display
    let short_url = url.rsplit('/').next().unwrap_or(url);
    if line_number >= 0 {
      format!("{}:{}", short_url, line_number + 1)
    } else {
      short_url.to_string()
    }
  }
}

mod cdp {
  use serde::Serialize;

  /// <https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#method-setSamplingInterval>
  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct SetSamplingIntervalArgs {
    pub interval: i32,
  }
}
