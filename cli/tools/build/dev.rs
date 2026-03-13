// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use deno_config::deno_json::BuildConfig;
use deno_core::serde_json;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_path_util::url_from_directory_path;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

use deno_bundler::analyze::analyze_graph;
use deno_bundler::asset_discovery::discover_assets;
use deno_bundler::chunk::build_chunk_graph;
use deno_bundler::chunk::ChunkGraph;
use deno_bundler::config::EnvironmentId;
use deno_bundler::emit::emit_dev_chunk;
use deno_bundler::emit::emit_hmr_update;
use deno_bundler::graph::BundlerGraph;
use deno_bundler::graph_builder::build_bundler_graph;
use deno_bundler::js::hmr::compute_hmr_boundaries;
use deno_bundler::js::hmr::HmrBoundaryResult;
use deno_bundler::js::hmr::HmrGraph;
use deno_bundler::plugin::create_default_plugin_driver;
use deno_bundler::plugin::create_plugin_driver;
use deno_bundler::plugin::PluginDriver;
use deno_bundler::process::transform_modules;

use crate::args::DevFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use deno_npm_installer::graph::NpmCachingStrategy;

use super::plugin_host;

/// HMR message types sent to browser clients.
#[derive(Debug, Clone)]
enum HmrMessage {
  Update {
    boundaries: Vec<u32>,
    invalidated: Vec<u32>,
    modules: HashMap<u32, String>,
  },
  FullReload,
  #[allow(dead_code)]
  Error(String),
}

impl HmrMessage {
  fn to_json(&self) -> String {
    match self {
      HmrMessage::Update {
        boundaries,
        invalidated,
        modules,
      } => {
        let boundaries_json: Vec<String> =
          boundaries.iter().map(|b| b.to_string()).collect();
        let invalidated_json: Vec<String> =
          invalidated.iter().map(|i| i.to_string()).collect();
        let mut modules_json = String::from("{");
        for (i, (mid, code)) in modules.iter().enumerate() {
          if i > 0 {
            modules_json.push(',');
          }
          modules_json.push_str(&format!(
            "\"{}\":{}",
            mid,
            serde_json::to_string(code).unwrap_or_default()
          ));
        }
        modules_json.push('}');
        format!(
          r#"{{"type":"update","boundaries":[{}],"invalidated":[{}],"modules":{}}}"#,
          boundaries_json.join(","),
          invalidated_json.join(","),
          modules_json,
        )
      }
      HmrMessage::FullReload => r#"{"type":"full-reload"}"#.to_string(),
      HmrMessage::Error(msg) => {
        format!(
          r#"{{"type":"error","message":{}}}"#,
          serde_json::to_string(msg).unwrap_or_default()
        )
      }
    }
  }
}

/// Per-environment bundler state.
struct EnvironmentState {
  #[allow(dead_code)]
  name: String,
  bundler_graph: BundlerGraph,
  chunk_graph: ChunkGraph,
  chunk_code: HashMap<u32, String>,
}

/// State shared between the HTTP server and the file watcher.
struct DevServerState {
  build_config: BuildConfig,
  root_dir: PathBuf,
  environments: HashMap<String, EnvironmentState>,
}

/// HMR graph adapter for BundlerGraph.
struct BundlerHmrGraph<'a> {
  graph: &'a BundlerGraph,
}

impl HmrGraph for BundlerHmrGraph<'_> {
  fn self_accepts(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .graph
      .get_module(specifier)
      .and_then(|m| m.hmr_info.as_ref())
      .map(|h| h.self_accepts)
      .unwrap_or(false)
  }

  fn declines(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .graph
      .get_module(specifier)
      .and_then(|m| m.hmr_info.as_ref())
      .map(|h| h.declines)
      .unwrap_or(false)
  }

  fn accepts_dep(
    &self,
    importer: &ModuleSpecifier,
    dep: &ModuleSpecifier,
  ) -> bool {
    let Some(m) = self.graph.get_module(importer) else {
      return false;
    };
    let Some(hmr) = &m.hmr_info else {
      return false;
    };
    // Check if any accepted dep matches.
    hmr.accepted_deps.iter().any(|dep_specifier| {
      m.dependencies
        .iter()
        .any(|d| d.specifier == *dep_specifier && &d.resolved == dep)
    })
  }

  fn importers(&self, specifier: &ModuleSpecifier) -> Vec<ModuleSpecifier> {
    let mut result = Vec::new();
    for m in self.graph.modules() {
      for dep in &m.dependencies {
        if &dep.resolved == specifier {
          result.push(m.specifier.clone());
        }
      }
    }
    result
  }

  fn is_entry(&self, specifier: &ModuleSpecifier) -> bool {
    self.graph.entries().contains(specifier)
  }

  fn has_module(&self, specifier: &ModuleSpecifier) -> bool {
    self.graph.get_module(specifier).is_some()
  }
}

type HyperBody = Full<Bytes>;

pub async fn dev(
  flags: Arc<Flags>,
  dev_flags: DevFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  // Read build configuration from deno.json.
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

  // Resolve and load JS plugins if configured.
  let plugin_driver = if !build_config.plugins.is_empty() {
    let specifiers = plugin_host::resolve_plugin_specifiers(&build_config.plugins)?;
    for spec in &specifiers {
      log::warn!("  {} {}", colors::cyan("plugin"), spec);
    }
    let proxy = plugin_host::create_and_load_plugins(specifiers).await?;
    let proxy = Arc::new(proxy);
    let bridge = plugin_host::JsPluginBridge::new(
      proxy,
      tokio::runtime::Handle::current(),
    );
    create_plugin_driver(vec![Box::new(bridge)])
  } else {
    create_default_plugin_driver()
  };

  // Build module graphs per environment.
  let graph_creator = factory.module_graph_creator().await?.clone();
  let root_url = url_from_directory_path(&root_dir)?;
  let mut environments = HashMap::default();
  let mut env_id_counter = 0u32;

  for (name, env_config) in &build_config.environments {
    let env_id = EnvironmentId::new(env_id_counter);
    env_id_counter += 1;

    let entries: Vec<ModuleSpecifier> = env_config
      .entries
      .iter()
      .map(|entry| {
        root_url.join(entry).unwrap_or_else(|_| {
          ModuleSpecifier::parse(&format!("file:///{}", entry)).unwrap()
        })
      })
      .collect();

    log::warn!(
      "\n  {} {} graph ({} entries)...",
      colors::green("Building"),
      name,
      entries.len(),
    );

    let deno_module_graph = graph_creator
      .create_graph(GraphKind::All, entries.clone(), NpmCachingStrategy::Eager)
      .await?;

    let mut bundler_graph =
      build_bundler_graph(&deno_module_graph, env_id, &entries);

    // Transform (includes TS transpilation via plugin chain),
    // discover non-JS assets, then analyze.
    transform_modules(&mut bundler_graph, &plugin_driver);
    discover_assets(&mut bundler_graph);
    analyze_graph(&mut bundler_graph);

    log::warn!(
      "  {} {} ({} modules)",
      colors::green("Built"),
      name,
      bundler_graph.len(),
    );

    let chunk_graph = build_chunk_graph(&bundler_graph);

    let mut chunk_code = HashMap::default();
    for chunk in chunk_graph.chunks() {
      let output = emit_dev_chunk(
        chunk,
        &bundler_graph,
        &chunk_graph,
        deno_bundler::config::SourceMapMode::Inline,
      );
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

  let state = Arc::new(RwLock::new(DevServerState {
    build_config,
    root_dir: root_dir.clone(),
    environments,
  }));

  // Plugin driver is immutable after creation — shared for file watcher.
  let plugin_driver = Arc::new(plugin_driver);

  // HMR broadcast channel.
  let (hmr_tx, _) = broadcast::channel::<String>(64);

  // Start file watcher.
  let watcher_state = state.clone();
  let watcher_hmr_tx = hmr_tx.clone();
  let watcher_root = root_dir.clone();
  let watcher_plugin_driver = plugin_driver.clone();
  tokio::spawn(async move {
    if let Err(e) =
      run_file_watcher(watcher_root, watcher_state, watcher_hmr_tx, watcher_plugin_driver).await
    {
      log::error!("File watcher error: {}", e);
    }
  });

  // Start HTTP server with hyper.
  let addr = format!("{}:{}", dev_flags.host, dev_flags.port);
  let listener = TcpListener::bind(&addr).await.map_err(|e| {
    deno_core::anyhow::anyhow!("Failed to bind to {}: {}", addr, e)
  })?;

  log::warn!(
    "\n  {} {}",
    colors::green("Local:"),
    colors::cyan(format!("http://{}:{}/", dev_flags.host, dev_flags.port)),
  );

  loop {
    let (stream, _addr) = listener.accept().await?;
    let io = TokioIo::new(stream);
    let state = state.clone();
    let hmr_tx = hmr_tx.clone();

    tokio::spawn(async move {
      let service = hyper::service::service_fn(move |req| {
        let state = state.clone();
        let hmr_tx = hmr_tx.clone();
        async move { handle_request(req, state, hmr_tx).await }
      });

      let server = hyper::server::conn::http1::Builder::new();
      let conn = server.serve_connection(io, service).with_upgrades();
      if let Err(e) = conn.await {
        log::debug!("Connection error: {}", e);
      }
    });
  }
}

async fn handle_request(
  mut req: hyper::Request<Incoming>,
  state: Arc<RwLock<DevServerState>>,
  hmr_tx: broadcast::Sender<String>,
) -> Result<hyper::Response<HyperBody>, hyper::Error> {
  let path = req.uri().path().to_string();

  // WebSocket upgrade for HMR.
  if path == "/__hmr"
    && req
      .headers()
      .get("upgrade")
      .map_or(false, |v| v == "websocket")
  {
    let (resp, upgrade_fut) =
      match fastwebsockets::upgrade::upgrade(&mut req) {
        Ok(r) => r,
        Err(_) => {
          return Ok(
            hyper::Response::builder()
              .status(400)
              .body(Full::new(Bytes::from("Bad WebSocket request")))
              .unwrap(),
          );
        }
      };

    tokio::spawn(async move {
      match upgrade_fut.await {
        Ok(ws) => {
          if let Err(e) = handle_hmr_websocket(ws, hmr_tx).await {
            log::debug!("HMR WebSocket error: {}", e);
          }
        }
        Err(e) => {
          log::debug!("WebSocket upgrade failed: {}", e);
        }
      }
    });

    Ok(resp.map(|_| Full::new(Bytes::new())))
  } else {
    let state = state.read().await;
    Ok(serve_http(&path, &state))
  }
}

async fn handle_hmr_websocket(
  mut ws: WebSocket<TokioIo<hyper::upgrade::Upgraded>>,
  hmr_tx: broadcast::Sender<String>,
) -> Result<(), AnyError> {
  log::debug!("HMR WebSocket client connected");

  let mut rx = hmr_tx.subscribe();

  loop {
    tokio::select! {
      msg = rx.recv() => {
        match msg {
          Ok(text) => {
            let frame = Frame::text(fastwebsockets::Payload::Owned(text.into_bytes()));
            if ws.write_frame(frame).await.is_err() {
              break;
            }
          }
          Err(broadcast::error::RecvError::Lagged(_)) => continue,
          Err(broadcast::error::RecvError::Closed) => break,
        }
      }
      frame = ws.read_frame() => {
        match frame {
          Ok(frame) => {
            match frame.opcode {
              OpCode::Close => break,
              OpCode::Ping => {
                let _ = ws.write_frame(Frame::pong(frame.payload)).await;
              }
              _ => {} // Ignore other client frames.
            }
          }
          Err(_) => break,
        }
      }
    }
  }

  log::debug!("HMR WebSocket client disconnected");
  Ok(())
}

fn serve_http(path: &str, state: &DevServerState) -> hyper::Response<HyperBody> {
  match path {
    "/" => {
      let body = build_index_html(state);
      hyper::Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
    }
    _ => {
      if let Some(code) = serve_chunk(path, state) {
        hyper::Response::builder()
          .header(
            "Content-Type",
            "application/javascript; charset=utf-8",
          )
          .body(Full::new(Bytes::from(code)))
          .unwrap()
      } else {
        hyper::Response::builder()
          .status(404)
          .body(Full::new(Bytes::from(format!("Not found: {}", path))))
          .unwrap()
      }
    }
  }
}

fn build_index_html(state: &DevServerState) -> String {
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

    for chunk_id in env_state.chunk_graph.entry_chunks() {
      envs_html.push_str(&format!(
        " | <a href=\"/{}/{}.js\">entry chunk {}</a>",
        name, chunk_id.0, chunk_id.0
      ));
    }
    envs_html.push_str("</li>");
  }

  format!(
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
      if (msg.type === 'full-reload') {{
        location.reload();
      }}
    }};
    ws.onopen = () => console.log('[HMR] connected');
    ws.onclose = () => console.log('[HMR] disconnected');
  </script>
</body>
</html>"#,
    envs_html,
    state.root_dir.display(),
  )
}

fn serve_chunk(path: &str, state: &DevServerState) -> Option<String> {
  let path = path.strip_prefix('/')?;
  let (env_name, rest) = path.split_once('/')?;
  let chunk_id_str = rest.strip_suffix(".js")?;
  let chunk_id: u32 = chunk_id_str.parse().ok()?;

  let env_state = state.environments.get(env_name)?;
  env_state.chunk_code.get(&chunk_id).cloned()
}

/// Run the file watcher, triggering HMR updates on changes.
async fn run_file_watcher(
  root_dir: PathBuf,
  state: Arc<RwLock<DevServerState>>,
  hmr_tx: broadcast::Sender<String>,
  plugin_driver: Arc<PluginDriver>,
) -> Result<(), AnyError> {
  use notify::RecursiveMode;
  use notify::Watcher;
  use std::time::Duration;

  let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<PathBuf>>(16);

  let mut watcher = notify::recommended_watcher(
    move |event: Result<notify::Event, notify::Error>| {
      if let Ok(event) = event {
        if event.kind.is_modify() || event.kind.is_create() {
          let paths: Vec<PathBuf> = event
            .paths
            .into_iter()
            .filter(|p| {
              matches!(
                p.extension().and_then(|e| e.to_str()),
                Some(
                  "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" | "css"
                    | "html" | "json"
                )
              )
            })
            .collect();
          if !paths.is_empty() {
            let _ = tx.blocking_send(paths);
          }
        }
      }
    },
  )?;

  watcher.watch(&root_dir, RecursiveMode::Recursive)?;
  log::debug!("File watcher started on {}", root_dir.display());

  // Debounce: collect changes over 100ms.
  loop {
    let Some(first_paths) = rx.recv().await else {
      break;
    };

    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut all_paths = first_paths;
    while let Ok(more) = rx.try_recv() {
      all_paths.extend(more);
    }
    all_paths.sort();
    all_paths.dedup();

    log::warn!(
      "\n  {} {} file{} changed",
      colors::green("Detected"),
      all_paths.len(),
      if all_paths.len() == 1 { "" } else { "s" },
    );

    for path in &all_paths {
      if let Some(name) = path.file_name() {
        log::warn!("    {}", name.to_string_lossy());
      }
    }

    // Re-read changed files, re-transpile, compute HMR boundaries.
    let mut state = state.write().await;
    let mut full_reload = false;

    for (_env_name, env_state) in &mut state.environments {
      let changed_specifiers: Vec<ModuleSpecifier> = all_paths
        .iter()
        .filter_map(|p| ModuleSpecifier::from_file_path(p).ok())
        .filter(|s| env_state.bundler_graph.get_module(s).is_some())
        .collect();

      if changed_specifiers.is_empty() {
        continue;
      }

      // Re-read, re-transform, and re-analyze changed modules.
      // Skip modules whose source hasn't actually changed (content hash).
      let mut actually_changed: Vec<deno_ast::ModuleSpecifier> = Vec::new();
      for spec in &changed_specifiers {
        if let Ok(path) = spec.to_file_path() {
          if let Ok(new_source) = std::fs::read_to_string(&path) {
            // Content hash check: skip if file hasn't changed.
            let new_hash = {
              use std::hash::Hash;
              use std::hash::Hasher;
              let mut hasher = rustc_hash::FxHasher::default();
              new_source.hash(&mut hasher);
              hasher.finish()
            };
            if let Some(module) =
              env_state.bundler_graph.get_module(spec)
            {
              if module.source_hash == Some(new_hash) {
                continue; // Source unchanged, skip re-transform.
              }
            }

            if let Some(module) =
              env_state.bundler_graph.get_module_mut(spec)
            {
              // Run through the plugin transform chain (includes transpilation).
              let orig_loader = module.original_loader;
              let output = plugin_driver.transform(
                new_source,
                &path,
                "file",
                orig_loader,
              );
              module.source = output.content;
              module.loader = output.loader;
              module.source_map = output.source_map;
              module.source_hash = Some(new_hash);

              // Use transform output AST if available (avoids re-parsing).
              if let Some(program) = &output.program {
                module.module_info = Some(
                  deno_bundler::js::module_info_swc::extract_module_info(
                    program,
                  ),
                );
                module.hmr_info = Some(
                  deno_bundler::js::hmr_info_swc::extract_hmr_info(program),
                );
                module.is_async = module
                  .module_info
                  .as_ref()
                  .map(|i| i.has_tla)
                  .unwrap_or(false);
                module.transformed_program = Some(program.clone());
                module.parsed = None;
              } else if let Ok(parsed) =
                deno_ast::parse_module(deno_ast::ParseParams {
                  specifier: spec.clone(),
                  text: module.source.clone().into(),
                  media_type: deno_ast::MediaType::JavaScript,
                  capture_tokens: false,
                  scope_analysis: false,
                  maybe_syntax: None,
                })
              {
                let program = parsed.program();
                module.module_info = Some(
                  deno_bundler::js::module_info_swc::extract_module_info(
                    &program,
                  ),
                );
                module.hmr_info = Some(
                  deno_bundler::js::hmr_info_swc::extract_hmr_info(&program),
                );
                module.is_async = module
                  .module_info
                  .as_ref()
                  .map(|i| i.has_tla)
                  .unwrap_or(false);
                module.parsed = Some(parsed);
                module.transformed_program = None;
              }

              actually_changed.push(spec.clone());
            }
          }
        }
      }
      let changed_specifiers = actually_changed;

      // Re-emit affected chunks.
      for chunk in env_state.chunk_graph.chunks() {
        let affected = chunk
          .modules
          .iter()
          .any(|m| changed_specifiers.contains(m));
        if affected {
          let output = emit_dev_chunk(
            chunk,
            &env_state.bundler_graph,
            &env_state.chunk_graph,
            deno_bundler::config::SourceMapMode::Inline,
          );
          env_state.chunk_code.insert(chunk.id.0, output.code);
        }
      }

      // Compute HMR boundaries.
      let hmr_graph = BundlerHmrGraph {
        graph: &env_state.bundler_graph,
      };

      match compute_hmr_boundaries(&hmr_graph, &changed_specifiers) {
        HmrBoundaryResult::Update(update) => {
          let mut modules = HashMap::new();
          for spec in &update.invalidated {
            if let Some((code, _source_map)) = emit_hmr_update(
              spec,
              &env_state.bundler_graph,
              deno_bundler::config::SourceMapMode::Inline,
            ) {
              let mid =
                env_state.bundler_graph.module_index(spec).unwrap_or(0);
              modules.insert(mid, code);
            }
          }

          let boundaries: Vec<u32> = update
            .boundaries
            .iter()
            .filter_map(|s| env_state.bundler_graph.module_index(s))
            .collect();
          let invalidated: Vec<u32> = update
            .invalidated
            .iter()
            .filter_map(|s| env_state.bundler_graph.module_index(s))
            .collect();

          let msg = HmrMessage::Update {
            boundaries,
            invalidated,
            modules,
          };
          let _ = hmr_tx.send(msg.to_json());

          log::warn!("  {} HMR update sent", colors::green("→"));
        }
        HmrBoundaryResult::FullReload => {
          full_reload = true;
        }
      }
    }

    if full_reload {
      let _ = hmr_tx.send(HmrMessage::FullReload.to_json());
      log::warn!(
        "  {} full reload triggered",
        colors::yellow("→"),
      );
    }
  }

  Ok(())
}
