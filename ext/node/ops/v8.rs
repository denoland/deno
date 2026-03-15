// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::io::BufWriter;
use std::io::Write;
use std::ptr::NonNull;
use std::rc::Rc;

use deno_core::FastString;
use deno_core::GarbageCollected;
use deno_core::InspectorMsg;
use deno_core::InspectorMsgKind;
use deno_core::InspectorSessionKind;
use deno_core::JsRuntimeInspector;
use deno_core::OpState;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::v8;
use deno_error::JsErrorBox;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

#[op2(fast)]
pub fn op_v8_cached_data_version_tag() -> u32 {
  v8::script_compiler::cached_data_version_tag()
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

/// Persistent coverage connection that stores an inspector session,
/// similar to Node.js's V8CoverageConnection.
pub struct V8CoverageConnection {
  session: deno_core::LocalInspectorSession,
  next_id: std::cell::Cell<i32>,
  coverage_dir: std::path::PathBuf,
  response: Rc<RefCell<Option<String>>>,
}

impl V8CoverageConnection {
  pub fn new(
    inspector: Rc<JsRuntimeInspector>,
    coverage_dir: std::path::PathBuf,
  ) -> Self {
    let response: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let response_clone = response.clone();

    let callback: Box<dyn Fn(InspectorMsg)> =
      Box::new(move |msg: InspectorMsg| {
        if let InspectorMsgKind::Message(_id) = msg.kind {
          *response_clone.borrow_mut() = Some(msg.content);
        }
      });

    let session = JsRuntimeInspector::create_local_session(
      inspector,
      callback,
      InspectorSessionKind::NonBlocking {
        wait_for_disconnect: false,
      },
    );

    Self {
      session,
      next_id: std::cell::Cell::new(1),
      coverage_dir,
      response,
    }
  }

  fn next_id(&self) -> i32 {
    let id = self.next_id.get();
    self.next_id.set(id + 1);
    id
  }

  pub fn start(&mut self) {
    self
      .session
      .post_message::<()>(self.next_id(), "Profiler.enable", None);
    self.session.post_message(
      self.next_id(),
      "Profiler.startPreciseCoverage",
      Some(serde_json::json!({
        "callCount": true,
        "detailed": true,
      })),
    );
  }

  pub fn take_coverage(&mut self) {
    self.session.post_message::<()>(
      self.next_id(),
      "Profiler.takePreciseCoverage",
      None,
    );

    if let Some(response) = self.response.borrow_mut().take() {
      self.write_coverage(&response);
    }
  }

  pub fn stop_coverage(&mut self) {
    self.session.post_message::<()>(
      self.next_id(),
      "Profiler.stopPreciseCoverage",
      None,
    );
    self
      .session
      .post_message::<()>(self.next_id(), "Profiler.disable", None);
  }

  #[allow(clippy::disallowed_methods)]
  fn write_coverage(&self, response: &str) {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(response)
    else {
      return;
    };
    let Some(coverages) = message.get("result").and_then(|r| r.get("result"))
    else {
      return;
    };
    let Ok(script_coverages) =
      serde_json::from_value::<Vec<serde_json::Value>>(coverages.clone())
    else {
      return;
    };

    if let Err(e) = std::fs::create_dir_all(&self.coverage_dir) {
      log::error!(
        "Failed to create coverage dir {:?}: {:?}",
        self.coverage_dir,
        e
      );
      return;
    }

    for coverage in script_coverages {
      let url = coverage.get("url").and_then(|u| u.as_str()).unwrap_or("");
      if url.is_empty()
        || url.starts_with("ext:")
        || url.starts_with("[ext:")
        || url.starts_with("node:")
      {
        continue;
      }
      static COVERAGE_FILE_COUNTER: std::sync::atomic::AtomicU32 =
        std::sync::atomic::AtomicU32::new(0);
      let seq = COVERAGE_FILE_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      let pid = std::process::id();
      let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
      let filename = format!("coverage-{}-{}-{:08}.json", pid, now, seq);
      let filepath = self.coverage_dir.join(&filename);
      let file = match std::fs::File::create(&filepath) {
        Ok(f) => f,
        Err(e) => {
          log::error!("Failed to create coverage file {:?}: {:?}", filepath, e);
          continue;
        }
      };
      let mut out = BufWriter::new(file);
      let json = serde_json::to_string_pretty(&coverage).unwrap();
      if let Err(e) = out.write_all(json.as_bytes()) {
        log::error!("Failed to write coverage file {:?}: {:?}", filepath, e);
      }
      let _ = out.flush();
    }
  }
}

/// Called during bootstrap to start coverage collection
/// if NODE_V8_COVERAGE env var is set.
#[op2(fast)]
pub fn op_v8_start_coverage(state: &mut OpState) {
  if state.has::<V8CoverageConnection>() {
    return;
  }
  let coverage_dir = match std::env::var("NODE_V8_COVERAGE") {
    Ok(dir) if !dir.is_empty() => std::path::PathBuf::from(dir),
    _ => return,
  };
  let Some(inspector) = state.try_borrow::<Rc<JsRuntimeInspector>>() else {
    return;
  };
  let inspector = inspector.clone();
  let mut connection = V8CoverageConnection::new(inspector, coverage_dir);
  connection.start();
  state.put(connection);
}

#[op2(fast)]
pub fn op_v8_take_coverage(state: &mut OpState) {
  if let Some(connection) = state.try_borrow_mut::<V8CoverageConnection>() {
    connection.take_coverage();
  }
}

#[op2(fast)]
pub fn op_v8_stop_coverage(state: &mut OpState) {
  if let Some(connection) = state.try_borrow_mut::<V8CoverageConnection>() {
    connection.stop_coverage();
  }
}

pub struct Serializer<'a> {
  inner: v8::ValueSerializer<'a>,
}

pub struct SerializerDelegate {
  obj: v8::Global<v8::Object>,
}

// SAFETY: we're sure this can be GCed
unsafe impl v8::cppgc::GarbageCollected for Serializer<'_> {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Serializer"
  }
}

impl SerializerDelegate {
  fn obj<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Object> {
    v8::Local::new(scope, &self.obj)
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
  let obj = v8::Global::new(scope, obj);
  let inner =
    v8::ValueSerializer::new(scope, Box::new(SerializerDelegate { obj }));
  Serializer { inner }
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
  inner: v8::ValueDeserializer<'a>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for Deserializer<'_> {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Deserializer"
  }
}

pub struct DeserializerDelegate {
  obj: v8::TracedReference<v8::Object>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for DeserializerDelegate {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.obj);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DeserializerDelegate"
  }
}

impl v8::ValueDeserializerImpl for DeserializerDelegate {
  fn read_host_object<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    _value_deserializer: &dyn v8::ValueDeserializerHelper,
  ) -> Option<v8::Local<'s, v8::Object>> {
    let obj = self.obj.get(scope).unwrap();
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
  let obj = v8::TracedReference::new(scope, obj);
  let inner = v8::ValueDeserializer::new(
    scope,
    Box::new(DeserializerDelegate { obj }),
    buf_slice,
  );
  Ok(Deserializer {
    inner,
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
