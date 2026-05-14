// Copyright 2018-2026 the Deno authors. MIT license.
//
// qjs_v8_compat: a rusty_v8-shaped API surface backed by QuickJS-ng.
//
// # What this crate is
//
// A compat layer that re-exports a `v8` module mirroring the shape of the
// `rusty_v8` API, but implemented on top of QuickJS-ng's reference-counted
// C runtime. With the `quickjs` feature on, deno_core can `use v8` from
// this crate instead of `rusty_v8` and target an embedded JS engine with
// ~1ms cold start and ~700KB binary footprint.
//
// # GC model translation
//
// V8 is a tracing GC with rooted handle scopes; QuickJS-ng is refcounted.
// We bridge the two by treating each `HandleScope` as a *frame* on a stack
// of owned `JSValue`s. Constructing a `Local<'s, T>` pushes the value onto
// the current frame; dropping the frame `JS_FreeValue`s everything it
// still owns. `Global<T>` takes its own ref via `JS_DupValue` on creation
// and frees on `Drop`.
//
// This invariant — every JSValue belongs to exactly one frame at all times,
// and is exactly once dropped — is the whole game. The tests in
// `tests/refcount.rs` exercise it with a mock backend.
//
// # Status
//
// This is an initial scaffold. It compiles, exposes the type surface
// deno_core imports, and ships a pure-Rust mock backend that lets the GC
// invariants be tested without a linked QuickJS-ng. Wiring it as the
// engine for deno_core (so that `cargo test -p deno_core --features quickjs`
// works) is the follow-up. See `ARCHITECTURE.md` for the integration plan.
//
// Every place where QuickJS-ng diverges from V8 in observable semantics is
// marked with `// QJS-DIVERGE:` and a short note.

#![allow(clippy::missing_safety_doc)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

pub mod ffi;
pub mod sys;

pub mod arena;
pub mod buffer;
pub mod context;
pub mod exception;
pub mod external;
pub mod function;
pub mod isolate;
pub mod module;
pub mod object;
pub mod primitives;
pub mod promise;
pub mod scope;
pub mod script;
pub mod snapshot;
pub mod template;
pub mod value;

// The big idea: deno_core does `use v8` and expects rusty_v8's surface.
// This module re-exports everything under that name so deno_core can pick
// us with a feature flag and otherwise be unchanged.
// Also re-export the v8-shaped surface at the crate root so deno_core can
// alias the whole crate as `v8` (`extern crate qjs_v8_compat as v8;`) and
// have `use v8::Local;` resolve correctly. Without this, only the `v8`
// submodule path works, which forces every internal `use v8::*` site in
// deno_core to be touched.
pub use crate::buffer::*;
pub use crate::context::*;
pub use crate::exception::*;
pub use crate::external::*;
pub use crate::function::*;
pub use crate::isolate::*;
pub use crate::module::*;
pub use crate::object::*;
pub use crate::primitives::*;
pub use crate::promise::*;
// And the sub-namespaces — these have to be explicit `pub use` because
// glob re-export doesn't include modules.
pub use crate::scope::PinnedRef;
pub use crate::scope::*;
pub use crate::script::*;
pub use crate::snapshot::*;
pub use crate::template::*;
// Typed-array, V8 init, and oddball type stubs that op2 + ext/* crates
// reference by name. They live inside the v8 submodule for
// `qjs_v8_compat::v8::Int8Array` callers and are re-exported at the
// crate root for `extern crate qjs_v8_compat as v8;` callers.
pub use crate::v8::BigInt64Array;
pub use crate::v8::BigUint64Array;
pub use crate::v8::DataView;
pub use crate::v8::Float32Array;
pub use crate::v8::Float64Array;
pub use crate::v8::FunctionBuilder;
pub use crate::v8::IdleTask;
pub use crate::v8::Int8Array;
pub use crate::v8::Int16Array;
pub use crate::v8::Int32;
pub use crate::v8::Int32Array;
pub use crate::v8::NearHeapLimitCallback;
pub use crate::v8::PlatformImpl;
pub use crate::v8::Task;
pub use crate::v8::Uint8ClampedArray;
pub use crate::v8::Uint16Array;
pub use crate::v8::Uint32;
pub use crate::v8::Uint32Array;
pub use crate::v8::UniquePtr;
pub use crate::v8::V8;
pub use crate::v8::WasmModuleObject;
pub use crate::v8::WasmStreaming;
pub use crate::v8::WriteFlags;
pub use crate::v8::cppgc;
pub use crate::v8::data;
pub use crate::v8::fast_api;
pub use crate::v8::icu;
pub use crate::v8::inspector;
pub use crate::v8::json;
pub use crate::v8::latin1_to_utf8;
pub use crate::v8::new_custom_platform;
pub use crate::v8::null;
pub use crate::v8::script_compiler;
pub use crate::v8::simdutf;
pub use crate::v8::undefined;
pub use crate::value::*;

/// Mirror of `v8::scope!(let name, parent)` — the rusty_v8 declarative
/// macro that elides a handle-scope rooted on `parent` into the local
/// binding. The bare `($name, $parent)` form binds `$name` as a
/// `&mut HandleScope` (matching what rusty_v8 produces) so call sites
/// like `Local::new($scope, handle)` don't move a value-typed scope.
#[macro_export]
macro_rules! scope {
  (let $name:ident, $parent:expr) => {
    let mut __scope_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __scope_inner);
  };
  ($name:ident, $parent:expr) => {
    let mut __scope_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __scope_inner);
  };
}

/// Mirror of `v8::tc_scope!(let name, parent)`. Creates a
/// `TryCatch`-wrapped HandleScope.
#[macro_export]
macro_rules! tc_scope {
  (let $name:ident, $parent:expr) => {
    let mut __tc_inner = $crate::HandleScope::new($parent);
    let mut __tc_outer = $crate::TryCatch::new(&mut __tc_inner);
    let $name = &mut __tc_outer;
  };
  ($name:ident, $parent:expr) => {
    let mut __tc_inner = $crate::HandleScope::new($parent);
    let mut __tc_outer = $crate::TryCatch::new(&mut __tc_inner);
    let $name = &mut __tc_outer;
  };
}

/// Mirror of `v8::callback_scope!(unsafe name, raw)` (and the
/// `let`/bare variants). The `unsafe` token mirrors rusty_v8's macro,
/// which marks the call site as constructing a CallbackScope from a raw
/// pointer V8 hands the host — same shape on the QuickJS side, so we
/// accept the keyword and discard it.
#[macro_export]
macro_rules! callback_scope {
  (unsafe $name:ident, $raw:expr) => {
    let mut __cb_inner = unsafe { $crate::CallbackScope::new($raw) };
    let $name = &mut __cb_inner;
  };
  (let $name:ident, $raw:expr) => {
    let mut __cb_inner = unsafe { $crate::CallbackScope::new($raw) };
    let $name = &mut __cb_inner;
  };
  ($name:ident, $raw:expr) => {
    let mut __cb_inner = unsafe { $crate::CallbackScope::new($raw) };
    let $name = &mut __cb_inner;
  };
}

/// Mirror of `v8::isolate_scope!(let name, isolate)`.
#[macro_export]
macro_rules! isolate_scope {
  (let $name:ident, $isolate:expr) => {
    let mut __iso_inner = $crate::HandleScope::new($isolate);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __iso_inner);
  };
  ($name:ident, $isolate:expr) => {
    let mut __iso_inner = $crate::HandleScope::new($isolate);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __iso_inner);
  };
}

/// Mirror of `v8::scope_with_context!(let name, isolate, context)`.
/// On QuickJS we only have one context per JSContext, so the explicit
/// context parameter is accepted and ignored. Trailing commas allowed.
#[macro_export]
macro_rules! scope_with_context {
  (let $name:ident, $parent:expr, $_ctx:expr $(,)?) => {
    let mut __swc_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __swc_inner);
  };
  ($name:ident, $parent:expr, $_ctx:expr $(,)?) => {
    let mut __swc_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __swc_inner);
  };
}

/// Mirror of `v8::escapable_handle_scope!(let name, parent)`. On the
/// QuickJS backend we don't enforce the escape semantics statically;
/// the macro just creates an `EscapableHandleScope`.
#[macro_export]
macro_rules! escapable_handle_scope {
  (let $name:ident, $parent:expr) => {
    let mut __esc_inner = $crate::EscapableHandleScope::new($parent);
    let $name = &mut __esc_inner;
  };
  ($name:ident, $parent:expr) => {
    let mut __esc_inner = $crate::EscapableHandleScope::new($parent);
    let $name = &mut __esc_inner;
  };
}

/// Mirror of `v8::context_scope!(let name, parent)` and the
/// `(name, this, isolate)` 3-arg form deno_core's jsrealm macros use.
#[macro_export]
macro_rules! context_scope {
  (let $name:ident, $parent:expr) => {
    let mut __cs_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __cs_inner);
  };
  ($name:ident, $parent:expr) => {
    let mut __cs_inner = $crate::HandleScope::new($parent);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __cs_inner);
  };
  ($name:ident, $this:expr, $isolate:expr) => {
    let mut __cs_inner = $crate::HandleScope::new($isolate);
    let $name = $crate::scope::PinScope::from_handle_scope_mut(&mut __cs_inner);
  };
}

pub mod v8 {
  pub use crate::buffer::*;
  pub use crate::context::*;
  pub use crate::exception::*;
  pub use crate::external::*;
  pub use crate::function::*;
  pub use crate::isolate::*;
  pub use crate::module::*;
  pub use crate::object::*;
  pub use crate::primitives::*;
  pub use crate::promise::*;
  pub use crate::scope::*;
  pub use crate::script::*;
  pub use crate::snapshot::*;
  pub use crate::template::*;
  pub use crate::value::*;

  // Sub-namespaces that deno_core imports as `v8::foo`.
  pub mod cppgc {
    //! Stub — QuickJS has no cppgc equivalent. QJS-DIVERGE: cppgc has no
    //! analog. We expose empty types so generic code compiles; using any
    //! of them at runtime is unsupported.

    /// Marker trait — same shape as rusty_v8's `cppgc::GarbageCollected`.
    /// On QuickJS no specialized GC tracing happens; the JSRuntime's
    /// refcount is the only mechanism. Marked `unsafe` to match rusty_v8;
    /// deno_core's `unsafe impl GarbageCollected for ...` blocks rely on
    /// that.
    pub unsafe trait GarbageCollected {
      fn trace(&self, _visitor: &mut Visitor) {}
      fn get_name(&self) -> &'static core::ffi::CStr {
        c"qjs::GarbageCollected"
      }
    }

    /// Trait stub for `cppgc::Traced`. Mirror of rusty_v8 — marked
    /// unsafe to match the upstream impl signature.
    pub unsafe trait Traced {
      fn trace(&self, _visitor: &mut Visitor) {}
    }

    /// Stub for `cppgc::make_garbage_collected`. On V8 this allocates a
    /// `Member<T>` in the cppgc heap; on QuickJS we just box the value.
    /// Accepts an optional heap argument (rusty_v8 takes `(heap, value)`
    /// in newer versions and `(value)` in older).
    pub fn make_garbage_collected<H, T: 'static>(_heap: H, value: T) -> Member<T> {
      let _ = std::boxed::Box::new(value);
      Member(core::marker::PhantomData)
    }

    pub struct Member<T>(core::marker::PhantomData<T>);
    pub struct Ptr<T>(core::marker::PhantomData<T>);
    pub struct Persistent<T>(core::marker::PhantomData<T>);
    pub struct GcCell<T>(core::marker::PhantomData<T>);
    pub struct UnsafePtr<T>(core::marker::PhantomData<T>);
    pub struct Visitor;

    impl<T> Member<T> {
      pub fn new<U>(_value: &U) -> Self {
        Self(core::marker::PhantomData)
      }
      pub fn get(&self) -> Option<&T> {
        None
      }
    }
    impl<T> Persistent<T> {
      pub fn new<U>(_value: &U) -> Self {
        Self(core::marker::PhantomData)
      }
      pub fn get(&self) -> Option<&T> {
        None
      }
    }
    impl<T> UnsafePtr<T> {
      /// SAFETY: caller guarantees the pointer is still valid.
      /// Stub — returns a reference to a dangling pointer (cppgc isn't
      /// wired up on the QuickJS backend, so this is unreachable in
      /// practice).
      pub unsafe fn as_ref<'a>(&'a self) -> &'a T {
        let p: *const T = core::ptr::dangling();
        unsafe { &*p }
      }
    }
    impl Visitor {
      pub fn trace<T>(&mut self, _member: &Member<T>) {}
    }

    pub fn initalize_process() {}
    pub fn shutdown_process() {}
  }

  pub mod fast_api {
    //! Stub — QuickJS has no JIT and no fast-API analog. Fast-api call
    //! paths fall through to the slow path on the QuickJS backend.
    //!
    //! The op2 macro generates code that constructs `CFunction` /
    //! `CFunctionInfo` / `CTypeInfo` descriptors and reads
    //! `FastApiCallbackOptions::data`. We mirror their shape so the
    //! generated code compiles; the slow-path callbacks fire instead
    //! and the descriptors are never consulted at runtime.

    use core::marker::PhantomData;

    pub struct FastApiCallbackOptions<'s> {
      pub data: super::Local<'s, super::Value>,
      _scope: PhantomData<&'s ()>,
    }

    impl<'s> FastApiCallbackOptions<'s> {
      pub fn data(&self) -> super::Local<'s, super::Value> {
        self.data
      }
    }

    /// Mirror of v8's `CTypeInfo`. We accept the same constructor args
    /// and store nothing — the descriptor is never inspected because
    /// fast paths are disabled on QuickJS.
    #[derive(Copy, Clone)]
    pub struct CTypeInfo {
      _ty: Type,
      _seq: SequenceType,
      _flags: Flags,
    }

    impl CTypeInfo {
      /// Mirrors rusty_v8's `CTypeInfo::new(ty, flags)` (2 args). The
      /// sequence type defaults to Scalar.
      pub const fn new(ty: Type, flags: Flags) -> Self {
        Self {
          _ty: ty,
          _seq: SequenceType::Scalar,
          _flags: flags,
        }
      }
      /// Variant including sequence type explicitly.
      pub const fn new_with_seq(
        ty: Type,
        seq: SequenceType,
        flags: Flags,
      ) -> Self {
        Self {
          _ty: ty,
          _seq: seq,
          _flags: flags,
        }
      }
    }

    /// Mirror of v8's `CFunctionInfo`. Stores its descriptor pointers as
    /// raw pointers; on QuickJS the fast-call dispatcher doesn't read
    /// them so we keep them as opaque addresses.
    pub struct CFunctionInfo {
      _return_info: CTypeInfo,
      _args: &'static [CTypeInfo],
      _i64: Int64Representation,
    }

    impl CFunctionInfo {
      /// Mirror of v8's `CFunctionInfo::new(return_type, &args, repr)` —
      /// the const-callable shape the op2 macro emits at compile time.
      pub const fn new(
        return_info: CTypeInfo,
        args: &'static [CTypeInfo],
        i64: Int64Representation,
      ) -> Self {
        Self {
          _return_info: return_info,
          _args: args,
          _i64: i64,
        }
      }
    }

    #[derive(Copy, Clone)]
    pub struct CFunction {
      _addr: *const core::ffi::c_void,
      _info: *const CFunctionInfo,
    }
    unsafe impl Send for CFunction {}
    unsafe impl Sync for CFunction {}

    impl CFunction {
      pub const fn new(
        addr: *const core::ffi::c_void,
        info: *const CFunctionInfo,
      ) -> Self {
        Self {
          _addr: addr,
          _info: info,
        }
      }
      pub const fn address(&self) -> *const core::ffi::c_void {
        self._addr
      }
      pub const fn type_info(&self) -> *const CFunctionInfo {
        self._info
      }
    }
    impl FastApiOneByteString {
      pub fn as_bytes(&self) -> &[u8] {
        unsafe {
          core::slice::from_raw_parts(self.data, self.length as usize)
        }
      }
    }

    #[repr(u8)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum Type {
      Void,
      Bool,
      Uint8,
      Uint32,
      Int32,
      Int64,
      Uint64,
      Float32,
      Float64,
      Pointer,
      V8Value,
      SeqOneByteString,
      ApiObject,
      Any,
      CallbackOptions,
    }

    impl Type {
      /// Mirror of rusty_v8's `Type::as_info()` — returns a CTypeInfo
      /// describing this type with default flags. The op2 macro uses
      /// this to build CFunctionInfo descriptors.
      pub const fn as_info(self) -> CTypeInfo {
        CTypeInfo::new(self, Flags::NONE)
      }
    }

    #[repr(u8)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum SequenceType {
      Scalar,
      IsSequence,
      IsTypedArray,
      IsArrayBuffer,
    }

    /// Mirror of v8's `fast_api::Flags`. Bit-flag set; the values match
    /// V8's enum where they exist. The fast-call dispatcher on QuickJS
    /// never reads these so we just store them.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct Flags(pub u8);

    impl Flags {
      pub const NONE: Self = Self(0);
      pub const ALLOW_SHARED: Self = Self(1 << 0);
      pub const ENFORCE_RANGE: Self = Self(1 << 1);
      pub const CLAMP: Self = Self(1 << 2);
      pub const fn empty() -> Self {
        Self(0)
      }
      pub const fn bits(self) -> u8 {
        self.0
      }
    }

    impl core::ops::BitOr for Flags {
      type Output = Self;
      fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
      }
    }

    #[repr(u8)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum Int64Representation {
      Number,
      BigInt,
    }

    /// Mirror of v8's `fast_api::FastApiOneByteString`. The op2 macro
    /// generates fast-call signatures that name this; the slow-path
    /// dispatcher fires instead under QuickJS, so this is a shape stub.
    #[repr(C)]
    pub struct FastApiOneByteString {
      pub data: *const u8,
      pub length: u32,
    }
  }

  pub mod inspector {
    //! Stub — QuickJS has no CDP inspector. The QuickJS backend ships with
    //! the inspector disabled; debugger features are not available.
    pub struct V8Inspector;
    pub struct V8InspectorClient;
    pub struct V8InspectorClientBase;
    pub struct V8InspectorSession;
    pub struct ChannelBase;
    pub struct Channel;
    pub struct StringView<'s>(core::marker::PhantomData<&'s ()>);
    pub struct StringBuffer;

    impl V8Inspector {
      pub fn create<C, P>(_isolate: P, _client: C) -> V8Inspector {
        V8Inspector
      }
      pub fn context_created(
        &self,
        _context: crate::value::Local<'_, crate::context::Context>,
        _context_group_id: i32,
        _human_readable_name: StringView<'_>,
        _aux_data: StringView<'_>,
      ) {
      }
      pub fn connect<C>(
        &self,
        _context_group_id: i32,
        _channel: C,
        _state: StringView<'_>,
        _trust: V8InspectorClientTrustLevel,
      ) -> V8InspectorSession {
        V8InspectorSession
      }
      pub fn context_destroyed<C>(&self, _context: C) {}
      pub fn create_stack_trace(
        &self,
        _stack_trace: Option<crate::value::Local<'_, crate::value::StackTrace>>,
      ) -> std::boxed::Box<V8StackTrace> {
        std::boxed::Box::new(V8StackTrace)
      }
      pub fn exception_thrown(
        &self,
        _context: crate::value::Local<'_, crate::context::Context>,
        _message: StringView<'_>,
        _exception: crate::value::Local<'_, crate::value::Value>,
        _detailed_message: StringView<'_>,
        _url: StringView<'_>,
        _line_number: u32,
        _column_number: u32,
        _stack_trace: std::boxed::Box<V8StackTrace>,
        _script_id: i32,
      ) -> u32 {
        0
      }
    }
    pub struct V8StackTrace;
    impl V8InspectorClient {
      pub fn new<C>(_client: C) -> Self {
        Self
      }
    }
    impl V8InspectorSession {
      pub fn dispatch_protocol_message(&self, _message: StringView<'_>) {}
      pub fn schedule_pause_on_next_statement(
        &self,
        _reason: StringView<'_>,
        _details: StringView<'_>,
      ) {
      }
    }

    /// Helper trait some deno_core code uses to call `.context_created`
    /// on `Rc<Rc<V8Inspector>>`. Mirrors the rusty_v8 ergonomic.
    pub trait V8InspectorContextCreated {
      fn context_created(
        &self,
        _isolate: &mut crate::isolate::Isolate,
        _context_group_id: i32,
        _human_readable_name: StringView<'_>,
        _aux_data: StringView<'_>,
      );
    }
    impl V8InspectorContextCreated for std::rc::Rc<std::rc::Rc<V8Inspector>> {
      fn context_created(
        &self,
        _isolate: &mut crate::isolate::Isolate,
        _context_group_id: i32,
        _human_readable_name: StringView<'_>,
        _aux_data: StringView<'_>,
      ) {
      }
    }
    impl Channel {
      pub fn new<C>(_channel: C) -> Self {
        Self
      }
    }
    impl<'s> StringView<'s> {
      pub fn empty() -> Self {
        Self(core::marker::PhantomData)
      }
      pub fn from(_bytes: &'s [u8]) -> Self {
        Self(core::marker::PhantomData)
      }
    }
    impl<'s> From<&'s [u8]> for StringView<'s> {
      fn from(_b: &'s [u8]) -> Self {
        Self(core::marker::PhantomData)
      }
    }
    impl StringBuffer {
      pub fn create<'s>(_view: StringView<'s>) -> UniquePtr<StringBuffer> {
        UniquePtr::from(std::boxed::Box::new(Self))
      }
      pub fn string(&self) -> StringView<'_> {
        StringView::empty()
      }
    }
    impl<'s> StringView<'s> {
      pub fn to_string(&self) -> std::string::String {
        std::string::String::new()
      }
    }
    use std::boxed::Box;
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub enum V8InspectorClientTrustLevel {
      Untrusted,
      FullyTrusted,
    }

    /// Stub trait for `inspector::V8InspectorClientImpl`. On QuickJS the
    /// inspector path is never taken; deno_core's session bookkeeping
    /// implements this trait but the methods are unreachable at runtime.
    /// The method set mirrors rusty_v8's so deno_core's impl block
    /// matches.
    pub trait V8InspectorClientImpl {
      fn run_message_loop_on_pause(&self, _context_group_id: i32) {}
      fn quit_message_loop_on_pause(&self) {}
      fn run_if_waiting_for_debugger(&self, _context_group_id: i32) {}
      fn ensure_default_context_in_group(
        &self,
        _context_group_id: i32,
      ) -> Option<crate::value::Local<'_, crate::context::Context>> {
        None
      }
      fn resource_name_to_url(
        &self,
        _resource_name: &StringView<'_>,
      ) -> Option<UniquePtr<StringBuffer>> {
        None
      }
    }

    /// Stub trait for `inspector::ChannelImpl`. Same approach as
    /// V8InspectorClientImpl.
    pub trait ChannelImpl {
      fn send_response(
        &self,
        _call_id: i32,
        _message: UniquePtr<StringBuffer>,
      ) {
      }
      fn send_notification(&self, _message: UniquePtr<StringBuffer>) {}
      fn flush_protocol_notifications(&self) {}
    }

    /// Mirror of v8's `UniquePtr<T>`. Just a wrapper around Option<Box<T>>
    /// so deno_core's signatures (which use `v8::UniquePtr<...>`) compile.
    pub struct UniquePtr<T>(Option<std::boxed::Box<T>>);
    impl<T> UniquePtr<T> {
      pub fn from(value: std::boxed::Box<T>) -> Self {
        Self(Some(value))
      }
      pub fn into_raw(self) -> *mut T {
        match self.0 {
          Some(b) => std::boxed::Box::into_raw(b),
          None => core::ptr::null_mut(),
        }
      }
      pub fn unwrap(self) -> std::boxed::Box<T> {
        self.0.expect("UniquePtr unwrap on null")
      }
    }
  }

  pub mod icu {
    //! Stub — ICU is not bundled with QuickJS. Locale-sensitive ops fall
    //! back to QuickJS's built-in Intl shim.
    pub fn set_common_data_71(_data: &[u8]) -> Result<(), ()> {
      Ok(())
    }
    pub fn set_common_data_73(_data: &[u8]) -> Result<(), ()> {
      Ok(())
    }
    pub fn set_common_data_77(_data: &[u8]) -> Result<(), ()> {
      Ok(())
    }
  }

  pub mod json {
    //! `JSON.stringify` / `JSON.parse` exposed through QuickJS.
    use crate::primitives::String;
    use crate::scope::HandleScope;
    use crate::value::Local;
    use crate::value::Value;

    pub fn stringify<'s>(
      _scope: &mut HandleScope<'s>,
      _value: Local<'s, Value>,
    ) -> Option<Local<'s, String>> {
      None
    }
    pub fn parse<'s>(
      _scope: &mut HandleScope<'s>,
      _source: Local<'s, String>,
    ) -> Option<Local<'s, Value>> {
      None
    }
  }

  pub mod script_compiler {
    //! Compiled-script primitives. QuickJS supports JS_WriteObject /
    //! JS_ReadObject for bytecode persistence — that maps to V8's
    //! CachedData. Module compilation goes through JS_Eval with
    //! JS_EVAL_TYPE_MODULE | JS_EVAL_FLAG_COMPILE_ONLY.
    pub use crate::external::CachedData;
    use crate::function::Function;
    use crate::module::Module;
    use crate::scope::HandleScope;
    use crate::script::Script;
    use crate::value::Local;

    pub struct Source;
    impl Source {
      pub fn new<'s>(
        _source_string: crate::value::Local<'s, crate::primitives::String>,
        _origin: Option<&crate::script::ScriptOrigin<'s>>,
      ) -> Self {
        Self
      }
      pub fn new_with_cached_data<'s>(
        _source_string: crate::value::Local<'s, crate::primitives::String>,
        _origin: Option<&crate::script::ScriptOrigin<'s>>,
        _cached_data: CachedData,
      ) -> Self {
        Self
      }
      pub fn get_cached_data(&self) -> Option<&CachedData> {
        None
      }
    }
    impl CachedData {
      pub fn new(_data: &[u8]) -> Self {
        Self(Vec::new())
      }
    }

    pub enum CompileOptions {
      NoCompileOptions,
      ConsumeCodeCache,
      EagerCompile,
    }

    /// Stub for `script_compiler::compile`. Real eval flows go through
    /// `JS_Eval` directly; this entry point exists to satisfy
    /// generic-snapshot code that pre-compiles via the script_compiler
    /// API on V8. Returns `None` on QuickJS.
    pub fn compile<'s, S, O, N>(
      _scope: &mut S,
      _source: &mut Source,
      _options: O,
      _no_cache_reason: N,
    ) -> Option<Local<'s, Script>> {
      None
    }
    pub fn compile_module<'s>(
      _scope: &mut HandleScope<'s>,
      _source: &mut Source,
    ) -> Option<Local<'s, Module>> {
      None
    }
    pub fn compile_function<'s, S, O, N>(
      _scope: &mut S,
      _source: &mut Source,
      _arguments: &[Local<'s, super::String>],
      _context_extensions: &[Local<'s, super::Object>],
      _options: O,
      _no_cache_reason: N,
    ) -> Option<Local<'s, Function>> {
      None
    }
    pub fn compile_module2<'s, S, O, N>(
      _scope: &mut S,
      _source: &mut Source,
      _options: O,
      _no_cache_reason: N,
    ) -> Option<Local<'s, Module>> {
      None
    }
    pub enum NoCacheReason {
      NoReason,
      BecauseCachingDisabled,
      BecauseNoResource,
      BecauseInlineScript,
      BecauseModule,
      BecauseStreamingSource,
      BecauseInspector,
      BecauseScriptTooSmall,
      BecauseCacheTooCold,
      BecauseV8Extension,
      BecauseExtensionModule,
      BecausePacScript,
      BecauseInDocumentWrite,
      BecauseResourceWithNoCacheHandler,
      BecauseDeferredProduceCodeCache,
    }
  }

  // Free functions in the v8 namespace.
  pub fn new_default_platform(
    _thread_pool_size: u32,
    _idle_task_support: bool,
  ) -> std::rc::Rc<()> {
    std::rc::Rc::new(())
  }
  pub fn new_unprotected_default_platform(
    _thread_pool_size: u32,
    _idle_task_support: bool,
  ) -> std::rc::Rc<()> {
    std::rc::Rc::new(())
  }

  pub fn undefined<'s, 'i>(
    _scope: &mut crate::scope::PinScope<'s, 'i>,
  ) -> crate::value::Local<'s, crate::value::Primitive> {
    crate::value::Local::from_raw(crate::sys::jsv_undefined())
  }
  pub fn null<'s, 'i>(
    _scope: &mut crate::scope::PinScope<'s, 'i>,
  ) -> crate::value::Local<'s, crate::value::Primitive> {
    crate::value::Local::from_raw(crate::sys::jsv_null())
  }

  pub struct V8;
  impl V8 {
    pub fn initialize_platform<P>(_p: P) {}
    pub fn initialize() {}
    pub fn dispose() -> bool {
      true
    }
    pub fn dispose_platform() {}
    pub fn set_flags_from_string(_s: &str) {}
    pub fn set_flags_from_command_line<S>(args: Vec<S>) -> Vec<S> {
      args
    }
  }

  /// `WriteFlags` — string write-flag bitset. Mirrors rusty_v8's
  /// associated-constant set.
  #[derive(Copy, Clone, Eq, PartialEq, Debug)]
  pub struct WriteFlags(pub u32);
  impl WriteFlags {
    pub const NONE: Self = Self(0);
    pub const HINT_MANY_WRITES_EXPECTED: Self = Self(1);
    pub const NO_NULL_TERMINATION: Self = Self(2);
    pub const PRESERVE_ONE_BYTE_NULL: Self = Self(4);
    pub const REPLACE_INVALID_UTF8: Self = Self(8);
    #[allow(non_upper_case_globals)]
    pub const kReplaceInvalidUtf8: Self = Self::REPLACE_INVALID_UTF8;
    pub const fn empty() -> Self {
      Self(0)
    }
  }
  impl core::ops::BitOr for WriteFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
      Self(self.0 | rhs.0)
    }
  }

  /// Stub for `v8::latin1_to_utf8`. The real API converts a latin1
  /// buffer to UTF-8 in-place; we never call this on QuickJS, so the
  /// fn-pointer existence is what matters.
  pub fn latin1_to_utf8(_input_len: usize, _input: *const u8, _output: *mut u8) -> usize {
    0
  }

  /// Stub for `v8::simdutf` — rusty_v8 has it as a sub-namespace for
  /// SIMD UTF-8 helpers. Inert under QuickJS.
  pub mod simdutf {
    pub fn validate_utf8(input: &[u8]) -> bool {
      core::str::from_utf8(input).is_ok()
    }
    pub fn validate_ascii(input: &[u8]) -> bool {
      input.is_ascii()
    }
    pub fn utf8_length_from_utf16(_input: &[u16]) -> usize {
      0
    }
  }

  /// Mirrors rusty_v8's `data` submodule that re-organizes the typed
  /// array zoo + a few base types. deno_core uses paths like
  /// `v8::data::Uint8Array` and `v8::data::Boolean`.
  pub mod data {
    pub use super::ArrayBufferView;
    pub use super::BigInt64Array;
    pub use super::BigUint64Array;
    pub use super::DataView;
    pub use super::Float32Array;
    pub use super::Float64Array;
    pub use super::Int8Array;
    pub use super::Int16Array;
    pub use super::Int32;
    pub use super::Int32Array;
    pub use super::Uint8ClampedArray;
    pub use super::Uint16Array;
    pub use super::Uint32;
    pub use super::Uint32Array;
    pub use crate::buffer::Uint8Array;
    pub use crate::primitives::Boolean;
    pub use crate::primitives::Integer;
    pub use crate::primitives::Number;
    pub use crate::value::Value;
  }

  /// Stub trait for `v8::PlatformImpl`. deno_core implements this on
  /// its custom platform type; under QuickJS the platform abstraction
  /// is unused but the trait must accept the method set deno_core
  /// declares.
  pub trait PlatformImpl {
    fn post_task(&self, _isolate_ptr: *mut core::ffi::c_void, _task: Task) {}
    fn post_non_nestable_task(
      &self,
      _isolate_ptr: *mut core::ffi::c_void,
      _task: Task,
    ) {
    }
    fn post_delayed_task(
      &self,
      _isolate_ptr: *mut core::ffi::c_void,
      _task: Task,
      _delay_in_seconds: f64,
    ) {
    }
    fn post_non_nestable_delayed_task(
      &self,
      _isolate_ptr: *mut core::ffi::c_void,
      _task: Task,
      _delay_in_seconds: f64,
    ) {
    }
    fn post_idle_task(
      &self,
      _isolate_ptr: *mut core::ffi::c_void,
      _task: IdleTask,
    ) {
    }
  }

  // Re-export inspector::UniquePtr as v8::UniquePtr so deno_core
  // signatures using `v8::UniquePtr<v8::inspector::StringBuffer>` resolve.
  pub use inspector::UniquePtr;

  // Typed-array stubs. QuickJS-ng has typed arrays under the hood (they're
  // ordinary JSObjects of class TypedArray) but we don't yet expose
  // distinct Local<Int8Array>-style wrappers. The op2 macro and various
  // ext/* crates reference these names by type alone — we mirror the
  // shape so generic code compiles. Runtime use through these types is
  // not yet supported.
  macro_rules! typed_array_stub {
    ($($name:ident),* $(,)?) => { $(
      pub struct $name;
      impl crate::value::ValueType for $name {
        fn is(_raw: &crate::sys::JSValue) -> bool { false }
      }
      impl $name {
        /// Mirror of `v8::TypedArray::new(scope, buf, offset, length)`.
        pub fn new<'s, 'b>(
          scope: &mut crate::scope::HandleScope<'s>,
          _buf: crate::value::Local<'b, crate::buffer::ArrayBuffer>,
          _offset: usize,
          _length: usize,
        ) -> Option<crate::value::Local<'s, $name>> {
          let raw = crate::sys::new_object(scope.ctx());
          scope.track_owned(raw);
          Some(crate::value::Local::from_raw(raw))
        }
      }
      impl<'s> From<crate::value::Local<'s, $name>>
        for crate::value::Local<'s, crate::value::Value>
      {
        fn from(
          v: crate::value::Local<'s, $name>,
        ) -> crate::value::Local<'s, crate::value::Value> {
          crate::value::Local::from_raw(crate::value::Local::raw(&v))
        }
      }
    )* }
  }
  typed_array_stub!(
    Int8Array,
    Uint8ClampedArray,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    BigInt64Array,
    BigUint64Array,
    Float32Array,
    Float64Array,
    DataView,
  );

  // Other oddballs deno_core references by name.
  pub struct Int32;
  impl Int32 {
    pub fn value(&self) -> i32 {
      0
    }
  }
  pub struct Uint32;
  impl Uint32 {
    pub fn value(&self) -> u32 {
      0
    }
  }
  pub struct Task;
  impl Task {
    pub fn run(&mut self) {}
  }
  pub struct IdleTask;
  /// `v8::FunctionBuilder<T>` — used to construct FunctionTemplates and
  /// Functions. The phantom generic is the v8 type the builder produces.
  /// We track the declared `length` so the produced JS function has the
  /// right `.length` property (deno_core's `setUpAsyncStub` reads it
  /// when wiring async ops).
  pub struct FunctionBuilder<T> {
    length: i32,
    _t: core::marker::PhantomData<T>,
  }
  impl<T> FunctionBuilder<T> {
    pub fn new<F>(_callback: F) -> Self
    where
      F: crate::function::MapFnTo<super::FunctionCallback>,
    {
      Self {
        length: 0,
        _t: core::marker::PhantomData,
      }
    }
    pub fn data<'s>(self, _data: super::Local<'s, super::Value>) -> Self {
      self
    }
    pub fn length(mut self, length: i32) -> Self {
      self.length = length;
      self
    }
    pub fn side_effect_type(self, _t: super::SideEffectType) -> Self {
      self
    }
    pub fn build<'s, S: crate::scope::HandleScopeSource>(
      self,
      scope: &mut S,
    ) -> Option<super::Local<'s, T>> {
      let ctx = scope.default_ctx();
      let raw = unsafe {
        crate::ffi::JS_NewCFunction(
          ctx,
          super::function::function_new_trampoline,
          core::ptr::null(),
          self.length,
        )
      };
      Some(super::Local::from_raw(raw))
    }
    pub fn build_fast<'s, S: crate::scope::HandleScopeSource, F>(
      self,
      scope: &mut S,
      _fast_function: F,
    ) -> Option<super::Local<'s, T>> {
      self.build(scope)
    }
    pub fn constructor_behavior(
      self,
      _b: crate::function::ConstructorBehavior,
    ) -> Self {
      self
    }
  }
  pub type NearHeapLimitCallback = unsafe extern "C" fn(
    data: *mut core::ffi::c_void,
    current_heap_limit: usize,
    initial_heap_limit: usize,
  ) -> usize;

  /// Stub for `v8::new_custom_platform` — used for snapshot/test
  /// platforms. QuickJS has no platform abstraction; returns a unit Rc.
  /// Generic over extra args for compatibility with rusty_v8 variants
  /// that take additional arguments (worker tasks, platform impl, etc).
  pub fn new_custom_platform<A, B>(
    _thread_pool_size: u32,
    _idle_task_support: bool,
    _a: A,
    _b: B,
  ) -> Platform {
    Platform
  }

  /// Wrapper around `()` so we can hang `.make_shared()` and similar
  /// builder methods used by deno_core's platform setup. Mirrors the
  /// rusty_v8 call chain `new_custom_platform(...).make_shared()`.
  pub struct Platform;
  impl Platform {
    pub fn make_shared(self) -> std::sync::Arc<Self> {
      std::sync::Arc::new(self)
    }
  }
}

pub use arena::Arena;
pub use arena::ArenaStats;
pub use arena::MockJSValue;
pub use arena::MockTag;
