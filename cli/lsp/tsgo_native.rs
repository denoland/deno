// Copyright 2018-2026 the Deno authors. MIT license.

//! Native typescript-go LSP proxy.
//!
//! When `DENO_TSGO_NPM_LSP=1` is set, this module spawns `tsgo --lsp` from the
//! `@typescript/native-preview` npm package (or from PATH) and proxies LSP
//! messages bidirectionally between the editor and tsgo.
//!
//! For Deno type support, it writes the output of `deno types` into
//! `node_modules/@types/deno/index.d.ts` so that tsgo picks up Deno's type
//! definitions automatically via the standard `@types` resolution.

use std::env;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Notify;

use crate::tsc::get_types_declaration_file_text;

/// Pinned version of @typescript/native-preview to auto-download.
const TSGO_NPM_VERSION: &str = "7.0.0-dev.20260331.1";

/// npm registry base URL for the platform-specific packages.
const NPM_REGISTRY: &str = "https://registry.npmjs.org";

/// Get the npm platform identifier for the current OS and architecture.
fn npm_platform() -> Option<(&'static str, &'static str)> {
  match (env::consts::OS, env::consts::ARCH) {
    ("macos", "aarch64") => Some(("darwin", "arm64")),
    ("macos", "x86_64") => Some(("darwin", "x64")),
    ("linux", "x86_64") => Some(("linux", "x64")),
    ("linux", "aarch64") => Some(("linux", "arm64")),
    ("windows", "x86_64") => Some(("win32", "x64")),
    ("windows", "aarch64") => Some(("win32", "arm64")),
    _ => None,
  }
}

/// Get the default Deno cache directory for storing the downloaded tsgo binary.
fn deno_cache_dir() -> Option<PathBuf> {
  // Respect DENO_DIR if set
  if let Ok(dir) = env::var("DENO_DIR") {
    return Some(PathBuf::from(dir));
  }

  // Platform-specific cache directories
  #[cfg(target_os = "macos")]
  {
    env::var("HOME")
      .ok()
      .map(|h| PathBuf::from(h).join("Library/Caches/deno"))
  }
  #[cfg(target_os = "linux")]
  {
    env::var("XDG_CACHE_HOME")
      .ok()
      .map(|d| PathBuf::from(d).join("deno"))
      .or_else(|| {
        env::var("HOME")
          .ok()
          .map(|h| PathBuf::from(h).join(".cache/deno"))
      })
  }
  #[cfg(target_os = "windows")]
  {
    env::var("LOCALAPPDATA")
      .ok()
      .map(|d| PathBuf::from(d).join("deno"))
  }
  #[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
  )))]
  {
    None
  }
}

/// Download the tsgo binary from the npm registry and cache it.
///
/// Downloads `@typescript/native-preview-{os}-{arch}` from npm, extracts
/// the `tsgo` binary, and stores it in `{deno_cache}/dl/tsgo-npm-{version}/`.
fn download_tsgo_from_npm() -> Result<PathBuf, AnyError> {
  let (os, arch) = npm_platform().ok_or_else(|| {
    deno_core::anyhow::anyhow!(
      "unsupported platform for tsgo: {} {}",
      env::consts::OS,
      env::consts::ARCH
    )
  })?;

  let cache_dir = deno_cache_dir().ok_or_else(|| {
    deno_core::anyhow::anyhow!("could not determine Deno cache directory")
  })?;

  let bin_name = if cfg!(windows) { "tsgo.exe" } else { "tsgo" };
  let dl_dir = cache_dir
    .join("dl")
    .join(format!("tsgo-npm-{}", TSGO_NPM_VERSION));
  let bin_path = dl_dir.join(bin_name);

  // Already cached
  if bin_path.exists() {
    log::info!(
      "deno lsp: using cached tsgo {}: {}",
      TSGO_NPM_VERSION,
      bin_path.display()
    );
    return Ok(bin_path);
  }

  let pkg_name = format!("native-preview-{}-{}", os, arch);
  let tarball_url = format!(
    "{0}/@typescript/{1}/-/{1}-{2}.tgz",
    NPM_REGISTRY, pkg_name, TSGO_NPM_VERSION
  );

  log::info!(
    "deno lsp: downloading tsgo {} from npm...",
    TSGO_NPM_VERSION
  );
  log::info!("deno lsp: {}", tarball_url);

  // Create a temp directory for download
  let temp_dir = tempfile::tempdir()?;
  let tgz_path = temp_dir.path().join("tsgo.tgz");

  // Download the tarball using curl (available on all platforms)
  let download_status = std::process::Command::new("curl")
    .args([
      "-fsSL",
      "--retry",
      "3",
      "-o",
      &tgz_path.to_string_lossy(),
      &tarball_url,
    ])
    .status();

  match download_status {
    Ok(status) if status.success() => {}
    Ok(status) => {
      return Err(deno_core::anyhow::anyhow!(
        "curl failed to download tsgo (exit code: {}). URL: {}",
        status,
        tarball_url
      ));
    }
    Err(e) => {
      return Err(deno_core::anyhow::anyhow!(
        "failed to run curl to download tsgo: {}. \
         Ensure curl is installed or install @typescript/native-preview manually.",
        e
      ));
    }
  }

  log::info!("deno lsp: extracting tsgo binary...");

  // Extract just the tsgo binary from the tarball
  // The binary is at package/lib/tsgo inside the tgz
  let extract_dir = temp_dir.path().join("extract");
  std::fs::create_dir_all(&extract_dir)?;

  let extract_status = std::process::Command::new("tar")
    .args([
      "xzf",
      &tgz_path.to_string_lossy(),
      "-C",
      &extract_dir.to_string_lossy(),
      &format!("package/lib/{}", bin_name),
    ])
    .status();

  match extract_status {
    Ok(status) if status.success() => {}
    Ok(status) => {
      return Err(deno_core::anyhow::anyhow!(
        "tar failed to extract tsgo (exit code: {})",
        status
      ));
    }
    Err(e) => {
      return Err(deno_core::anyhow::anyhow!(
        "failed to run tar to extract tsgo: {}",
        e
      ));
    }
  }

  let extracted_bin = extract_dir.join("package/lib").join(bin_name);
  if !extracted_bin.exists() {
    return Err(deno_core::anyhow::anyhow!(
      "tsgo binary not found in npm package at package/lib/{}",
      bin_name
    ));
  }

  // Move to cache directory
  std::fs::create_dir_all(&dl_dir)?;
  std::fs::copy(&extracted_bin, &bin_path)?;

  // Set executable permission on Unix
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      &bin_path,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }

  log::info!(
    "deno lsp: tsgo {} cached at {}",
    TSGO_NPM_VERSION,
    bin_path.display()
  );
  Ok(bin_path)
}

/// Resolve the platform-specific native tsgo binary from the
/// `@typescript/native-preview` npm package.
///
/// The npm package uses optional platform-specific dependencies:
/// `@typescript/native-preview-{os}-{arch}/lib/tsgo`
fn find_native_binary_in_node_modules(node_modules: &Path) -> Option<PathBuf> {
  let (os, arch) = npm_platform()?;
  let bin_name = if cfg!(windows) { "tsgo.exe" } else { "tsgo" };

  let platform_pkg = format!("native-preview-{}-{}", os, arch);
  let candidate = node_modules
    .join("@typescript")
    .join(&platform_pkg)
    .join("lib")
    .join(bin_name);

  candidate.exists().then_some(candidate)
}

/// Find the tsgo binary starting from a given directory. Resolution order:
/// 1. `DENO_TSGO_NPM_PATH` env var (explicit override)
/// 2. Platform-specific native binary in `node_modules/@typescript/native-preview-{os}-{arch}/`
/// 3. Walk up parent directories looking for the same
/// 4. `tsgo` on PATH
/// 5. Auto-download from npm registry to Deno cache directory
fn find_tsgo_binary(start_dir: &Path) -> Result<PathBuf, AnyError> {
  // 1. Explicit path override
  if let Ok(path) = env::var("DENO_TSGO_NPM_PATH") {
    let p = PathBuf::from(&path);
    if p.exists() {
      log::info!(
        "deno lsp: using tsgo from DENO_TSGO_NPM_PATH: {}",
        p.display()
      );
      return Ok(p);
    }
    log::info!(
      "deno lsp: warning: DENO_TSGO_NPM_PATH={} does not exist",
      path
    );
  }

  // 2 & 3. Walk up from start_dir looking for the native binary
  let mut dir = start_dir.to_path_buf();
  loop {
    let node_modules = dir.join("node_modules");
    if node_modules.is_dir()
      && let Some(binary) = find_native_binary_in_node_modules(&node_modules)
    {
      log::info!("deno lsp: using tsgo native binary: {}", binary.display());
      return Ok(binary);
    }
    if !dir.pop() {
      break;
    }
  }

  // 4. PATH lookup
  let which_cmd = if cfg!(windows) { "where" } else { "which" };
  if let Ok(output) = std::process::Command::new(which_cmd).arg("tsgo").output()
    && output.status.success()
  {
    let path_str = String::from_utf8_lossy(&output.stdout)
      .lines()
      .next()
      .unwrap_or("")
      .trim()
      .to_string();
    if !path_str.is_empty() {
      let p = PathBuf::from(&path_str);
      if p.exists() {
        log::info!("deno lsp: using tsgo from PATH: {}", p.display());
        return Ok(p);
      }
    }
  }

  // 5. Auto-download from npm
  download_tsgo_from_npm()
}

/// Write Deno type declarations into `node_modules/@types/deno/` so that tsgo
/// picks them up via standard @types resolution.
///
/// Returns the path to the created types directory for cleanup.
fn install_deno_types(workspace_root: &Path) -> Result<PathBuf, AnyError> {
  let types_dir = workspace_root.join("node_modules/@types/deno");
  std::fs::create_dir_all(&types_dir)?;

  // Write package.json
  let package_json_path = types_dir.join("package.json");
  let package_json = serde_json::json!({
    "name": "@types/deno",
    "version": "0.0.0-generated",
    "description": "Auto-generated Deno type declarations for tsgo native LSP proxy. Safe to delete.",
    "types": "index.d.ts"
  });
  let mut f = std::fs::File::create(&package_json_path)?;
  f.write_all(serde_json::to_string_pretty(&package_json)?.as_bytes())?;

  // Write type declarations, stripping triple-slash reference directives
  // that conflict with @types resolution (e.g., `no-default-lib`)
  let types_text = get_types_declaration_file_text();
  let filtered_types: String = types_text
    .lines()
    .filter(|line| {
      let trimmed = line.trim();
      !(trimmed.starts_with("/// <reference no-default-lib")
        || trimmed.starts_with("/// <reference lib="))
    })
    .collect::<Vec<_>>()
    .join("\n");
  let index_dts_path = types_dir.join("index.d.ts");
  let mut f = std::fs::File::create(&index_dts_path)?;
  f.write_all(
    b"// Auto-generated by deno lsp (tsgo native proxy).\n\
      // Do not edit - this file is regenerated on each LSP startup.\n\
      // Safe to delete; will be recreated when the LSP starts again.\n\n",
  )?;
  f.write_all(filtered_types.as_bytes())?;

  Ok(types_dir)
}

/// Remove the auto-generated @types/deno directory.
fn cleanup_deno_types(types_dir: &Path) {
  if types_dir.exists() {
    if let Err(e) = std::fs::remove_dir_all(types_dir) {
      log::info!(
        "deno lsp: warning: could not clean up {}: {}",
        types_dir.display(),
        e
      );
    } else {
      log::info!("deno lsp: cleaned up {}", types_dir.display());
    }
  }
}

fn spawn_tsgo(tsgo_path: &Path) -> Result<Child, AnyError> {
  let child = Command::new(tsgo_path)
    .arg("--lsp")
    .arg("--stdio")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()?;
  Ok(child)
}

/// Read a single LSP message from an async buffered reader.
/// Returns the raw message bytes (header + body).
async fn read_lsp_message(
  reader: &mut BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<Option<Vec<u8>>, AnyError> {
  let mut content_length: Option<usize> = None;
  let mut header_bytes = Vec::new();

  loop {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
      return Ok(None);
    }
    header_bytes.extend_from_slice(line.as_bytes());

    let trimmed = line.trim();
    if trimmed.is_empty() {
      break;
    }

    if let Some(value) = trimmed
      .strip_prefix("Content-Length:")
      .or_else(|| trimmed.strip_prefix("content-length:"))
    {
      content_length = Some(value.trim().parse()?);
    }
  }

  let content_length = content_length.ok_or_else(|| {
    deno_core::anyhow::anyhow!("LSP message missing Content-Length header")
  })?;

  let mut body = vec![0u8; content_length];
  reader.read_exact(&mut body).await?;

  let mut full_message = header_bytes;
  full_message.extend_from_slice(&body);
  Ok(Some(full_message))
}

/// Read a single LSP message and return the JSON body as bytes separately
/// from the raw framed message.
async fn read_lsp_message_with_body(
  reader: &mut BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<Option<(Vec<u8>, Vec<u8>)>, AnyError> {
  let mut content_length: Option<usize> = None;
  let mut header_bytes = Vec::new();

  loop {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
      return Ok(None);
    }
    header_bytes.extend_from_slice(line.as_bytes());

    let trimmed = line.trim();
    if trimmed.is_empty() {
      break;
    }

    if let Some(value) = trimmed
      .strip_prefix("Content-Length:")
      .or_else(|| trimmed.strip_prefix("content-length:"))
    {
      content_length = Some(value.trim().parse()?);
    }
  }

  let content_length = content_length.ok_or_else(|| {
    deno_core::anyhow::anyhow!("LSP message missing Content-Length header")
  })?;

  let mut body = vec![0u8; content_length];
  reader.read_exact(&mut body).await?;

  let mut raw = header_bytes;
  raw.extend_from_slice(&body);
  Ok(Some((body, raw)))
}

/// Frame a JSON body as an LSP message with Content-Length header.
/// Used when intercepting/modifying LSP messages before forwarding.
#[allow(
  dead_code,
  reason = "useful utility for future LSP message interception"
)]
fn frame_lsp_message(body: &[u8]) -> Vec<u8> {
  let header = format!("Content-Length: {}\r\n\r\n", body.len());
  let mut out = Vec::with_capacity(header.len() + body.len());
  out.extend_from_slice(header.as_bytes());
  out.extend_from_slice(body);
  out
}

/// Extract the workspace root from an LSP `initialize` request's params.
fn extract_workspace_root(json: &serde_json::Value) -> Option<PathBuf> {
  let params = json.get("params")?;

  // Try rootUri first (preferred in LSP 3.x)
  if let Some(root_uri) = params.get("rootUri").and_then(|v| v.as_str())
    && let Ok(url) = Url::parse(root_uri)
    && let Ok(path) = url.to_file_path()
  {
    return Some(path);
  }

  // Fall back to rootPath (deprecated but still sent by some editors)
  if let Some(root_path) = params.get("rootPath").and_then(|v| v.as_str()) {
    return Some(PathBuf::from(root_path));
  }

  // Try workspaceFolders
  if let Some(folders) =
    params.get("workspaceFolders").and_then(|v| v.as_array())
    && let Some(first) = folders.first()
    && let Some(uri) = first.get("uri").and_then(|v| v.as_str())
    && let Ok(url) = Url::parse(uri)
    && let Ok(path) = url.to_file_path()
  {
    return Some(path);
  }

  None
}

/// Main entry point: run the tsgo native LSP proxy.
///
/// This function never returns under normal operation - it runs until the
/// editor disconnects or tsgo exits.
#[allow(
  clippy::disallowed_methods,
  reason = "LSP proxy needs direct stdin/stdout/env access"
)]
pub async fn start_proxy() -> Result<(), AnyError> {
  log::info!("deno lsp: starting tsgo native LSP proxy (prototype)");
  log::info!("deno lsp: powered by @typescript/native-preview (typescript-go)");
  log::info!(
    "deno lsp: note: jsr:, npm:, and http/https specifiers are not supported in this mode"
  );
  log::info!(
    "deno lsp: hint: add '\"types\": [\"deno\"]' to your tsconfig.json compilerOptions for Deno API types"
  );

  // We install Deno types eagerly from cwd. If the initialize request
  // provides a different workspace root, we'll install there too.
  let cwd = env::current_dir()?;
  let mut types_dirs: Vec<PathBuf> = Vec::new();

  match install_deno_types(&cwd) {
    Ok(dir) => {
      log::info!("deno lsp: installed Deno types to {}/", dir.display());
      types_dirs.push(dir);
    }
    Err(e) => {
      log::warn!("deno lsp: warning: could not install Deno types: {}", e);
    }
  }

  let tsgo_path = find_tsgo_binary(&cwd)?;
  let mut child = spawn_tsgo(&tsgo_path)?;
  let child_stdin = child.stdin.take().expect("child stdin");
  let child_stdout = child.stdout.take().expect("child stdout");

  log::info!("deno lsp: tsgo process started (pid: {:?})", child.id());

  let shutdown_notify = Arc::new(Notify::new());

  // Task 1: editor stdin -> tsgo stdin
  let editor_to_tsgo = {
    let mut editor_reader = BufReader::new(tokio::io::stdin());
    let mut tsgo_writer = child_stdin;
    let types_dirs_clone = types_dirs.clone();
    let shutdown = shutdown_notify.clone();

    tokio::spawn(async move {
      let mut initialized = false;
      loop {
        match read_lsp_message_with_body(&mut editor_reader).await {
          Ok(Some((body, raw))) => {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body)
            {
              let method = json
                .get("method")
                .and_then(|m: &serde_json::Value| m.as_str());

              if !initialized && method == Some("initialize") {
                initialized = true;
                log::info!("deno lsp: received initialize request");

                // Install Deno types at workspace root if different from cwd
                if let Some(ws_root) = extract_workspace_root(&json) {
                  let already_installed = types_dirs_clone.iter().any(|d| {
                    d.starts_with(&ws_root)
                      || ws_root.starts_with(
                        d.parent()
                          .and_then(|p| p.parent())
                          .and_then(|p| p.parent())
                          .unwrap_or(Path::new("")),
                      )
                  });
                  if !already_installed {
                    match install_deno_types(&ws_root) {
                      Ok(dir) => {
                        log::info!(
                          "deno lsp: installed Deno types to {}/",
                          dir.display()
                        );
                      }
                      Err(e) => {
                        log::info!(
                          "deno lsp: warning: could not install Deno types at workspace root: {}",
                          e
                        );
                      }
                    }
                  }
                }
              }

              if method == Some("shutdown") {
                log::info!("deno lsp: received shutdown request");
                shutdown.notify_one();
              }
            }

            if let Err(e) = tsgo_writer.write_all(&raw).await {
              log::info!("deno lsp: error writing to tsgo: {}", e);
              break;
            }
            if let Err(e) = tsgo_writer.flush().await {
              log::info!("deno lsp: error flushing to tsgo: {}", e);
              break;
            }
          }
          Ok(None) => {
            log::info!("deno lsp: editor stdin closed");
            break;
          }
          Err(e) => {
            log::info!("deno lsp: error reading from editor: {}", e);
            break;
          }
        }
      }
    })
  };

  // Task 2: tsgo stdout -> editor stdout
  let tsgo_to_editor = {
    let mut tsgo_reader = BufReader::new(child_stdout);
    let mut editor_writer = tokio::io::stdout();

    tokio::spawn(async move {
      loop {
        match read_lsp_message(&mut tsgo_reader).await {
          Ok(Some(raw)) => {
            if let Err(e) = editor_writer.write_all(&raw).await {
              log::info!("deno lsp: error writing to editor: {}", e);
              break;
            }
            if let Err(e) = editor_writer.flush().await {
              log::info!("deno lsp: error flushing to editor: {}", e);
              break;
            }
          }
          Ok(None) => {
            log::info!("deno lsp: tsgo stdout closed");
            break;
          }
          Err(e) => {
            log::info!("deno lsp: error reading from tsgo: {}", e);
            break;
          }
        }
      }
    })
  };

  // Wait for either direction to finish
  tokio::select! {
    r = editor_to_tsgo => {
      if let Err(e) = r {
        log::info!("deno lsp: editor->tsgo task failed: {}", e);
      }
    }
    r = tsgo_to_editor => {
      if let Err(e) = r {
        log::info!("deno lsp: tsgo->editor task failed: {}", e);
      }
    }
  }

  // Clean up
  log::info!("deno lsp: shutting down tsgo process");
  let _ = child.kill().await;

  for dir in &types_dirs {
    cleanup_deno_types(dir);
  }

  Ok(())
}
