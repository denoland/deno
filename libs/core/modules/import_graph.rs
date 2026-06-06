// Copyright 2018-2026 the Deno authors. MIT license.

//! Instrumentation for the JS module import graph.
//!
//! Two independent knobs:
//!
//! - `DENO_SNAPSHOT_IMPORT_GRAPH=<path>` — append a JSONL graph entry per
//!   import/lazy-load to the given file. Designed for snapshot-build
//!   analysis but works at runtime too. Each line is `{"from","to","kind"}`
//!   where `kind` is `"esm"` | `"lazy_esm"` | `"lazy_script"`.
//!
//! - `DENO_LOG_LAZY_LOAD=1` — print a short line to stderr each time a
//!   `lazy_loaded_esm` module is loaded via `op_lazy_load_esm` or a
//!   `lazy_loaded_js` script via `op_load_ext_script`. Use to see what
//!   `deno run hello.js` actually evaluates at startup (i.e. what *fell
//!   out* of the snapshot and is now paying parse/compile cost lazily).
//!
//! For `lazy_*` edges `from` is the calling script (best-effort via the v8
//! stack trace) and is `"<unknown>"` if no user frame could be identified.

// This module is a developer-facing instrumentation knob gated entirely on
// env vars set by humans running snapshot builds or debugging startup time
// (`DENO_SNAPSHOT_IMPORT_GRAPH`, `DENO_LOG_LAZY_LOAD`). Using `sys_traits`
// here would force every embedder to plumb a Sys through, for zero benefit
// since this code never runs except when those env vars are set. Same for
// stderr: this is a debug print to a human, not user-facing CLI output.
#![allow(
  clippy::disallowed_methods,
  clippy::print_stderr,
  reason = "developer-facing snapshot/startup instrumentation gated on env vars"
)]

use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::v8;

const ENV_VAR: &str = "DENO_SNAPSHOT_IMPORT_GRAPH";
const STDERR_ENV_VAR: &str = "DENO_LOG_LAZY_LOAD";

struct Writer {
  inner: Mutex<BufWriter<File>>,
}

fn writer() -> Option<&'static Writer> {
  static WRITER: OnceLock<Option<Writer>> = OnceLock::new();
  WRITER
    .get_or_init(|| {
      let path = std::env::var_os(ENV_VAR)?;
      let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| {
          panic!(
            "{ENV_VAR}: failed to open {} for append: {e}",
            std::path::Path::new(&path).display()
          )
        });
      Some(Writer {
        inner: Mutex::new(BufWriter::new(file)),
      })
    })
    .as_ref()
}

pub(crate) fn is_enabled() -> bool {
  writer().is_some()
}

fn stderr_log_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| {
    matches!(std::env::var_os(STDERR_ENV_VAR).as_deref(), Some(v) if v != "0" && !v.is_empty())
  })
}

fn log_to_stderr(kind: &str, to: &str, from: Option<&str>) {
  // Single-line, parse-friendly. Caller is best-effort, may be missing.
  match from {
    Some(f) => eprintln!("[lazy] {kind:<11} {to:<48} <- {f}"),
    None => eprintln!("[lazy] {kind:<11} {to}"),
  }
}

fn record(from: &str, to: &str, kind: &str) {
  let Some(w) = writer() else { return };
  let mut guard = w.inner.lock().unwrap();
  // Hand-rolled JSON keeps this self-contained (no serde dep churn) and
  // guarantees the line is appended atomically up to the trailing newline.
  let _ = writeln!(
    guard,
    r#"{{"from":{},"to":{},"kind":"{}"}}"#,
    json_string(from),
    json_string(to),
    kind,
  );
  // Flush eagerly so a crashed snapshot build still leaves a partial graph.
  let _ = guard.flush();
}

/// A static ESM `import` edge discovered while compiling `from`.
pub(crate) fn record_esm_import(from: &str, to: &str) {
  record(from, to, "esm");
}

/// `Deno.core` lazy ESM: a cache miss, i.e. the module was actually parsed,
/// instantiated, and evaluated.
pub(crate) fn record_lazy_esm(scope: &mut v8::PinScope, to: &str) {
  let stderr = stderr_log_enabled();
  if !is_enabled() && !stderr {
    return;
  }
  let from = caller_specifier(scope);
  if is_enabled() {
    record(from.as_deref().unwrap_or("<unknown>"), to, "lazy_esm");
  }
  if stderr {
    log_to_stderr("lazy_esm", to, from.as_deref());
  }
}

/// Like [`record_lazy_esm`] but for the cache-hit path. Recorded into the
/// graph file (so analysis sees the edge) but suppressed from stderr —
/// nothing was actually parsed.
pub(crate) fn record_lazy_esm_cached(scope: &mut v8::PinScope, to: &str) {
  if !is_enabled() {
    return;
  }
  let from = caller_specifier(scope);
  record(
    from.as_deref().unwrap_or("<unknown>"),
    to,
    "lazy_esm_cached",
  );
}

/// `Deno.core.loadExtScript(...)`. Called via the op only on cache miss
/// (the JS-side `loadedScripts` map short-circuits cache hits before the
/// op fires), so this always corresponds to a real load.
pub(crate) fn record_lazy_script(scope: &mut v8::PinScope, to: &str) {
  let stderr = stderr_log_enabled();
  if !is_enabled() && !stderr {
    return;
  }
  let from = caller_specifier(scope);
  if is_enabled() {
    record(from.as_deref().unwrap_or("<unknown>"), to, "lazy_script");
  }
  if stderr {
    log_to_stderr("lazy_script", to, from.as_deref());
  }
}

/// Walk the current v8 stack and return the script name of the first user
/// frame outside `ext:core/01_core.js` (which only contains the wrapper
/// helpers `loadExtScript`/`createLazyLoader` and would otherwise mask every
/// real caller). Returns `None` if no such frame is found.
fn caller_specifier(scope: &mut v8::PinScope) -> Option<String> {
  let stack = v8::StackTrace::current_stack_trace(scope, 32)?;
  let count = stack.get_frame_count();
  for i in 0..count {
    let frame = stack.get_frame(scope, i)?;
    if !frame.is_user_javascript() {
      continue;
    }
    let Some(name) = frame.get_script_name(scope) else {
      continue;
    };
    let name = name.to_rust_string_lossy(scope);
    if name == "ext:core/01_core.js" {
      continue;
    }
    return Some(name);
  }
  None
}

fn json_string(s: &str) -> String {
  let mut out = String::with_capacity(s.len() + 2);
  out.push('"');
  for c in s.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      c if (c as u32) < 0x20 => {
        out.push_str(&format!("\\u{:04x}", c as u32));
      }
      c => out.push(c),
    }
  }
  out.push('"');
  out
}
