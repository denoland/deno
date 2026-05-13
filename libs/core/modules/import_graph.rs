// Copyright 2018-2026 the Deno authors. MIT license.

//! Build-time instrumentation for the JS module import graph.
//!
//! Enabled by setting `DENO_SNAPSHOT_IMPORT_GRAPH=<path>` in the environment
//! while running the snapshot build (or any `JsRuntime` startup). Each edge
//! is appended as a JSON object on its own line:
//!
//! ```text
//! {"from":"ext:foo/01_a.js","to":"ext:bar/02_b.js","kind":"esm"}
//! ```
//!
//! `kind` is one of:
//! - `"esm"`        — a static `import` discovered while compiling a module
//! - `"lazy_esm"`   — `Deno.core` `createLazyLoader` / `op_lazy_load_esm`
//! - `"lazy_script"` — `Deno.core.loadExtScript(...)`
//!
//! For `lazy_*` edges `from` is the calling script (best-effort via the v8
//! stack trace) and is `"<unknown>"` if no user frame could be identified.

use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::v8;

const ENV_VAR: &str = "DENO_SNAPSHOT_IMPORT_GRAPH";

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

/// `Deno.core` lazy ESM (`createLazyLoader` / `op_lazy_load_esm`).
pub(crate) fn record_lazy_esm(scope: &mut v8::PinScope, to: &str) {
  if !is_enabled() {
    return;
  }
  let from = caller_specifier(scope);
  record(from.as_deref().unwrap_or("<unknown>"), to, "lazy_esm");
}

/// `Deno.core.loadExtScript(...)`.
pub(crate) fn record_lazy_script(scope: &mut v8::PinScope, to: &str) {
  if !is_enabled() {
    return;
  }
  let from = caller_specifier(scope);
  record(from.as_deref().unwrap_or("<unknown>"), to, "lazy_script");
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
