// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

use deno_core::serde_json;
use serde::Deserialize;

// V8 CPU Profile data structures (shared with flamegraph module)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuProfile {
  pub nodes: Vec<ProfileNode>,
  pub start_time: i64,
  pub end_time: i64,
  #[serde(default)]
  pub samples: Vec<i32>,
  #[serde(default)]
  #[allow(dead_code, reason = "deserialized but not directly read")]
  pub time_deltas: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileNode {
  pub id: i32,
  pub call_frame: CallFrame,
  #[serde(default)]
  pub hit_count: i32,
  #[serde(default)]
  pub children: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CallFrame {
  pub function_name: String,
  #[allow(dead_code, reason = "deserialized but not directly read")]
  pub script_id: String,
  pub url: String,
  pub line_number: i32,
  #[allow(dead_code, reason = "deserialized but not directly read")]
  pub column_number: i32,
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

pub(crate) fn generate_markdown_report(
  profile: &serde_json::Value,
  filepath: &std::path::Path,
  interval_us: i64,
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
    duration_ms, total_samples, interval_us, total_functions
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
  #[allow(clippy::too_many_arguments, reason = "private code")]
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
    // Keep last two path segments for context (e.g. "src/foo.js" not just "foo.js")
    let short_url = {
      let mut parts = url.rsplitn(3, '/');
      let file = parts.next().unwrap_or(url);
      match parts.next() {
        Some(parent) => format!("{}/{}", parent, file),
        None => file.to_string(),
      }
    };
    if line_number >= 0 {
      format!("{}:{}", short_url, line_number + 1)
    } else {
      short_url
    }
  }
}
