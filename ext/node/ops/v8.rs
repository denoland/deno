// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Write as _;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::FastString;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

static EXPOSE_GC_FROM_SET_FLAGS: AtomicBool = AtomicBool::new(false);

#[op2(fast)]
pub fn op_v8_cached_data_version_tag() -> u32 {
  v8::script_compiler::cached_data_version_tag()
}

#[op2(fast)]
pub fn op_v8_set_flags_from_string(#[string] flags: &str) {
  for flag in flags.split_ascii_whitespace() {
    match flag {
      "--expose_gc" | "--expose-gc" => {
        EXPOSE_GC_FROM_SET_FLAGS.store(true, Ordering::SeqCst);
      }
      "--noexpose_gc" | "--no-expose-gc" => {
        EXPOSE_GC_FROM_SET_FLAGS.store(false, Ordering::SeqCst);
      }
      _ => {}
    }
  }
}

fn gc_callback(
  scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  scope.low_memory_notification();
}

pub fn install_gc_if_exposed<'s, 'i, T>(
  scope: &mut v8::PinScope<'s, 'i, T>,
  context: v8::Local<'s, v8::Context>,
) {
  if !EXPOSE_GC_FROM_SET_FLAGS.load(Ordering::SeqCst) {
    return;
  }

  let scope = &mut v8::ContextScope::new(scope, context);
  let global = context.global(scope);
  let key = v8::String::new_external_onebyte_static(scope, b"gc").unwrap();
  let template = v8::FunctionTemplate::new(scope, gc_callback);
  let function = template.get_function(scope).unwrap();
  function.set_name(key);
  global.set(scope, key.into(), function.into());
}

#[op2(fast)]
pub fn op_v8_get_heap_statistics(
  scope: &mut v8::PinScope<'_, '_>,
  #[buffer] buffer: &mut [f64],
) {
  let stats = scope.get_heap_statistics();

  buffer[0] = stats.total_heap_size() as f64;
  buffer[1] = stats.total_heap_size_executable() as f64;
  buffer[2] = stats.total_physical_size() as f64;
  buffer[3] = stats.total_available_size() as f64;
  buffer[4] = stats.used_heap_size() as f64;
  buffer[5] = stats.heap_size_limit() as f64;
  buffer[6] = stats.malloced_memory() as f64;
  buffer[7] = stats.peak_malloced_memory() as f64;
  buffer[8] = if stats.does_zap_garbage() { 1.0 } else { 0.0 };
  buffer[9] = stats.number_of_native_contexts() as f64;
  buffer[10] = stats.number_of_detached_contexts() as f64;
  buffer[11] = stats.total_global_handles_size() as f64;
  buffer[12] = stats.used_global_handles_size() as f64;
  buffer[13] = stats.external_memory() as f64;
  buffer[14] = stats.total_allocated_bytes() as f64;
}

#[op2(fast)]
#[smi]
pub fn op_v8_number_of_heap_spaces(scope: &mut v8::PinScope<'_, '_>) -> u32 {
  scope.number_of_heap_spaces() as u32
}

#[op2]
#[string]
pub fn op_v8_update_heap_space_statistics(
  scope: &mut v8::PinScope<'_, '_>,
  #[buffer] buffer: &mut [f64],
  #[smi] space_index: u32,
) -> Option<String> {
  let stats = scope.get_heap_space_statistics(space_index as usize)?;
  buffer[0] = stats.space_size() as f64;
  buffer[1] = stats.space_used_size() as f64;
  buffer[2] = stats.space_available_size() as f64;
  buffer[3] = stats.physical_space_size() as f64;
  Some(stats.space_name().to_string_lossy().into_owned())
}

#[op2]
#[buffer]
pub fn op_v8_take_heap_snapshot(scope: &mut v8::PinScope<'_, '_>) -> Vec<u8> {
  let mut buf = Vec::new();
  scope.take_heap_snapshot(|chunk| {
    buf.extend_from_slice(chunk);
    true
  });
  buf
}

// --- setHeapSnapshotNearHeapLimit -----------------------------------------
//
// Implements `v8.setHeapSnapshotNearHeapLimit(limit)`. Installs a V8
// near-heap-limit callback that streams a `.heapsnapshot` file to disk (up to
// `limit` times) right before the process would OOM, mirroring Node's
// `Environment::NearHeapLimitCallback` (src/heap_utils.cc).

// State for the near-heap-limit snapshot callback. Leaked (never freed) so it
// outlives the isolate, which keeps the callback installed for the
// isolate/process lifetime.
struct HeapSnapshotNearHeapLimitState {
  // Raw isolate pointer captured at op-call time, used to take the snapshot
  // from within the extern "C" callback (which is not handed an isolate).
  isolate: v8::UnsafeRawIsolatePtr,
  limit: u32,
  taken: u32,
  // Reentrancy guard: taking a snapshot can trigger GC and re-enter the
  // callback; we must not recurse into another snapshot.
  processing: bool,
  dir: PathBuf,
  pid: u32,
  seq: u32,
}

// Howard Hinnant's civil-from-days algorithm: converts a count of days since
// the Unix epoch into a (year, month, day) tuple (UTC). Avoids pulling in a
// date/time dependency just to format the snapshot filename.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
  let z = z + 719468;
  let era = if z >= 0 { z } else { z - 146096 } / 146097;
  let doe = (z - era * 146097) as u64; // [0, 146096]
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
  let mp = (5 * doy + 2) / 153; // [0, 11]
  let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
  let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
  let y = if m <= 2 { y + 1 } else { y };
  (y, m, d)
}

// Builds a filename matching Deno/Node's `writeHeapSnapshot` naming scheme:
// `Heap.<YYYYMMDD>.<HHMMSS>.<pid>.<threadId>.<seq(3 digits)>.heapsnapshot`.
//
// The thread id is hardcoded to 0, matching `writeHeapSnapshot` in v8.ts. In
// Node this slot is the worker's `threadId`, which isn't readily available to
// this native callback. As a result, two worker threads that OOM within the
// same second (each with its own `seq` starting at 0) can produce identical
// filenames and overwrite each other's snapshot. Plumbing the real `threadId`
// through would disambiguate them.
fn heap_snapshot_filename(pid: u32, seq: u32) -> String {
  let secs = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or_default();
  let days = (secs / 86400) as i64;
  let secs_of_day = secs % 86400;
  let (year, month, day) = civil_from_days(days);
  let hours = secs_of_day / 3600;
  let minutes = (secs_of_day % 3600) / 60;
  let seconds = secs_of_day % 60;
  format!(
    "Heap.{year:04}{month:02}{day:02}.{hours:02}{minutes:02}{seconds:02}.{pid}.0.{seq:03}.heapsnapshot"
  )
}

#[allow(
  clippy::print_stderr,
  reason = "Node prints the snapshot path to stderr unconditionally; \
   mirror that so the OOM diagnostic is always visible."
)]
extern "C" fn near_heap_limit_snapshot_callback(
  data: *mut c_void,
  current_heap_limit: usize,
  _initial_heap_limit: usize,
) -> usize {
  // SAFETY: `data` is the leaked `HeapSnapshotNearHeapLimitState` pointer
  // installed by `op_v8_set_heap_snapshot_near_heap_limit`. It outlives the
  // isolate, so this mutable reference is valid for the callback's duration
  // (V8 invokes near-heap-limit callbacks synchronously and non-concurrently).
  let state = unsafe { &mut *(data as *mut HeapSnapshotNearHeapLimitState) };

  // SAFETY: `state.isolate` was captured from the live isolate scope in the op
  // and the isolate is valid while this near-heap-limit callback runs. The
  // reconstructed `Isolate` is a non-owning handle (no Drop), used only for the
  // duration of this call.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr_unchecked(state.isolate) };

  // Give V8 headroom so it doesn't immediately OOM while we write the snapshot.
  // Mirrors Node returning `current_heap_limit + max_young_gen_size`: sum the
  // used size of the young-generation spaces.
  let mut young_headroom = 0usize;
  let nspaces = isolate.number_of_heap_spaces();
  for i in 0..nspaces {
    if let Some(s) = isolate.get_heap_space_statistics(i) {
      let name = s.space_name().to_string_lossy();
      if matches!(name.as_ref(), "new_space" | "new_large_object_space") {
        young_headroom = young_headroom.saturating_add(s.space_used_size());
      }
    }
  }
  // Guarantee a sane minimum floor so V8 has room to finish writing.
  const MIN_HEADROOM: usize = 4 * 1024 * 1024;
  let new_limit =
    current_heap_limit.saturating_add(young_headroom.max(MIN_HEADROOM));

  // Nested/reentrant call while a snapshot is being generated: give transient
  // room so V8 can finish writing without OOMing mid-snapshot.
  if state.processing {
    return new_limit;
  }
  // No more snapshots allowed: remove the callback and return the *unchanged*
  // heap limit so V8 proceeds to OOM normally. Mirrors Node, which removes its
  // callback and returns `current_heap_limit` once the budget is exhausted;
  // returning a raised limit here would let the heap grow unbounded and never
  // OOM.
  if state.taken >= state.limit {
    // SAFETY: removing the near-heap-limit callback from within the callback
    // is supported by V8 (Node does the same). Passing 0 leaves V8 to restore
    // the minimal viable limit for the current heap size.
    isolate
      .remove_near_heap_limit_callback(near_heap_limit_snapshot_callback, 0);
    return current_heap_limit;
  }

  state.processing = true;
  state.taken += 1;
  state.seq += 1;

  let filename = heap_snapshot_filename(state.pid, state.seq);
  let path = state.dir.join(&filename);

  match std::fs::File::create(&path) {
    Ok(file) => {
      log::info!("Writing heap snapshot to {}", path.display());
      let mut writer = std::io::BufWriter::new(file);
      let mut write_ok = true;
      // Stream chunks straight to disk instead of buffering the whole snapshot
      // in memory (which would OOM the very process we're trying to snapshot).
      isolate.take_heap_snapshot(|chunk| {
        if writer.write_all(chunk).is_err() {
          write_ok = false;
          return false;
        }
        true
      });
      if !write_ok || writer.flush().is_err() {
        log::error!("Failed to write heap snapshot to {}", path.display());
      }
    }
    Err(e) => {
      log::error!(
        "Failed to create heap snapshot file {}: {e}",
        path.display()
      );
    }
  }

  state.processing = false;
  new_limit
}

#[op2(nofast)]
pub fn op_v8_set_heap_snapshot_near_heap_limit(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
  #[smi] limit: u32,
) -> Result<(), deno_permissions::PermissionCheckError> {
  let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let dir = state
    .borrow_mut::<PermissionsContainer>()
    .check_write(Cow::Owned(dir), "v8.setHeapSnapshotNearHeapLimit")?
    .into_owned_path();
  // SAFETY: capture the raw pointer of the current isolate so the extern "C"
  // callback (which is not passed an isolate) can take a snapshot. The isolate
  // outlives the callback, so the pointer stays valid whenever it runs.
  let isolate = unsafe { scope.as_raw_isolate_ptr() };
  let state = Box::new(HeapSnapshotNearHeapLimitState {
    isolate,
    limit,
    taken: 0,
    processing: false,
    dir,
    pid: std::process::id(),
    seq: 0,
  });
  // Leak the state so it outlives the isolate (see the struct doc comment).
  let data = Box::into_raw(state) as *mut c_void;
  scope.add_near_heap_limit_callback(near_heap_limit_snapshot_callback, data);
  Ok(())
}

// Walks the V8 heap snapshot and counts nodes that look like instances of a
// class whose constructor name matches `ctor_name`. Used by `util.queryObjects`
// / `v8.queryObjects` to implement `{ format: 'count' }` without exposing
// `HeapProfiler::QueryObjects` (which the rusty_v8 crate does not bind).
//
// Limitation: matches by the immediate constructor name only, so instances of
// subclasses of `ctor` won't be counted. This is sufficient for Node's leak
// tests (which check direct instances of `Channel`, `SourceTextModule`, ...).
#[op2(nofast)]
#[smi]
pub fn op_v8_query_objects_count(
  scope: &mut v8::PinScope<'_, '_>,
  #[string] ctor_name: &str,
) -> u32 {
  use deno_core::serde_json;
  use deno_core::serde_json::Value;

  let mut buf = Vec::new();
  scope.take_heap_snapshot(|chunk| {
    buf.extend_from_slice(chunk);
    true
  });
  if buf.is_empty() {
    return 0;
  }

  let snapshot: Value = match serde_json::from_slice(&buf) {
    Ok(v) => v,
    Err(_) => return 0,
  };

  let meta = match snapshot.get("snapshot").and_then(|s| s.get("meta")) {
    Some(m) => m,
    None => return 0,
  };
  let node_fields = match meta.get("node_fields").and_then(|f| f.as_array()) {
    Some(a) => a,
    None => return 0,
  };
  let node_field_count = node_fields.len();
  if node_field_count == 0 {
    return 0;
  }
  let type_field_index = node_fields.iter().position(|f| f == "type");
  let name_field_index = node_fields.iter().position(|f| f == "name");
  let (Some(type_field_index), Some(name_field_index)) =
    (type_field_index, name_field_index)
  else {
    return 0;
  };

  // `node_types` is an array where the entry at `type_field_index` is the
  // list of named type variants (the rest are scalars like "string"/"number").
  let object_type_index = match meta
    .get("node_types")
    .and_then(|t| t.as_array())
    .and_then(|t| t.get(type_field_index))
    .and_then(|t| t.as_array())
  {
    Some(types) => match types.iter().position(|t| t == "object") {
      Some(i) => i as u64,
      None => return 0,
    },
    None => return 0,
  };

  let nodes = match snapshot.get("nodes").and_then(|n| n.as_array()) {
    Some(a) => a,
    None => return 0,
  };
  let strings = match snapshot.get("strings").and_then(|s| s.as_array()) {
    Some(a) => a,
    None => return 0,
  };

  let mut count: u32 = 0;
  for chunk in nodes.chunks_exact(node_field_count) {
    let Some(ty) = chunk[type_field_index].as_u64() else {
      continue;
    };
    if ty != object_type_index {
      continue;
    }
    let Some(name_idx) = chunk[name_field_index].as_u64() else {
      continue;
    };
    let Some(name) = strings.get(name_idx as usize).and_then(|s| s.as_str())
    else {
      continue;
    };
    if name == ctor_name {
      count = count.saturating_add(1);
    }
  }
  count
}

#[op2(fast)]
pub fn op_v8_get_heap_code_statistics(
  scope: &mut v8::PinScope<'_, '_>,
  #[buffer] buffer: &mut [f64],
) {
  if let Some(stats) = scope.get_heap_code_and_metadata_statistics() {
    buffer[0] = stats.code_and_metadata_size() as f64;
    buffer[1] = stats.bytecode_and_metadata_size() as f64;
    buffer[2] = stats.external_script_source_size() as f64;
    buffer[3] = stats.cpu_profiler_metadata_size() as f64;
  }
}

pub struct Serializer<'a> {
  delegate_state: Rc<SerializerDelegateState>,
  inner: v8::ValueSerializer<'a>,
}

struct SerializerDelegateState {
  obj: v8::TracedReference<v8::Object>,
}

pub struct SerializerDelegate {
  state: Rc<SerializerDelegateState>,
}

// SAFETY: we're sure this can be GCed
unsafe impl v8::cppgc::GarbageCollected for Serializer<'_> {
  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    visitor.trace(&self.delegate_state.obj);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Serializer"
  }
}

impl SerializerDelegate {
  fn obj<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Object> {
    self.state.obj.get(scope).unwrap()
  }
}

impl v8::ValueSerializerImpl for SerializerDelegate {
  fn get_shared_array_buffer_id<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
  ) -> Option<u32> {
    let obj = self.obj(scope);
    let key = FastString::from_static("_getSharedArrayBufferId")
      .v8_string(scope)
      .unwrap()
      .into();
    if let Some(v) = obj.get(scope, key)
      && let Ok(fun) = v.try_cast::<v8::Function>()
    {
      return fun
        .call(scope, obj.into(), &[shared_array_buffer.into()])
        .and_then(|ret| ret.uint32_value(scope));
    }
    None
  }
  fn has_custom_host_object(&self, _isolate: &v8::Isolate) -> bool {
    false
  }
  fn throw_data_clone_error<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    message: v8::Local<'s, v8::String>,
  ) {
    let obj = self.obj(scope);
    let key = FastString::from_static("_getDataCloneError")
      .v8_string(scope)
      .unwrap()
      .into();
    if let Some(v) = obj.get(scope, key) {
      let fun = v
        .try_cast::<v8::Function>()
        .expect("_getDataCloneError should be a function");
      if let Some(error) = fun.call(scope, obj.into(), &[message.into()]) {
        scope.throw_exception(error);
        return;
      }
    }
    let error = v8::Exception::type_error(scope, message);
    scope.throw_exception(error);
  }

  fn write_host_object<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<'s, v8::Object>,
    _value_serializer: &dyn ValueSerializerHelper,
  ) -> Option<bool> {
    let obj = self.obj(scope);
    let key = FastString::from_static("_writeHostObject")
      .v8_string(scope)
      .unwrap()
      .into();
    if let Some(v) = obj.get(scope, key)
      && let Ok(v) = v.try_cast::<v8::Function>()
    {
      v.call(scope, obj.into(), &[object.into()])?;
      return Some(true);
    }

    None
  }

  fn is_host_object<'s>(
    &self,
    _scope: &mut v8::PinScope<'s, '_>,
    _object: v8::Local<'s, v8::Object>,
  ) -> Option<bool> {
    // should never be called because has_custom_host_object returns false
    None
  }
}

#[op2]
#[cppgc]
pub fn op_v8_new_serializer(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
) -> Serializer<'static> {
  let delegate_state = Rc::new(SerializerDelegateState {
    obj: v8::TracedReference::new(scope, obj),
  });
  let inner = v8::ValueSerializer::new(
    scope,
    Box::new(SerializerDelegate {
      state: delegate_state.clone(),
    }),
  );
  Serializer {
    inner,
    delegate_state,
  }
}

#[op2(fast)]
pub fn op_v8_set_treat_array_buffer_views_as_host_objects(
  #[cppgc] ser: &Serializer,
  value: bool,
) {
  ser
    .inner
    .set_treat_array_buffer_views_as_host_objects(value);
}

#[op2]
pub fn op_v8_release_buffer(#[cppgc] ser: &Serializer) -> Uint8Array {
  ser.inner.release().into()
}

#[op2(fast)]
pub fn op_v8_transfer_array_buffer(
  #[cppgc] ser: &Serializer,
  #[smi] id: u32,
  array_buffer: v8::Local<v8::ArrayBuffer>,
) {
  ser.inner.transfer_array_buffer(id, array_buffer);
}

#[op2(fast)]
pub fn op_v8_write_double(#[cppgc] ser: &Serializer, double: f64) {
  ser.inner.write_double(double);
}

#[op2(fast)]
pub fn op_v8_write_header(#[cppgc] ser: &Serializer) {
  ser.inner.write_header();
}

#[op2]
pub fn op_v8_write_raw_bytes(
  #[cppgc] ser: &Serializer,
  #[anybuffer] source: &[u8],
) {
  ser.inner.write_raw_bytes(source);
}

#[op2(fast)]
pub fn op_v8_write_uint32(#[cppgc] ser: &Serializer, num: u32) {
  ser.inner.write_uint32(num);
}

#[op2(fast)]
pub fn op_v8_write_uint64(#[cppgc] ser: &Serializer, hi: u32, lo: u32) {
  let num = ((hi as u64) << 32) | (lo as u64);
  ser.inner.write_uint64(num);
}

#[op2(nofast, reentrant)]
pub fn op_v8_write_value(
  scope: &mut v8::PinScope<'_, '_>,
  #[cppgc] ser: &Serializer,
  value: v8::Local<v8::Value>,
) {
  let context = scope.get_current_context();
  ser.inner.write_value(context, value);
}

struct DeserBuffer {
  ptr: Option<NonNull<u8>>,
  // Hold onto backing store to keep the underlying buffer
  // alive while we hold a reference to it.
  _backing_store: v8::SharedRef<v8::BackingStore>,
}

pub struct Deserializer<'a> {
  buf: DeserBuffer,
  delegate_state: Rc<DeserializerDelegateState>,
  inner: v8::ValueDeserializer<'a>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Deserializer<'_> {
  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    visitor.trace(&self.delegate_state.obj);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Deserializer"
  }
}

struct DeserializerDelegateState {
  obj: v8::TracedReference<v8::Object>,
}

pub struct DeserializerDelegate {
  state: Rc<DeserializerDelegateState>,
}

impl v8::ValueDeserializerImpl for DeserializerDelegate {
  fn read_host_object<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    _value_deserializer: &dyn v8::ValueDeserializerHelper,
  ) -> Option<v8::Local<'s, v8::Object>> {
    let obj = self.state.obj.get(scope).unwrap();
    let key = FastString::from_static("_readHostObject")
      .v8_string(scope)
      .unwrap()
      .into();
    let scope = std::pin::pin!(v8::AllowJavascriptExecutionScope::new(scope));
    let scope = &mut scope.init();
    if let Some(v) = obj.get(scope, key)
      && let Ok(v) = v.try_cast::<v8::Function>()
    {
      let result = v.call(scope, obj.into(), &[])?;
      match result.try_cast() {
        Ok(res) => return Some(res),
        Err(_) => {
          let msg =
            FastString::from_static("readHostObject must return an object")
              .v8_string(scope)
              .unwrap();
          let error = v8::Exception::type_error(scope, msg);
          scope.throw_exception(error);
          return None;
        }
      }
    }
    None
  }
}

#[op2]
#[cppgc]
pub fn op_v8_new_deserializer(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<v8::Object>,
  buffer: v8::Local<v8::ArrayBufferView>,
) -> Result<Deserializer<'static>, JsErrorBox> {
  let offset = buffer.byte_offset();
  let len = buffer.byte_length();
  let backing_store = buffer.get_backing_store().ok_or_else(|| {
    JsErrorBox::generic("deserialization buffer has no backing store")
  })?;
  let (buf_slice, buf_ptr) = if let Some(data) = backing_store.data() {
    // SAFETY: the offset is valid for the underlying buffer because we're getting it directly from v8
    let data_ptr = unsafe { data.as_ptr().cast::<u8>().add(offset) };
    (
      // SAFETY: the len is valid, from v8, and the data_ptr is valid (as above)
      unsafe { std::slice::from_raw_parts(data_ptr.cast_const().cast(), len) },
      Some(data.cast()),
    )
  } else {
    (&[] as &[u8], None::<NonNull<u8>>)
  };
  let delegate_state = Rc::new(DeserializerDelegateState {
    obj: v8::TracedReference::new(scope, obj),
  });
  let inner = v8::ValueDeserializer::new(
    scope,
    Box::new(DeserializerDelegate {
      state: delegate_state.clone(),
    }),
    buf_slice,
  );
  Ok(Deserializer {
    inner,
    delegate_state,
    buf: DeserBuffer {
      _backing_store: backing_store,
      ptr: buf_ptr,
    },
  })
}

#[op2(fast)]
pub fn op_v8_transfer_array_buffer_de(
  #[cppgc] deser: &Deserializer,
  #[smi] id: u32,
  array_buffer: v8::Local<v8::Value>,
) -> Result<(), deno_core::error::DataError> {
  if let Ok(shared_array_buffer) =
    array_buffer.try_cast::<v8::SharedArrayBuffer>()
  {
    deser
      .inner
      .transfer_shared_array_buffer(id, shared_array_buffer)
  }
  let array_buffer = array_buffer.try_cast::<v8::ArrayBuffer>()?;
  deser.inner.transfer_array_buffer(id, array_buffer);
  Ok(())
}

#[op2(fast)]
pub fn op_v8_read_double(
  #[cppgc] deser: &Deserializer,
) -> Result<f64, JsErrorBox> {
  let mut double = 0f64;
  if !deser.inner.read_double(&mut double) {
    return Err(JsErrorBox::type_error("ReadDouble() failed"));
  }
  Ok(double)
}

#[op2(nofast)]
pub fn op_v8_read_header(
  scope: &mut v8::PinScope<'_, '_>,
  #[cppgc] deser: &Deserializer,
) -> bool {
  let context = scope.get_current_context();
  let res = deser.inner.read_header(context);
  res.unwrap_or_default()
}

#[op2(fast)]
#[number]
pub fn op_v8_read_raw_bytes(
  #[cppgc] deser: &Deserializer,
  #[number] length: usize,
) -> usize {
  let Some(buf_ptr) = deser.buf.ptr else {
    return 0;
  };
  if let Some(buf) = deser.inner.read_raw_bytes(length) {
    let ptr = buf.as_ptr();
    (ptr as usize) - (buf_ptr.as_ptr() as usize)
  } else {
    0
  }
}

#[op2(fast)]
pub fn op_v8_read_uint32(
  #[cppgc] deser: &Deserializer,
) -> Result<u32, JsErrorBox> {
  let mut value = 0;
  if !deser.inner.read_uint32(&mut value) {
    return Err(JsErrorBox::type_error("ReadUint32() failed"));
  }

  Ok(value)
}

#[op2]
pub fn op_v8_read_uint64(
  #[cppgc] deser: &Deserializer,
) -> Result<(u32, u32), JsErrorBox> {
  let mut val = 0;
  if !deser.inner.read_uint64(&mut val) {
    return Err(JsErrorBox::type_error("ReadUint64() failed"));
  }

  Ok(((val >> 32) as u32, val as u32))
}

#[op2(fast)]
pub fn op_v8_get_wire_format_version(#[cppgc] deser: &Deserializer) -> u32 {
  deser.inner.get_wire_format_version()
}

#[op2(reentrant)]
pub fn op_v8_read_value<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[cppgc] deser: &Deserializer,
) -> v8::Local<'s, v8::Value> {
  let context = scope.get_current_context();
  let val = deser.inner.read_value(context);
  val.unwrap_or_else(|| v8::null(scope).into())
}

// --- GCProfiler -----------------------------------------------------------
//
// Implements `v8.GCProfiler`, a thin per-instance recorder that hooks the
// V8 GC prologue/epilogue callbacks. Each active profiler captures heap and
// heap-space statistics on every GC and records the wall-clock cost.

#[derive(Default)]
struct GcProfilerRegistryInner {
  next_id: u64,
  // Profilers that have been started but not yet stopped.
  profilers: HashMap<u64, GcProfilerState>,
  callbacks_registered: bool,
}

struct GcProfilerRegistry {
  inner: Rc<RefCell<GcProfilerRegistryInner>>,
}

struct GcProfilerState {
  pending_before: Option<GcSnapshot>,
  pending_start: Option<Instant>,
  statistics: Vec<GcStat>,
}

#[derive(Clone)]
struct GcSnapshot {
  // total_heap_size, total_heap_size_executable, total_physical_size,
  // total_available_size, used_heap_size, heap_size_limit,
  // malloced_memory, peak_malloced_memory,
  // total_global_handles_size, used_global_handles_size, external_memory.
  heap: [f64; 11],
  spaces: Vec<HeapSpaceSnapshot>,
}

#[derive(Clone)]
struct HeapSpaceSnapshot {
  name: String,
  size: f64,
  used_size: f64,
  available_size: f64,
  physical_size: f64,
}

struct GcStat {
  gc_type: &'static str,
  // Cost in nanoseconds (matches Node.js).
  cost_ns: f64,
  before: GcSnapshot,
  after: GcSnapshot,
}

fn gc_type_name(gc_type: v8::GCType) -> &'static str {
  // V8 callbacks may surface combined flags (e.g. kGCTypeIncrementalMarking |
  // kGCTypeMarkSweepCompact). Pick the lowest-priority single bit so the
  // returned label is stable and informative.
  match gc_type {
    v8::GCType::kGCTypeScavenge => "Scavenge",
    v8::GCType::kGCTypeMinorMarkSweep => "MinorMarkSweep",
    v8::GCType::kGCTypeMarkSweepCompact => "MarkSweepCompact",
    v8::GCType::kGCTypeIncrementalMarking => "IncrementalMarking",
    v8::GCType::kGCTypeProcessWeakCallbacks => "ProcessWeakCallbacks",
    v8::GCType::kGCTypeAll => "All",
    _ => "Unknown",
  }
}

fn capture_snapshot(isolate: &mut v8::Isolate) -> GcSnapshot {
  let h = isolate.get_heap_statistics();
  let heap = [
    h.total_heap_size() as f64,
    h.total_heap_size_executable() as f64,
    h.total_physical_size() as f64,
    h.total_available_size() as f64,
    h.used_heap_size() as f64,
    h.heap_size_limit() as f64,
    h.malloced_memory() as f64,
    h.peak_malloced_memory() as f64,
    h.total_global_handles_size() as f64,
    h.used_global_handles_size() as f64,
    h.external_memory() as f64,
  ];
  let nspaces = isolate.number_of_heap_spaces();
  let mut spaces = Vec::with_capacity(nspaces);
  for i in 0..nspaces {
    if let Some(s) = isolate.get_heap_space_statistics(i) {
      spaces.push(HeapSpaceSnapshot {
        name: s.space_name().to_string_lossy().into_owned(),
        size: s.space_size() as f64,
        used_size: s.space_used_size() as f64,
        available_size: s.space_available_size() as f64,
        physical_size: s.physical_space_size() as f64,
      });
    }
  }
  GcSnapshot { heap, spaces }
}

fn registry_rc(
  isolate: &v8::Isolate,
) -> Option<Rc<RefCell<GcProfilerRegistryInner>>> {
  isolate
    .get_slot::<GcProfilerRegistry>()
    .map(|r| r.inner.clone())
}

extern "C" fn gc_prologue_callback(
  isolate: v8::UnsafeRawIsolatePtr,
  _gc_type: v8::GCType,
  _flags: v8::GCCallbackFlags,
  _data: *mut c_void,
) {
  // SAFETY: V8 guarantees the isolate is valid during this callback.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr_unchecked(isolate) };
  let Some(rc) = registry_rc(&isolate) else {
    return;
  };
  // Bail out fast if no profilers are active so we don't capture heap
  // statistics on every GC unnecessarily.
  if rc.borrow().profilers.is_empty() {
    return;
  }
  let snapshot = capture_snapshot(&mut isolate);
  let now = Instant::now();
  let mut inner = rc.borrow_mut();
  for state in inner.profilers.values_mut() {
    state.pending_before = Some(snapshot.clone());
    state.pending_start = Some(now);
  }
}

extern "C" fn gc_epilogue_callback(
  isolate: v8::UnsafeRawIsolatePtr,
  gc_type: v8::GCType,
  _flags: v8::GCCallbackFlags,
  _data: *mut c_void,
) {
  // SAFETY: V8 guarantees the isolate is valid during this callback.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr_unchecked(isolate) };
  let Some(rc) = registry_rc(&isolate) else {
    return;
  };
  if rc.borrow().profilers.is_empty() {
    return;
  }
  let snapshot = capture_snapshot(&mut isolate);
  let now = Instant::now();
  let gc_type_str = gc_type_name(gc_type);
  let mut inner = rc.borrow_mut();
  for state in inner.profilers.values_mut() {
    let (Some(before), Some(start)) =
      (state.pending_before.take(), state.pending_start.take())
    else {
      continue;
    };
    let cost_ns = now.saturating_duration_since(start).as_nanos() as f64;
    state.statistics.push(GcStat {
      gc_type: gc_type_str,
      cost_ns,
      before,
      after: snapshot.clone(),
    });
  }
}

fn ensure_registry(
  scope: &mut v8::PinScope<'_, '_>,
) -> Rc<RefCell<GcProfilerRegistryInner>> {
  if let Some(existing) = scope.get_slot::<GcProfilerRegistry>() {
    return existing.inner.clone();
  }
  let inner = Rc::new(RefCell::new(GcProfilerRegistryInner::default()));
  scope.set_slot(GcProfilerRegistry {
    inner: inner.clone(),
  });
  inner
}

fn ensure_callbacks_registered(
  scope: &mut v8::PinScope<'_, '_>,
  inner: &Rc<RefCell<GcProfilerRegistryInner>>,
) {
  if inner.borrow().callbacks_registered {
    return;
  }
  scope.add_gc_prologue_callback(
    gc_prologue_callback,
    std::ptr::null_mut(),
    v8::GCType::kGCTypeAll,
  );
  scope.add_gc_epilogue_callback(
    gc_epilogue_callback,
    std::ptr::null_mut(),
    v8::GCType::kGCTypeAll,
  );
  inner.borrow_mut().callbacks_registered = true;
}

pub struct GcProfilerHandle {
  id: std::cell::Cell<Option<u64>>,
}

// SAFETY: GcProfilerHandle has no traceable references.
unsafe impl GarbageCollected for GcProfilerHandle {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GcProfilerHandle"
  }
}

#[op2]
#[cppgc]
pub fn op_v8_gc_profiler_new() -> GcProfilerHandle {
  GcProfilerHandle {
    id: std::cell::Cell::new(None),
  }
}

#[op2(fast)]
pub fn op_v8_gc_profiler_start(
  scope: &mut v8::PinScope<'_, '_>,
  #[cppgc] handle: &GcProfilerHandle,
) {
  if handle.id.get().is_some() {
    return;
  }
  let inner = ensure_registry(scope);
  ensure_callbacks_registered(scope, &inner);
  let id = {
    let mut borrow = inner.borrow_mut();
    let id = borrow.next_id;
    borrow.next_id = borrow.next_id.wrapping_add(1);
    borrow.profilers.insert(
      id,
      GcProfilerState {
        pending_before: None,
        pending_start: None,
        statistics: Vec::new(),
      },
    );
    id
  };
  handle.id.set(Some(id));
}

#[op2]
pub fn op_v8_gc_profiler_stop<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[cppgc] handle: &GcProfilerHandle,
) -> v8::Local<'s, v8::Value> {
  let Some(id) = handle.id.take() else {
    return v8::null(scope).into();
  };
  let Some(inner) = scope
    .get_slot::<GcProfilerRegistry>()
    .map(|r| r.inner.clone())
  else {
    return v8::null(scope).into();
  };
  let state = inner.borrow_mut().profilers.remove(&id);
  let Some(state) = state else {
    return v8::null(scope).into();
  };
  build_report(scope, &state.statistics).into()
}

const HEAP_KEYS: &[&str] = &[
  "totalHeapSize",
  "totalHeapSizeExecutable",
  "totalPhysicalSize",
  "totalAvailableSize",
  "usedHeapSize",
  "heapSizeLimit",
  "mallocedMemory",
  "peakMallocedMemory",
  "totalGlobalHandlesSize",
  "usedGlobalHandlesSize",
  "externalMemory",
];

fn build_snapshot<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  snap: &GcSnapshot,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);

  let heap_stats = v8::Object::new(scope);
  for (i, key) in HEAP_KEYS.iter().enumerate() {
    let k = v8::String::new(scope, key).unwrap();
    let v = v8::Number::new(scope, snap.heap[i]);
    heap_stats.set(scope, k.into(), v.into());
  }
  let k = v8::String::new(scope, "heapStatistics").unwrap();
  obj.set(scope, k.into(), heap_stats.into());

  let spaces_array = v8::Array::new(scope, snap.spaces.len() as i32);
  for (i, space) in snap.spaces.iter().enumerate() {
    let space_obj = v8::Object::new(scope);
    let k = v8::String::new(scope, "spaceName").unwrap();
    let name = v8::String::new(scope, &space.name).unwrap();
    space_obj.set(scope, k.into(), name.into());
    for (name, value) in [
      ("spaceSize", space.size),
      ("spaceUsedSize", space.used_size),
      ("spaceAvailableSize", space.available_size),
      ("physicalSpaceSize", space.physical_size),
    ] {
      let k = v8::String::new(scope, name).unwrap();
      let v = v8::Number::new(scope, value);
      space_obj.set(scope, k.into(), v.into());
    }
    spaces_array.set_index(scope, i as u32, space_obj.into());
  }
  let k = v8::String::new(scope, "heapSpaceStatistics").unwrap();
  obj.set(scope, k.into(), spaces_array.into());

  obj
}

fn build_report<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  stats: &[GcStat],
) -> v8::Local<'s, v8::Object> {
  let arr = v8::Array::new(scope, stats.len() as i32);
  for (i, stat) in stats.iter().enumerate() {
    let entry = v8::Object::new(scope);

    let k = v8::String::new(scope, "gcType").unwrap();
    let v = v8::String::new(scope, stat.gc_type).unwrap();
    entry.set(scope, k.into(), v.into());

    let k = v8::String::new(scope, "cost").unwrap();
    let v = v8::Number::new(scope, stat.cost_ns);
    entry.set(scope, k.into(), v.into());

    let before = build_snapshot(scope, &stat.before);
    let k = v8::String::new(scope, "beforeGC").unwrap();
    entry.set(scope, k.into(), before.into());

    let after = build_snapshot(scope, &stat.after);
    let k = v8::String::new(scope, "afterGC").unwrap();
    entry.set(scope, k.into(), after.into());

    arr.set_index(scope, i as u32, entry.into());
  }
  let wrapper = v8::Object::new(scope);
  let k = v8::String::new(scope, "statistics").unwrap();
  wrapper.set(scope, k.into(), arr.into());
  wrapper
}
