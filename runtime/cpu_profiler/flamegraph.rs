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

  // Layout parameters (matching inferno/cargo-flamegraph style)
  let frame_height: usize = 16;
  let font_size: f64 = 12.0;
  let font_width: f64 = 0.59;
  let x_pad: usize = 10;
  let y_pad_top: usize = 66;
  let y_pad_bottom: usize = 40;
  let max_depth = root.max_depth();
  let image_width: usize = 1200;
  let image_height = y_pad_top + (max_depth + 1) * frame_height + y_pad_bottom;

  let mut svg = String::new();

  // SVG header with onload for interactivity
  svg.push_str(&format!(
    r##"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" width="100%" height="100%" onload="init(evt)" viewBox="0 0 {image_width} {image_height}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:fg="http://github.com/nicholasgasior/gofern" style="min-height:100vh">
<defs>
  <linearGradient id="background" y1="0" y2="1">
    <stop stop-color="#eeeeee" offset="5%"/>
    <stop stop-color="#eeeeb0" offset="95%"/>
  </linearGradient>
</defs>
<style>
  text {{ font-family: Verdana, sans-serif; font-size: {font_size}px; fill: rgb(0,0,0); }}
  #search {{ text-anchor: end; opacity: 0.1; cursor: pointer; }}
  #search:hover, #search.show {{ opacity: 1; }}
  #matched {{ text-anchor: end; }}
  #subtitle {{ text-anchor: middle; font-color: rgb(160,160,160); }}
  #unzoom {{ cursor: pointer; }}
  #frames > *:hover {{ stroke: black; stroke-width: 0.5; cursor: pointer; }}
  .hide {{ display: none; }}
  .parent {{ opacity: 0.5; }}
</style>
<script type="text/ecmascript"><![CDATA[
  "use strict";
  var nametype = "Function:";
  var fontsize = {font_size};
  var fontwidth = {font_width};
  var xpad = {x_pad};
  var inverted = false;
  var searchcolor = "rgb(230,0,230)";
  var fluiddrawing = true;
  var truncate_text_right = true;
{flamegraph_js}
]]></script>
<rect x="0" y="0" width="100%" height="100%" fill="url(#background)"/>
<text id="title" x="50%" y="24" text-anchor="middle" style="font-size:17px">CPU Flamegraph</text>
<text id="details" x="{x_pad}.0" y="{details_y}">&#160;</text>
<text id="unzoom" class="hide" x="{x_pad}.0" y="24">Reset Zoom</text>
<text id="search" x="{search_x}.0" y="24">Search</text>
<text id="matched" class="hide" x="{search_x}.0" y="{details_y}">&#160;</text>
<foreignObject x="{invert_x}" y="6" width="80" height="30">
<body xmlns="http://www.w3.org/1999/xhtml">
<label style="font-family:Verdana,sans-serif;font-size:12px;cursor:pointer"><input type="checkbox" id="invert_cb" style="cursor:pointer"/> Invert</label>
</body>
</foreignObject>
<g id="frames" total_samples="{total_samples}" width="{frames_width}" fg:max_depth="{max_depth}" fg:frame_height="{frame_height}" fg:y_pad_top="{y_pad_top}" fg:content_height="{image_height}">
"##,
    flamegraph_js = FLAMEGRAPH_JS,
    details_y = image_height - y_pad_bottom + 21,
    search_x = image_width - x_pad,
    invert_x = image_width / 2 + 80,
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
        r#"<g><title>{name} ({samples} samples, {pct:.2}%)</title><rect x="{x_pct:.4}%" y="{y}" width="{w_pct:.4}%" height="{h}" fill="rgb({r},{g},{b})" fg:x="{x_samples}" fg:w="{w_samples}" fg:y="{y}"/>"#,
        name = escape_xml(&node.name),
        samples = node.total,
        h = layout.frame_height - 1,
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
