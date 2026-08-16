// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

use deno_core::serde_json;

use super::cpuprof::CpuProfile;
use super::cpuprof::ProfileNode;

const FLAMEGRAPH_JS: &str = include_str!("flamegraph.js");

pub(crate) fn generate_flamegraph_svg(
  profile: &serde_json::Value,
  filepath: &std::path::Path,
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
    let mut frames = Vec::new();
    let mut current_id = sample_id;
    loop {
      if let Some(node) = node_map.get(&current_id) {
        let name = &node.call_frame.function_name;
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

  // Layout parameters (modern card-style flamegraph)
  let frame_height: usize = 20;
  let font_size: f64 = 12.0;
  let font_width: f64 = 0.6;
  let x_pad: usize = 16;
  let y_pad_top: usize = 60;
  let y_pad_bottom: usize = 40;
  let max_depth = root.max_depth();
  let image_width: usize = 1200;
  let image_height = y_pad_top + (max_depth + 1) * frame_height + y_pad_bottom;

  // Wall-clock duration of the profile, shown in the header.
  let duration_ms = (profile.end_time - profile.start_time) as f64 / 1000.0;

  let mut svg = String::new();

  // SVG header with onload for interactivity
  svg.push_str(&format!(
    r##"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" width="100%" height="100%" onload="init(evt)" viewBox="0 0 {image_width} {image_height}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:fg="http://github.com/nicholasgasior/gofern" style="min-height:100vh">
<style>
  text {{ font-family: ui-monospace, "SF Mono", "Menlo", "Consolas", monospace; font-size: {font_size}px; fill: #1e2329; }}
  #title {{ font-size: 15px; font-weight: 600; fill: #1f2328; }}
  #subtitle {{ text-anchor: middle; fill: #8b9098; font-size: 12px; }}
  #details {{ fill: #6a7079; font-size: 12px; }}
  #search {{ text-anchor: end; fill: #6a7079; opacity: 0.6; cursor: pointer; }}
  #search:hover, #search.show {{ opacity: 1; fill: #c026a8; }}
  #unzoom {{ text-anchor: end; fill: #6a7079; cursor: pointer; }}
  #matched {{ text-anchor: end; fill: #8b9098; }}
  #frames > * {{ cursor: pointer; }}
  #frames rect {{ stroke: #ffffff; stroke-width: 1; }}
  #frames > *:hover rect {{ stroke: #1f2328; stroke-width: 1; }}
  .hide {{ display: none; }}
  .parent {{ opacity: 0.45; }}
</style>
<script type="text/ecmascript"><![CDATA[
  "use strict";
  var nametype = "Function:";
  var fontsize = {font_size};
  var fontwidth = {font_width};
  var xpad = {x_pad};
  var inverted = false;
  var searchcolor = "rgb(192,38,168)";
  var fluiddrawing = true;
  var truncate_text_right = true;
{flamegraph_js}
]]></script>
<rect x="0" y="0" width="100%" height="100%" fill="#ffffff"/>
<circle cx="{dot1}" cy="20" r="6" fill="#ff5f56"/>
<circle cx="{dot2}" cy="20" r="6" fill="#ffbd2e"/>
<circle cx="{dot3}" cy="20" r="6" fill="#27c93f"/>
<text id="title" x="{title_x}" y="25">CPU Flamegraph</text>
<text id="subtitle" x="50%" y="25">{duration_ms:.0}ms &#183; {total_samples} samples</text>
<text id="unzoom" class="hide" x="{unzoom_x}" y="25">Reset Zoom</text>
<text id="search" x="{search_x}" y="25">Search</text>
<foreignObject id="invert_fo" x="{invert_x}" y="11" width="80" height="24">
<body xmlns="http://www.w3.org/1999/xhtml">
<label style="font-family:ui-monospace,Menlo,monospace;font-size:12px;color:#6a7079;cursor:pointer"><input type="checkbox" id="invert_cb" style="cursor:pointer"/> Invert</label>
</body>
</foreignObject>
<line x1="{x_pad}" y1="{divider_y}" x2="{divider_x2}" y2="{divider_y}" stroke="#ececf0" stroke-width="1"/>
<text id="details" x="{x_pad}" y="{details_y}">Frame width = time on CPU</text>
<text id="matched" class="hide" x="{search_x}" y="{details_y}">&#160;</text>
<g id="frames" total_samples="{total_samples}" width="{frames_width}" fg:max_depth="{max_depth}" fg:frame_height="{frame_height}" fg:y_pad_top="{y_pad_top}" fg:content_height="{image_height}">
"##,
    flamegraph_js = FLAMEGRAPH_JS,
    details_y = image_height - y_pad_bottom + 21,
    search_x = image_width - x_pad,
    unzoom_x = image_width - x_pad - 96,
    invert_x = image_width - x_pad - 200,
    dot1 = x_pad + 8,
    dot2 = x_pad + 26,
    dot3 = x_pad + 44,
    title_x = x_pad + 60,
    divider_y = y_pad_top - 10,
    divider_x2 = image_width - x_pad,
    frames_width = image_width - 2 * x_pad,
  ));

  // Flatten the tree into a list of frame rectangles using percentage-based
  // coordinates, with fg:x and fg:w storing raw sample counts for zoom.
  struct FlameLayout {
    max_depth: usize,
    total_samples: i32,
    frame_height: usize,
    y_pad_top: usize,
    image_width: usize,
    x_pad: usize,
    font_size: f64,
    font_width: f64,
  }

  fn collect_frames(
    node: &FlameNode,
    x_samples: i32,
    depth: usize,
    layout: &FlameLayout,
    svg: &mut String,
  ) {
    if node.name != "root" {
      let x_pct = 100.0 * x_samples as f64 / layout.total_samples as f64;
      let w_pct = 100.0 * node.total as f64 / layout.total_samples as f64;
      let y =
        layout.y_pad_top + (layout.max_depth - depth) * layout.frame_height;

      let (r, g, b) = flame_color(&node.name);
      let pct = (node.total as f64 / layout.total_samples as f64) * 100.0;

      svg.push_str(&format!(
        r#"<g><title>{name} ({samples} samples, {pct:.2}%)</title><rect x="{x_pct:.4}%" y="{y}" width="{w_pct:.4}%" height="{h}" rx="3" fill="rgb({r},{g},{b})" fg:x="{x_samples}" fg:w="{w_samples}" fg:y="{y}"/>"#,
        name = escape_xml(&node.name),
        samples = node.total,
        h = layout.frame_height - 2,
        w_samples = node.total,
      ));

      let chart_width = (layout.image_width - 2 * layout.x_pad) as f64;
      let avail_px = w_pct / 100.0 * chart_width - 6.0;
      let max_chars =
        (avail_px / (layout.font_size * layout.font_width)) as usize;
      if max_chars >= 3 {
        let label = if node.name.len() > max_chars {
          format!("{}..", &node.name[..max_chars.saturating_sub(2)])
        } else {
          node.name.clone()
        };
        let text_x_pct = x_pct + 100.0 * 3.0 / chart_width;
        svg.push_str(&format!(
          r#"<text x="{text_x_pct:.4}%" y="{ty}">{label}</text>"#,
          ty = y + layout.frame_height - 4,
          label = escape_xml(&label),
        ));
      }

      svg.push_str("</g>\n");
    }

    let mut child_x = x_samples;
    for child in &node.children {
      let child_depth = if node.name == "root" {
        depth
      } else {
        depth + 1
      };
      collect_frames(child, child_x, child_depth, layout, svg);
      child_x += child.total;
    }
  }

  let layout = FlameLayout {
    max_depth,
    total_samples,
    frame_height,
    y_pad_top,
    image_width,
    x_pad,
    font_size,
    font_width,
  };
  collect_frames(&root, 0, 0, &layout, &mut svg);

  svg.push_str("</g>\n</svg>\n");

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

/// Pick a vibrant color for a flamegraph frame.
///
/// Frames are colored from a fixed categorical palette (greens, golds,
/// oranges, blues) keyed by a hash of the function name, so the same
/// function always gets the same hue. A small per-name brightness jitter
/// keeps neighbouring tiles visually distinct.
fn flame_color(name: &str) -> (u8, u8, u8) {
  // Flat, saturated palette inspired by modern profiler UIs.
  const PALETTE: [(u8, u8, u8); 8] = [
    (0xE2, 0x8C, 0x2C), // orange
    (0x4F, 0xAA, 0x4F), // green
    (0xF1, 0xC6, 0x40), // gold
    (0x6F, 0xCB, 0x6B), // light green
    (0x4F, 0xA6, 0xD8), // blue
    (0xE6, 0xA1, 0x32), // amber
    (0x57, 0xB9, 0x5A), // medium green
    (0x84, 0xD4, 0x7B), // pale green
  ];

  // FNV-1a hash for a good spread across the palette.
  let mut hash: u32 = 2166136261;
  for b in name.bytes() {
    hash ^= b as u32;
    hash = hash.wrapping_mul(16777619);
  }

  let (r, g, b) = PALETTE[(hash as usize) % PALETTE.len()];
  // Subtle brightness jitter in the range [-10, +10].
  let jitter = ((hash >> 8) % 21) as i32 - 10;
  let adj = |c: u8| (c as i32 + jitter).clamp(0, 255) as u8;
  (adj(r), adj(g), adj(b))
}

fn escape_xml(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}
