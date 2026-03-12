// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::deno_json::BuildConfig;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_path_util::url_from_directory_path;
use std::collections::HashMap;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use deno_bundler::chunk::build_chunk_graph;
use deno_bundler::chunk::ChunkGraph;
use deno_bundler::config::EnvironmentId;
use deno_bundler::emit::emit_dev_chunk;
use deno_bundler::graph::BundlerGraph;
use deno_bundler::graph_builder::build_bundler_graph;

use crate::args::DevFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use deno_npm_installer::graph::NpmCachingStrategy;

/// Per-environment bundler state.
struct EnvironmentState {
  #[allow(dead_code)]
  name: String,
  bundler_graph: BundlerGraph,
  chunk_graph: ChunkGraph,
  /// Pre-emitted chunk code, keyed by chunk index.
  chunk_code: HashMap<u32, String>,
}

/// State shared between the HTTP server and the file watcher.
struct DevServerState {
  /// Build configuration from deno.json.
  build_config: BuildConfig,
  /// Root directory (where deno.json lives).
  root_dir: PathBuf,
  /// Per-environment state, keyed by environment name.
  environments: HashMap<String, EnvironmentState>,
}

pub async fn dev(
  flags: Arc<Flags>,
  dev_flags: DevFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  // Read build configuration from deno.json
  let deno_json = cli_options
    .start_dir
    .member_or_root_deno_json()
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "No deno.json found. \"deno dev\" requires a deno.json with a \"build\" section."
      )
    })?;

  let build_config = deno_json
    .to_build_config()
    .map_err(|e| {
      deno_core::anyhow::anyhow!("Failed to parse build config: {}", e)
    })?
    .ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "No \"build\" configuration found in deno.json. Add a \"build\" section with environments."
      )
    })?;

  let root_dir = deno_json
    .specifier
    .to_file_path()
    .map(|p| p.parent().unwrap().to_path_buf())
    .unwrap_or_else(|_| std::env::current_dir().unwrap());

  let env_count = build_config.environments.len();
  log::warn!(
    "{} dev server on {}:{} ({} environment{})",
    colors::green("Starting"),
    dev_flags.host,
    dev_flags.port,
    env_count,
    if env_count == 1 { "" } else { "s" },
  );

  for (name, env) in &build_config.environments {
    let runtime_str = env.runtime.as_deref().unwrap_or("default");
    log::warn!(
      "  {} {} [{}] ({} entr{})",
      colors::cyan("env"),
      name,
      runtime_str,
      env.entries.len(),
      if env.entries.len() == 1 { "y" } else { "ies" },
    );
  }

  for plugin in &build_config.plugins {
    log::warn!("  {} {}", colors::cyan("plugin"), plugin.specifier);
  }

  // Build module graphs for each environment.
  let graph_creator = factory.module_graph_creator().await?.clone();
  let root_url = url_from_directory_path(&root_dir)?;

  let mut environments = HashMap::default();
  let mut env_id_counter = 0u32;

  for (name, env_config) in &build_config.environments {
    let env_id = EnvironmentId::new(env_id_counter);
    env_id_counter += 1;

    // Resolve entry specifiers relative to root_dir.
    let entries: Vec<ModuleSpecifier> = env_config
      .entries
      .iter()
      .map(|entry| root_url.join(entry).unwrap_or_else(|_| {
        ModuleSpecifier::parse(&format!("file:///{}", entry)).unwrap()
      }))
      .collect();

    log::warn!(
      "\n  {} {} graph ({} entries)...",
      colors::green("Building"),
      name,
      entries.len(),
    );

    // Build deno_graph module graph.
    let deno_module_graph = graph_creator
      .create_graph(GraphKind::All, entries.clone(), NpmCachingStrategy::Eager)
      .await?;

    // Convert to bundler graph.
    let bundler_graph =
      build_bundler_graph(&deno_module_graph, env_id, &entries);

    log::warn!(
      "  {} {} ({} modules)",
      colors::green("Built"),
      name,
      bundler_graph.len(),
    );

    // Build chunk graph.
    let chunk_graph = build_chunk_graph(&bundler_graph);

    // Pre-emit all chunks.
    let mut chunk_code = HashMap::default();
    for chunk in chunk_graph.chunks() {
      let output = emit_dev_chunk(chunk, &bundler_graph, &chunk_graph);
      chunk_code.insert(chunk.id.0, output.code);
    }

    environments.insert(
      name.clone(),
      EnvironmentState {
        name: name.clone(),
        bundler_graph,
        chunk_graph,
        chunk_code,
      },
    );
  }

  let state = Arc::new(DevServerState {
    build_config,
    root_dir,
    environments,
  });

  // Start HTTP server
  let addr = format!("{}:{}", dev_flags.host, dev_flags.port);
  let listener = TcpListener::bind(&addr).await.map_err(|e| {
    deno_core::anyhow::anyhow!("Failed to bind to {}: {}", addr, e)
  })?;

  log::warn!(
    "\n  {} {}",
    colors::green("Local:"),
    colors::cyan(format!("http://{}:{}/", dev_flags.host, dev_flags.port)),
  );

  // Channel for signaling file changes
  let (_change_tx, _change_rx) = broadcast::channel::<Vec<PathBuf>>(16);

  // TODO: Start file watcher on root_dir
  // TODO: WebSocket upgrade for HMR

  // Accept connections
  loop {
    let (mut stream, _peer_addr) = listener.accept().await?;
    let state = state.clone();

    tokio::spawn(async move {
      let mut buf = vec![0u8; 4096];
      let n = match stream.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => return,
      };

      let request = String::from_utf8_lossy(&buf[..n]);
      let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

      let response = handle_request(path, &state);

      let _ = stream.write_all(response.as_bytes()).await;
      let _ = stream.flush().await;
    });
  }
}

fn handle_request(path: &str, state: &DevServerState) -> String {
  match path {
    "/" => serve_index(state),
    "/__hmr" => {
      // TODO: WebSocket upgrade
      let body = "WebSocket endpoint (not yet implemented)";
      format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
      )
    }
    _ => {
      // Try to match /<env>/<chunk_index>.js
      if let Some(body) = serve_chunk(path, state) {
        format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/javascript; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        )
      } else {
        let body = format!("Not found: {}", path);
        format!(
          "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        )
      }
    }
  }
}

/// Serve the index page with links to each environment's entry chunk.
fn serve_index(state: &DevServerState) -> String {
  let mut envs_html = String::new();
  for (name, env_state) in &state.environments {
    let env_config = state.build_config.environments.get(name);
    let runtime = env_config
      .and_then(|c| c.runtime.as_deref())
      .unwrap_or("default");

    envs_html.push_str(&format!(
      "<li><strong>{}</strong> [{}]: {} modules, {} chunks",
      name,
      runtime,
      env_state.bundler_graph.len(),
      env_state.chunk_graph.len(),
    ));

    // Link to entry chunks.
    for chunk_id in env_state.chunk_graph.entry_chunks() {
      envs_html.push_str(&format!(
        " | <a href=\"/{}/{}.js\">entry chunk {}</a>",
        name, chunk_id.0, chunk_id.0
      ));
    }
    envs_html.push_str("</li>");
  }

  let body = format!(
    r#"<!DOCTYPE html>
<html>
<head><title>Deno Dev Server</title></head>
<body>
  <h1>Deno Dev Server</h1>
  <h2>Environments</h2>
  <ul>{}</ul>
  <p>Root: {}</p>
  <script>
    // HMR client
    const ws = new WebSocket(`ws://${{location.host}}/__hmr`);
    ws.onmessage = (e) => {{
      const msg = JSON.parse(e.data);
      console.log('[HMR]', msg);
    }};
  </script>
</body>
</html>"#,
    envs_html,
    state.root_dir.display(),
  );

  format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
    body.len(),
    body
  )
}

/// Try to serve a chunk at /<env_name>/<chunk_id>.js
fn serve_chunk(path: &str, state: &DevServerState) -> Option<String> {
  let path = path.strip_prefix('/')?;
  let (env_name, rest) = path.split_once('/')?;
  let chunk_id_str = rest.strip_suffix(".js")?;
  let chunk_id: u32 = chunk_id_str.parse().ok()?;

  let env_state = state.environments.get(env_name)?;
  env_state.chunk_code.get(&chunk_id).cloned()
}
