// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::deno_json::BuildConfig;
use deno_core::error::AnyError;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::args::DevFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;

/// State shared between the HTTP server and the file watcher.
struct DevServerState {
  /// Build configuration from deno.json.
  build_config: BuildConfig,
  /// Root directory (where deno.json lives).
  root_dir: PathBuf,
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
    .map_err(|e| deno_core::anyhow::anyhow!("Failed to parse build config: {}", e))?
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

  let state = Arc::new(DevServerState {
    build_config,
    root_dir,
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
  // TODO: Build per-environment module graphs
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
  // For now, serve a simple status page
  match path {
    "/" => {
      let mut envs_html = String::new();
      for (name, env) in &state.build_config.environments {
        let runtime = env.runtime.as_deref().unwrap_or("default");
        envs_html.push_str(&format!(
          "<li><strong>{}</strong> [{}]: {:?}</li>",
          name, runtime, env.entries,
        ));
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
    // HMR client will be injected here
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
      // TODO: Serve bundled modules
      let body = format!("Not found: {}", path);
      format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
      )
    }
  }
}
