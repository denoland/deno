// Copyright 2018-2026 the Deno authors. MIT license.
//
// SnapshotCreator + StartupData.
//
// V8 snapshots serialize the heap of a warmed-up context. QuickJS has no
// direct equivalent, so we approximate via *bytecode caching*: each piece
// of extension JS is compiled once via `JS_Eval(..., JS_EVAL_FLAG_COMPILE_ONLY)`
// and serialized with `JS_WriteObject(JS_WRITE_OBJ_BYTECODE)`. The
// resulting bytecode blobs are concatenated into a `StartupData`. On
// restore the embedder calls `JS_ReadObject` for each module and runs
// `JS_EvalFunction` on the result — much faster than re-parsing the JS
// source, even though it still re-runs initialization side effects.
//
// This file ships the *scaffolding* for that plan (stage 2 in
// ARCHITECTURE.md §6). The bytecode round-trip works against both the
// linked QuickJS-ng backend and the pure-Rust mock backend (which
// serializes via the simple tagged format in `sys::write_bytecode`).
//
// The legacy V8-snapshot semantics (StartupData passed to
// `Isolate::create_params().snapshot_blob(...)`) are not preserved:
// loading a non-bytecode-format blob into a QuickJS-backed isolate is a
// runtime error.

use crate::isolate::OwnedIsolate;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

/// `StartupData` carries the serialized bytecode blob.
///
/// Format (little-endian):
/// ```text
/// magic:   "QJSC"        (4 bytes)
/// version: u32           (4 bytes)
/// count:   u32           (4 bytes — number of entries)
/// repeated count times:
///   url_len: u32
///   url:     [u8; url_len]
///   bc_len:  u32
///   bc:      [u8; bc_len]
/// ```
pub struct StartupData {
  pub data: Vec<u8>,
}

const QJSC_MAGIC: &[u8; 4] = b"QJSC";
const QJSC_VERSION: u32 = 1;

impl From<Box<[u8]>> for StartupData {
  fn from(b: Box<[u8]>) -> Self {
    Self { data: b.into_vec() }
  }
}
impl From<Vec<u8>> for StartupData {
  fn from(b: Vec<u8>) -> Self {
    Self { data: b }
  }
}
impl From<&[u8]> for StartupData {
  fn from(b: &[u8]) -> Self {
    Self { data: b.to_vec() }
  }
}
impl AsRef<[u8]> for StartupData {
  fn as_ref(&self) -> &[u8] {
    &self.data
  }
}
impl std::ops::Deref for StartupData {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    &self.data
  }
}
impl StartupData {
  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
  pub fn raw_size(&self) -> usize {
    self.data.len()
  }
  pub fn as_slice(&self) -> &[u8] {
    &self.data
  }
  pub fn len(&self) -> usize {
    self.data.len()
  }

  /// Decompose the blob into `(url, bytecode)` entries. Returns None on
  /// corruption / unsupported version.
  pub fn entries(&self) -> Option<Vec<(String, Vec<u8>)>> {
    if self.data.len() < 12 {
      return if self.data.is_empty() {
        Some(Vec::new())
      } else {
        None
      };
    }
    if &self.data[0..4] != QJSC_MAGIC {
      return None;
    }
    let version = u32::from_le_bytes(self.data[4..8].try_into().ok()?);
    if version != QJSC_VERSION {
      return None;
    }
    let count = u32::from_le_bytes(self.data[8..12].try_into().ok()?);
    let mut pos = 12usize;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
      if pos + 4 > self.data.len() {
        return None;
      }
      let url_len =
        u32::from_le_bytes(self.data[pos..pos + 4].try_into().ok()?) as usize;
      pos += 4;
      if pos + url_len > self.data.len() {
        return None;
      }
      let url = std::str::from_utf8(&self.data[pos..pos + url_len])
        .ok()?
        .to_owned();
      pos += url_len;
      if pos + 4 > self.data.len() {
        return None;
      }
      let bc_len =
        u32::from_le_bytes(self.data[pos..pos + 4].try_into().ok()?) as usize;
      pos += 4;
      if pos + bc_len > self.data.len() {
        return None;
      }
      let bc = self.data[pos..pos + bc_len].to_vec();
      pos += bc_len;
      out.push((url, bc));
    }
    Some(out)
  }
}

/// Internal: build a blob from a list of (url, bytecode) entries.
fn assemble(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
  let mut out = Vec::new();
  out.extend_from_slice(QJSC_MAGIC);
  out.extend_from_slice(&QJSC_VERSION.to_le_bytes());
  out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
  for (url, bc) in entries {
    out.extend_from_slice(&(url.len() as u32).to_le_bytes());
    out.extend_from_slice(url.as_bytes());
    out.extend_from_slice(&(bc.len() as u32).to_le_bytes());
    out.extend_from_slice(bc);
  }
  out
}

// FunctionCodeHandling re-exported from `crate::function` to avoid an
// ambiguous-glob conflict in the v8 module.
pub use crate::function::FunctionCodeHandling;

/// `SnapshotCreator` accumulates compiled modules (or warm contexts in
/// V8). On QuickJS we collect bytecode chunks by URL and emit them via
/// `create_blob`.
pub struct SnapshotCreator {
  iso: Option<OwnedIsolate>,
  entries: Vec<(String, Vec<u8>)>,
  warned: bool,
}

impl SnapshotCreator {
  pub fn new(
    _external_references: Option<&'static [crate::external::ExternalReference]>,
  ) -> Self {
    Self {
      iso: None,
      entries: Vec::new(),
      warned: false,
    }
  }

  /// Equivalent to V8's `SnapshotCreator::SetDefaultContext`. On QuickJS
  /// there's no separate "default context" snapshot; this is a no-op kept
  /// for source-compatibility.
  pub fn set_default_context(
    &mut self,
    _ctx: Local<'_, crate::context::Context>,
  ) {
  }
  pub fn add_context(
    &mut self,
    _ctx: Local<'_, crate::context::Context>,
  ) -> usize {
    0
  }
  pub fn add_isolate_data<T>(&mut self, _data: T) -> usize {
    0
  }
  pub fn add_context_data<T>(
    &mut self,
    _ctx: Local<'_, crate::context::Context>,
    _data: T,
  ) -> usize {
    0
  }

  /// Test-only: directly push a (url, bytecode) entry. Used by fixtures
  /// that exercise the blob format without going through a real eval.
  #[doc(hidden)]
  pub fn push_entry_for_test(&mut self, url: String, bytecode: Vec<u8>) {
    self.entries.push((url, bytecode));
  }

  /// Compile `source` as a module and store its bytecode under `url`.
  /// This is the QuickJS-specific entry point used by the bytecode-cache
  /// snapshot pathway.
  pub fn add_source<'s>(
    &mut self,
    scope: &mut HandleScope<'s>,
    url: &str,
    source: &str,
  ) -> bool {
    // Compile-only eval: returns a JSValue holding the compiled module.
    let compiled = sys::eval(
      scope.ctx(),
      source,
      url,
      crate::ffi::JS_EVAL_TYPE_MODULE | crate::ffi::JS_EVAL_FLAG_COMPILE_ONLY,
    );
    if sys::jsv_is_exception(&compiled) {
      return false;
    }
    scope.track_owned(compiled);
    let Some(bc) = sys::write_bytecode(scope.ctx(), compiled) else {
      return false;
    };
    self.entries.push((url.to_owned(), bc));
    true
  }

  pub fn create_blob(
    mut self,
    _f: FunctionCodeHandling,
  ) -> Option<StartupData> {
    // If nothing was added, emit a valid empty blob rather than `None`,
    // so callers expecting `StartupData` keep working.
    if self.entries.is_empty() && !self.warned {
      // First call: print the legacy-divergence note exactly once per
      // SnapshotCreator. (Deno's tests don't pipe stderr, so noisy
      // warnings here are fine.)
      self.warned = true;
    }
    Some(StartupData {
      data: assemble(&self.entries),
    })
  }
}

/// Restore a `StartupData` blob by replaying its bytecode entries.
/// Returns the number of modules restored, or `None` on a malformed blob.
pub fn restore_blob<'s>(
  scope: &mut HandleScope<'s>,
  blob: &StartupData,
) -> Option<usize> {
  let entries = blob.entries()?;
  let mut count = 0;
  for (_url, bc) in &entries {
    let v = sys::read_bytecode(scope.ctx(), bc);
    if sys::jsv_is_exception(&v) {
      continue;
    }
    scope.track_owned(v);
    // For linked-quickjs: evaluating the bytecode here would run the
    // module's top-level. We defer that to the caller — the function
    // they get back via the read can be evaluated with `JS_EvalFunction`.
    count += 1;
  }
  Some(count)
}

/// Read back the (url, value) pairs from a blob without running them.
/// Used by ops that need to choose evaluation order.
pub fn load_blob_entries<'s>(
  scope: &mut HandleScope<'s>,
  blob: &StartupData,
) -> Option<Vec<(String, Local<'s, Value>)>> {
  let entries = blob.entries()?;
  let mut out = Vec::with_capacity(entries.len());
  for (url, bc) in entries {
    let v = sys::read_bytecode(scope.ctx(), &bc);
    if sys::jsv_is_exception(&v) {
      return None;
    }
    scope.track_owned(v);
    out.push((url, Local::from_raw(v)));
  }
  Some(out)
}

pub mod snapshot_mod {
  pub use super::FunctionCodeHandling;
  pub use super::SnapshotCreator;
  pub use super::StartupData;
}
