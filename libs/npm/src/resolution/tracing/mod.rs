use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceGraphSnapshot {
  pub roots: BTreeMap<String, u32>,
  pub nodes: Vec<TraceNode>,
  pub path: TraceGraphPath,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceNodeDependency {
  pub kind: String,
  pub bare_specifier: String,
  pub name: String,
  pub version_req: String,
  pub peer_dep_version_req: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceNode {
  pub id: u32,
  pub resolved_id: String,
  pub children: BTreeMap<String, u32>,
  pub dependencies: Vec<TraceNodeDependency>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceGraphPath {
  pub specifier: String,
  pub node_id: u32,
  pub nv: String,
  pub previous: Option<Box<TraceGraphPath>>,
}

pub fn output(traces: &[TraceGraphSnapshot]) {
  let json = serde_json::to_string(traces).unwrap();
  let app_js = include_str!("./app.js");
  let app_css = include_str!("./app.css");
  let html = format!(
    "<!DOCTYPE html>
<style>
{app_css}
</style>
<div id=\"container\">
    <div id=\"slider\">
      <input type=\"range\" />
      <div id=\"stepText\">
      </div>
    </div>
    <div id=\"main\">
      <div id=\"graph\">
      </div>
      <div id=\"info\">
      </div>
    </div>
</div>
<script type=\"module\">
const rawTraces = {json};
{app_js}
</script>
"
  );
  let temp_file_path = std::env::temp_dir().join("deno-npm-trace.html");
  std::fs::write(&temp_file_path, html).unwrap();
  let url = format!(
    "file://{}",
    temp_file_path.to_string_lossy().replace('\\', "/")
  );
  #[allow(clippy::print_stderr)]
  {
    eprintln!(
      "\n==============\nTrace output ready! Please open your browser to: {}\n==============\n",
      url
    );
  }
}
