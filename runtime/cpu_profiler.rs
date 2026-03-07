// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;

/// Configuration for CPU profiling that can be passed to workers.
#[derive(Clone, Debug)]
pub struct CpuProfilerConfig {
  pub dir: PathBuf,
  pub name: Option<String>,
  pub interval: i32,
  pub md: bool,
  pub flamegraph: bool,
}

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
  generate_flamegraph: bool,
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

    // Generate flamegraph SVG if requested
    if inner.generate_flamegraph {
      let svg_filename = inner.filename.replace(".cpuprofile", ".svg");
      let svg_filepath = inner.dir.join(&svg_filename);
      if let Err(err) = generate_flamegraph_svg(profile, &svg_filepath) {
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
  interval: i32,
}

impl CpuProfiler {
  pub fn new(
    js_runtime: &mut JsRuntime,
    cpu_prof_dir: PathBuf,
    filename: String,
    interval: i32,
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
  #[allow(dead_code)]
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

  // Build parent map for walking up the call tree
  let mut parent_map: HashMap<i32, i32> = HashMap::new();
  for node in &profile.nodes {
    for &child_id in &node.children {
      parent_map.insert(child_id, node.id);
    }
  }

  let interval_us = interval as i64;

  fn make_key(node: &ProfileNode) -> String {
    format!(
      "{}:{}:{}",
      node.call_frame.function_name,
      node.call_frame.url,
      node.call_frame.line_number
    )
  }

  fn ensure_stats<'a>(
    stats: &'a mut HashMap<String, FunctionStats>,
    key: String,
    node: &ProfileNode,
  ) -> &'a mut FunctionStats {
    stats.entry(key).or_insert_with(|| FunctionStats {
      function_name: node.call_frame.function_name.clone(),
      url: node.call_frame.url.clone(),
      line_number: node.call_frame.line_number,
      self_time: 0,
      total_time: 0,
      self_samples: 0,
      total_samples: 0,
    })
  }

  // For each sample, credit self time to the sampled node,
  // and total time to every ancestor in the call stack.
  for &sample_id in &profile.samples {
    if let Some(node) = node_map.get(&sample_id) {
      // Self time: only the leaf node
      let key = make_key(node);
      let entry = ensure_stats(&mut function_stats, key, node);
      entry.self_time += interval_us;
      entry.self_samples += 1;

      // Total time: walk from the sampled node up to root
      let mut current_id = sample_id;
      // Use a set to avoid double-counting when the same function
      // appears multiple times in one stack (e.g. recursion)
      let mut visited_keys: std::collections::HashSet<String> =
        std::collections::HashSet::new();
      loop {
        if let Some(n) = node_map.get(&current_id) {
          let k = make_key(n);
          if visited_keys.insert(k.clone()) {
            let entry = ensure_stats(&mut function_stats, k, n);
            entry.total_time += interval_us;
            entry.total_samples += 1;
          }
        }
        if let Some(&parent_id) = parent_map.get(&current_id) {
          current_id = parent_id;
        } else {
          break;
        }
      }
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
  md.push_str("| Duration | Samples | Interval | Functions |\n");
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
    let total_pct = if total_self_time > 0 {
      (stats.total_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
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

  // Compute total time per node (self + all descendants) by walking the tree
  fn compute_node_total_time(
    node_id: i32,
    node_map: &HashMap<i32, &ProfileNode>,
    interval_us: i64,
    cache: &mut HashMap<i32, i64>,
  ) -> i64 {
    if let Some(&cached) = cache.get(&node_id) {
      return cached;
    }
    let Some(node) = node_map.get(&node_id) else {
      return 0;
    };
    let mut total = node.hit_count as i64 * interval_us;
    for &child_id in &node.children {
      total += compute_node_total_time(child_id, node_map, interval_us, cache);
    }
    cache.insert(node_id, total);
    total
  }

  let mut total_time_cache: HashMap<i32, i64> = HashMap::new();
  for node in &profile.nodes {
    compute_node_total_time(
      node.id,
      &node_map,
      interval_us,
      &mut total_time_cache,
    );
  }

  // Print call tree starting from root
  #[allow(clippy::too_many_arguments)]
  fn print_call_tree(
    md: &mut String,
    node_id: i32,
    node_map: &HashMap<i32, &ProfileNode>,
    node_total_times: &HashMap<i32, i64>,
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
          node_total_times,
          depth,
          total_self_time,
          interval_us,
          max_depth,
        );
      }
      return;
    }

    let self_time = node.hit_count as i64 * interval_us;
    let node_total_time =
      node_total_times.get(&node.id).copied().unwrap_or(self_time);
    let self_pct = if total_self_time > 0 {
      (self_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let self_time_ms = self_time as f64 / 1000.0;
    let total_pct = if total_self_time > 0 {
      (node_total_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let total_time_ms = node_total_time as f64 / 1000.0;

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
        node_total_times,
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
        &total_time_cache,
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
    let total_pct = if total_self_time > 0 {
      (stats.total_time as f64 / total_self_time as f64) * 100.0
    } else {
      0.0
    };
    let self_time_ms = stats.self_time as f64 / 1000.0;
    let total_time_ms = stats.total_time as f64 / 1000.0;
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
      total_pct,
      total_time_ms,
      stats.total_samples
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

fn generate_flamegraph_svg(
  profile: &serde_json::Value,
  filepath: &PathBuf,
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

  let node_map: HashMap<i32, &ProfileNode> =
    profile.nodes.iter().map(|n| (n.id, n)).collect();

  // Build parent map
  let mut parent_map: HashMap<i32, i32> = HashMap::new();
  for node in &profile.nodes {
    for &child_id in &node.children {
      parent_map.insert(child_id, node.id);
    }
  }

  // Build folded stacks from samples
  let mut stacks: HashMap<String, i32> = HashMap::new();
  for &sample_id in &profile.samples {
    // Walk from sample node to root to build the stack
    let mut frames = Vec::new();
    let mut current_id = sample_id;
    loop {
      if let Some(node) = node_map.get(&current_id) {
        let name = &node.call_frame.function_name;
        // Skip idle/root/program frames
        if !name.is_empty()
          && name != "(idle)"
          && name != "(root)"
          && name != "(program)"
        {
          let location = if node.call_frame.url.is_empty() {
            name.clone()
          } else {
            let short_url = node
              .call_frame
              .url
              .rsplit('/')
              .next()
              .unwrap_or(&node.call_frame.url);
            if node.call_frame.line_number >= 0 {
              format!(
                "{} ({}:{})",
                name,
                short_url,
                node.call_frame.line_number + 1
              )
            } else {
              format!("{} ({})", name, short_url)
            }
          };
          frames.push(location);
        }
      }
      if let Some(&parent_id) = parent_map.get(&current_id) {
        current_id = parent_id;
      } else {
        break;
      }
    }
    if frames.is_empty() {
      continue;
    }
    // Reverse so root is first
    frames.reverse();
    let stack = frames.join(";");
    *stacks.entry(stack).or_insert(0) += 1;
  }

  if stacks.is_empty() {
    return Ok(());
  }

  // Build a tree from the folded stacks for rendering
  let mut root = FlameNode::new("root".to_string());
  for (stack, count) in &stacks {
    let frames: Vec<&str> = stack.split(';').collect();
    root.insert(&frames, *count);
  }
  root.compute_total();

  let total_samples = root.total;
  if total_samples == 0 {
    return Ok(());
  }

  // SVG parameters
  let frame_height = 18;
  let font_size = 11;
  let x_pad = 10;
  let y_pad_top = 50;
  let y_pad_bottom = 10;
  let max_depth = root.max_depth();
  let svg_width = 1200;
  let svg_height = y_pad_top + (max_depth + 1) * frame_height + y_pad_bottom;
  let chart_width = svg_width - 2 * x_pad;

  let mut svg = String::new();

  // SVG header
  svg.push_str(&format!(
    r#"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
<style>
  .frame rect {{ stroke: #d0d0d0; stroke-width: 0.5; }}
  .frame text {{ font-family: monospace; font-size: {}px; fill: #333; }}
  .frame rect:hover {{ stroke: #333; stroke-width: 1; cursor: pointer; }}
  .title {{ font-family: sans-serif; font-size: 16px; font-weight: bold; fill: #333; }}
  .subtitle {{ font-family: sans-serif; font-size: 11px; fill: #888; }}
</style>
<rect width="100%" height="100%" fill="white" />
<text x="{}" y="20" class="title">CPU Flamegraph</text>
<text x="{}" y="36" class="subtitle">{} samples</text>
"#,
    svg_width, svg_height, font_size, x_pad, x_pad, total_samples
  ));

  // Render frames (bottom-up: root at bottom)
  #[allow(clippy::too_many_arguments)]
  fn render_node(
    svg: &mut String,
    node: &FlameNode,
    x: f64,
    width: f64,
    depth: usize,
    max_depth: usize,
    frame_height: usize,
    y_pad_top: usize,
    total_samples: i32,
    font_size: usize,
  ) {
    if node.name == "root" {
      // Don't render root, just its children
      let mut child_x = x;
      for child in &node.children {
        let child_width = (child.total as f64 / total_samples as f64) * width;
        if child_width >= 0.5 {
          render_node(
            svg,
            child,
            child_x,
            child_width,
            depth,
            max_depth,
            frame_height,
            y_pad_top,
            total_samples,
            font_size,
          );
          child_x += child_width;
        }
      }
      return;
    }

    // Flamegraph: root at bottom, leaves at top
    let y = y_pad_top + (max_depth - depth) * frame_height;

    let (r, g, b) = flame_color(&node.name);

    svg.push_str(&format!(
      r#"<g class="frame"><title>{} ({} samples, {:.1}%)</title><rect x="{:.1}" y="{}" width="{:.1}" height="{}" fill="rgb({},{},{})" />"#,
      escape_xml(&node.name),
      node.total,
      (node.total as f64 / total_samples as f64) * 100.0,
      x,
      y,
      width,
      frame_height - 1,
      r,
      g,
      b,
    ));

    // Only show text if there's enough room
    if width > 30.0 {
      let max_chars = (width as usize - 6) / (font_size * 6 / 10);
      let label = if node.name.len() > max_chars && max_chars > 3 {
        format!("{}..", &node.name[..max_chars - 2])
      } else {
        node.name.clone()
      };
      svg.push_str(&format!(
        r#"<text x="{:.1}" y="{}">{}</text>"#,
        x + 3.0,
        y + frame_height - 5,
        escape_xml(&label),
      ));
    }

    svg.push_str("</g>\n");

    // Render children
    let mut child_x = x;
    for child in &node.children {
      let child_width = (child.total as f64 / node.total as f64) * width;
      if child_width >= 0.5 {
        render_node(
          svg,
          child,
          child_x,
          child_width,
          depth + 1,
          max_depth,
          frame_height,
          y_pad_top,
          total_samples,
          font_size,
        );
        child_x += child_width;
      }
    }
  }

  render_node(
    &mut svg,
    &root,
    x_pad as f64,
    chart_width as f64,
    0,
    max_depth,
    frame_height,
    y_pad_top,
    total_samples,
    font_size,
  );

  svg.push_str("</svg>\n");

  let file = File::create(filepath)?;
  let mut out = BufWriter::new(file);
  out.write_all(svg.as_bytes())?;
  out.flush()?;

  Ok(())
}

#[derive(Debug)]
struct FlameNode {
  name: String,
  self_count: i32,
  total: i32,
  children: Vec<FlameNode>,
}

impl FlameNode {
  fn new(name: String) -> Self {
    Self {
      name,
      self_count: 0,
      total: 0,
      children: Vec::new(),
    }
  }

  fn insert(&mut self, frames: &[&str], count: i32) {
    if frames.is_empty() {
      self.self_count += count;
      return;
    }
    let child = self.children.iter_mut().find(|c| c.name == frames[0]);
    if let Some(child) = child {
      child.insert(&frames[1..], count);
    } else {
      let mut child = FlameNode::new(frames[0].to_string());
      child.insert(&frames[1..], count);
      self.children.push(child);
    }
  }

  fn compute_total(&mut self) -> i32 {
    let mut total = self.self_count;
    for child in &mut self.children {
      total += child.compute_total();
    }
    self.total = total;
    total
  }

  fn max_depth(&self) -> usize {
    if self.children.is_empty() {
      0
    } else {
      1 + self
        .children
        .iter()
        .map(|c| c.max_depth())
        .max()
        .unwrap_or(0)
    }
  }
}

/// Generate a warm color for flamegraph frames.
/// Uses a hash of the function name to vary hue slightly.
fn flame_color(name: &str) -> (u8, u8, u8) {
  let mut hash: u32 = 0;
  for b in name.bytes() {
    hash = hash.wrapping_mul(31).wrapping_add(b as u32);
  }
  // Warm colors: red-orange-yellow range
  let r = 200 + (hash % 55) as u8;
  let g = 80 + (hash.wrapping_shr(8) % 110) as u8;
  let b = 20 + (hash.wrapping_shr(16) % 40) as u8;
  (r, g, b)
}

fn escape_xml(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
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
