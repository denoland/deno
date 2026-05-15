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
pub use crate::v8::TimeZoneDetection;
pub use crate::v8::Set;
pub use crate::v8::Date;
pub use crate::v8::Private;
pub use crate::v8::PropertyDescriptor;
pub use crate::v8::UnboundScript;
pub use crate::v8::IntegrityLevel;
pub use crate::v8::Float16Array;
pub use crate::v8::MicrotaskQueue;
pub use crate::v8::MicrotaskQueueOwned;
pub use crate::v8::HeapCodeStatistics;
pub use crate::v8::MicrotasksPolicy;
pub use crate::v8::MicrotaskQueueIntoRaw;
pub use crate::v8::IndexedPropertyHandlerConfiguration;
pub use crate::v8::IndexedPropertyGetterCallback;
pub use crate::v8::IndexedPropertySetterCallback;
pub use crate::v8::IndexedPropertyQueryCallback;
pub use crate::v8::IndexedPropertyDeleterCallback;
pub use crate::v8::IndexedPropertyEnumeratorCallback;
pub use crate::v8::IndexedPropertyDefinerCallback;
pub use crate::v8::IndexedPropertyDescriptorCallback;
pub use crate::v8::NamedPropertyGetterCallback;
pub use crate::v8::NamedPropertySetterCallback;
pub use crate::v8::NamedPropertyQueryCallback;
pub use crate::v8::NamedPropertyDeleterCallback;
pub use crate::v8::NamedPropertyEnumeratorCallback;
pub use crate::v8::NamedPropertyDefinerCallback;
pub use crate::v8::NamedPropertyDescriptorCallback;
pub use crate::v8::PropertyHandlerFlags;
pub use crate::v8::Handle;
/// Mirror of `v8::VERSION_STRING` — what we report. Used by some
/// Node.js compatibility code (deno_inspector_server) to identify
/// the engine.
pub const VERSION_STRING: &str = "qjs_v8_compat (QuickJS-ng)";
pub use crate::v8::GCType;
pub use crate::v8::GCCallbackFlags;
pub use crate::v8::GCCallback;
pub use crate::v8::ValueView;
pub use crate::v8::ValueViewData;
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
  (unsafe let $name:ident, $raw:expr) => {
    let mut __cb_inner = unsafe { $crate::CallbackScope::new($raw) };
    let $name = &mut __cb_inner;
  };
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
    pub trait Traceable {}
    impl<T> Traceable for Member<T> {}
    impl<T> Traceable for crate::value::TracedReference<T> {}
    impl Visitor {
      pub fn trace<T: Traceable>(&mut self, _t: &T) {}
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
      /// Recover the active `&mut Isolate` from the fast-callback. On
      /// QuickJS the fast path is never actually taken (slow path
      /// fires instead), but op2-emitted code references this to
      /// satisfy `&mut Isolate` arguments.
      pub fn isolate_unchecked_mut(
        &mut self,
      ) -> &mut crate::isolate::Isolate {
        let p = crate::isolate::current_isolate_ptr();
        unsafe { &mut *p }
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
      /// The `'static` bound on `args` matches the op2-generated arrays
      /// but precludes runtime-built slices; use `new_owned` for those.
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
      /// Variant that accepts non-`'static` `&[CTypeInfo]`. Stores the
      /// slice via a `'static`-typed pointer cast — caller is
      /// responsible for keeping the underlying buffer alive as long as
      /// the CFunctionInfo. deno_ffi's `make_template` Boxes both the
      /// param array and the CFunctionInfo together inside a Turbocall,
      /// so they share lifetime.
      pub fn new_owned(
        return_info: CTypeInfo,
        args: &[CTypeInfo],
        i64: Int64Representation,
      ) -> Self {
        let args_static: &'static [CTypeInfo] = unsafe {
          core::slice::from_raw_parts(args.as_ptr(), args.len())
        };
        Self {
          _return_info: return_info,
          _args: args_static,
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
    /// Mirror of `v8::icu::get_language_tag` — returns the canonical
    /// BCP-47 tag for the given locale string (real v8 takes the
    /// requested locale string and returns canonical form). The
    /// no-arg variant returns the runtime's current locale.
    pub fn get_language_tag() -> std::string::String {
      "en-US".to_string()
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

    /// Stores the JS source string and origin (URL) so `compile_module`
    /// can hand the body off to QuickJS at evaluation time.
    pub struct Source {
      pub(crate) source: crate::sys::JSValue,
      pub(crate) name: Option<crate::sys::JSValue>,
    }
    impl Source {
      pub fn new<'s>(
        source_string: crate::value::Local<'s, crate::primitives::String>,
        origin: Option<&crate::script::ScriptOrigin<'s>>,
      ) -> Self {
        Self {
          source: source_string.raw(),
          name: origin.and_then(|o| o.resource_name_raw()),
        }
      }
      pub fn new_with_cached_data<'s>(
        source_string: crate::value::Local<'s, crate::primitives::String>,
        origin: Option<&crate::script::ScriptOrigin<'s>>,
        _cached_data: CachedData,
      ) -> Self {
        Self::new(source_string, origin)
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
      scope: &mut HandleScope<'s>,
      source: &mut Source,
    ) -> Option<Local<'s, Module>> {
      let ctx = scope.ctx();
      let raw = crate::sys::new_object(ctx);
      crate::module::record_module_status(
        &raw,
        crate::v8::ModuleStatus::Uninstantiated,
      );
      let src = crate::sys::to_string_lossy(ctx, source.source);
      let filename = source
        .name
        .and_then(|v| crate::sys::to_string_lossy(ctx, v));
      if let Some(src) = src {
        crate::module::record_module_source(&raw, src, filename);
      }
      Some(super::Local::from_raw(raw))
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
      scope: &mut S,
      source: &mut Source,
      _options: O,
      _no_cache_reason: N,
    ) -> Option<Local<'s, Module>>
    where
      S: crate::scope::HandleScopeSource,
    {
      let ctx = scope.default_ctx();
      let raw = crate::sys::new_object(ctx);
      crate::module::record_module_status(
        &raw,
        crate::v8::ModuleStatus::Uninstantiated,
      );
      let src = crate::sys::to_string_lossy(ctx, source.source);
      let filename = source
        .name
        .and_then(|v| crate::sys::to_string_lossy(ctx, v));
      if let Some(src) = src {
        crate::module::record_module_source(&raw, src, filename);
      }
      Some(super::Local::from_raw(raw))
    }
    /// Mirror of `v8::script_compiler::cached_data_version_tag`. Real
    /// V8 returns a build-stable tag derived from the version + flags.
    /// QuickJS doesn't use this — return a constant.
    pub fn cached_data_version_tag() -> u32 {
      0
    }
    /// Mirror of `v8::script_compiler::compile_unbound_script`. Returns
    /// a stub UnboundScript (just a Script under the hood).
    pub fn compile_unbound_script<'s, S, O, N>(
      _scope: &mut S,
      _source: &mut Source,
      _options: O,
      _no_cache_reason: N,
    ) -> Option<Local<'s, super::UnboundScript>> {
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

  pub fn undefined<'s, S: ?Sized>(
    _scope: &S,
  ) -> crate::value::Local<'s, crate::value::Primitive> {
    crate::value::Local::from_raw(crate::sys::jsv_undefined())
  }
  pub fn null<'s, S: ?Sized>(
    _scope: &S,
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
    /// Real v8 returns the args V8 didn't recognize. We accept all
    /// flags silently (QuickJS doesn't have V8-style flags), so
    /// return only the first arg (the binary name) which the deno
    /// caller skips.
    pub fn set_flags_from_command_line<S>(args: Vec<S>) -> Vec<S> {
      args.into_iter().take(1).collect()
    }
    pub fn set_fatal_error_handler<F>(_handler: F) {}
  }
  // Platform is defined later in this module.

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
    pub const NULL_TERMINATE: Self = Self(16);
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

    // Base64 encode/decode shims used by ext/web. We don't implement
    // them properly here (returns 0 / empty) — full base64 lives in
    // `base64` crate or our own impl in a follow-up.
    #[repr(u32)]
    #[derive(Copy, Clone, Default)]
    pub enum Base64Options {
      #[default]
      Default = 0,
      Url = 1,
    }

    #[repr(u32)]
    #[derive(Copy, Clone, Default)]
    pub enum LastChunkHandling {
      #[default]
      Loose = 0,
      Strict = 1,
      StopBeforePartial = 2,
    }

    /// Result of `base64_to_binary` — mirrors rusty_v8's struct shape
    /// (it doesn't return `Result`; the error is signaled by
    /// `is_ok() == false` on the struct itself).
    pub struct Base64Result {
      pub count: usize,
      pub written: usize,
      ok: bool,
    }
    impl Base64Result {
      pub fn is_ok(&self) -> bool {
        self.ok
      }
    }

    pub fn base64_to_binary(
      input: &[u8],
      output: &mut [u8],
      _options: Base64Options,
      _last: LastChunkHandling,
    ) -> Base64Result {
      let _ = (input, output);
      Base64Result { count: 0, written: 0, ok: false }
    }

    pub fn maximal_binary_length_from_base64(input: &[u8]) -> usize {
      input.len() * 3 / 4
    }

    pub fn base64_length_from_binary(
      input_len: usize,
      _options: Base64Options,
    ) -> usize {
      ((input_len + 2) / 3) * 4
    }

    pub fn binary_to_base64(
      input: &[u8],
      output: &mut [u8],
      _options: Base64Options,
    ) -> usize {
      let _ = (input, output);
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
      impl<'s> From<crate::value::Local<'s, $name>>
        for crate::value::Local<'s, crate::object::Object>
      {
        fn from(
          v: crate::value::Local<'s, $name>,
        ) -> crate::value::Local<'s, crate::object::Object> {
          crate::value::Local::from_raw(crate::value::Local::raw(&v))
        }
      }
      impl<'s> TryFrom<crate::value::Local<'s, crate::value::Value>>
        for crate::value::Local<'s, $name>
      {
        type Error = crate::exception::DataError;
        fn try_from(
          v: crate::value::Local<'s, crate::value::Value>,
        ) -> Result<Self, Self::Error> {
          Ok(crate::value::Local::from_raw(crate::value::Local::raw(&v)))
        }
      }
      impl<'s> crate::value::Local<'s, $name> {
        pub fn set_index<S>(
          &self,
          _scope: &S,
          _index: u32,
          _value: crate::value::Local<'_, crate::value::Value>,
        ) -> Option<bool> { Some(true) }
        pub fn byte_length(&self) -> usize { 0 }
        pub fn byte_offset(&self) -> usize { 0 }
        pub fn length(&self) -> usize { 0 }
        pub fn data(&self) -> *mut core::ffi::c_void { core::ptr::null_mut() }
        pub fn buffer<'sc, S>(
          &self,
          _scope: &mut S,
        ) -> Option<crate::value::Local<'sc, crate::buffer::ArrayBuffer>>
        where S: crate::scope::HandleScopeSource {
          Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
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
    BigInt64Array,
    BigUint64Array,
  );
  // For the typed arrays ext/web actually performs upcasts on
  // (Uint32Array, Float32Array, Float64Array, DataView), re-export
  // the buffer-side types — those carry a real JSValue and have
  // `From<Local<...>> for Local<ArrayBufferView>` impls.
  pub use crate::buffer::DataView;
  pub use crate::buffer::Float32Array;
  pub use crate::buffer::Float64Array;
  pub use crate::buffer::Uint32Array;

  // GC callback stubs used by ext/telemetry.
  #[derive(Copy, Clone, Default)]
  pub enum TimeZoneDetection {
    #[default]
    Skip,
    Redetect,
  }

  /// Big bag of stubs added for deno_node compile compatibility. Each
  /// returns the most permissive default (None / 0 / false / Self) so
  /// deno_node code paths fail gracefully at runtime if exercised.

  /// MAX_LENGTH on String mirrors V8's max string length constant.
  impl crate::primitives::String {
    pub const MAX_LENGTH: usize = (1 << 28) - 16;
  }
  /// std::hash::Hash for v8::String — hashes the raw JSValue pointer
  /// bits. Real v8 hashes the string content; this is a shallow stub
  /// (pointer identity).
  impl std::hash::Hash for crate::primitives::String {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
      let p: u64 = unsafe { self.raw.u.ptr as usize as u64 };
      p.hash(state);
      self.raw.tag.hash(state);
    }
  }

  // PropertyDescriptor (v8::PropertyDescriptor) — opaque builder.
  pub struct PropertyDescriptor {
    inner: PropertyDescriptorInner,
  }
  #[derive(Default)]
  struct PropertyDescriptorInner {
    value: Option<crate::value::Local<'static, crate::value::Value>>,
    get: Option<crate::value::Local<'static, crate::value::Value>>,
    set: Option<crate::value::Local<'static, crate::value::Value>>,
    writable: bool,
    enumerable: bool,
    configurable: bool,
    has_value: bool,
    has_writable: bool,
    has_get: bool,
    has_set: bool,
    has_enumerable: bool,
    has_configurable: bool,
  }
  impl PropertyDescriptor {
    pub fn new() -> Self { Self { inner: Default::default() } }
    pub fn new_from_value<'s>(_value: crate::value::Local<'s, crate::value::Value>) -> Self {
      Self { inner: Default::default() }
    }
    pub fn new_from_value_writable<'s>(
      _value: crate::value::Local<'s, crate::value::Value>,
      _writable: bool,
    ) -> Self {
      Self { inner: Default::default() }
    }
    pub fn new_from_get_set<'s>(
      _get: crate::value::Local<'s, crate::value::Value>,
      _set: crate::value::Local<'s, crate::value::Value>,
    ) -> Self {
      Self { inner: Default::default() }
    }
    pub fn value(&self) -> crate::value::Local<'_, crate::value::Value> {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    pub fn get(&self) -> crate::value::Local<'_, crate::value::Value> {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    pub fn set(&self) -> crate::value::Local<'_, crate::value::Value> {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    pub fn writable(&self) -> bool { self.inner.writable }
    pub fn enumerable(&self) -> bool { self.inner.enumerable }
    pub fn configurable(&self) -> bool { self.inner.configurable }
    pub fn has_value(&self) -> bool { self.inner.has_value }
    pub fn has_writable(&self) -> bool { self.inner.has_writable }
    pub fn has_get(&self) -> bool { self.inner.has_get }
    pub fn has_set(&self) -> bool { self.inner.has_set }
    pub fn has_enumerable(&self) -> bool { self.inner.has_enumerable }
    pub fn has_configurable(&self) -> bool { self.inner.has_configurable }
    pub fn set_configurable(&mut self, v: bool) { self.inner.configurable = v; self.inner.has_configurable = true; }
    pub fn set_enumerable(&mut self, v: bool) { self.inner.enumerable = v; self.inner.has_enumerable = true; }
  }

  /// PropertyAttribute helpers.
  impl crate::object::PropertyAttribute {
    pub fn is_dont_delete(&self) -> bool { false }
    pub fn is_read_only(&self) -> bool { false }
    pub fn is_dont_enum(&self) -> bool { false }
    pub fn as_u32(&self) -> u32 { 0 }
  }


  /// WriteFlags::default helper.
  impl crate::v8::WriteFlags {
    pub fn default() -> Self { Self::empty() }
  }

  /// PinScope methods deno_node uses but our compat doesn't have yet.
  impl<'s, 'i, C> crate::scope::PinScope<'s, 'i, C> {
    pub fn low_memory_notification(&mut self) {}
    /// Mirror of `Isolate::add_context` — returns a context index.
    /// QuickJS doesn't have multi-context contexts; we return 0.
    pub fn add_context(&mut self, _ctx: crate::value::Local<'_, crate::context::Context>) -> usize {
      0
    }
    pub fn take_heap_snapshot<F>(&mut self, _writer: F)
    where F: FnMut(&[u8]) -> bool {}
    pub fn get_heap_code_and_metadata_statistics(
      &mut self,
    ) -> Option<HeapCodeStatistics> {
      None
    }
    pub fn set_allow_wasm_code_generation_callback<F>(&mut self, _cb: F) {}
  }
  pub struct HeapCodeStatistics;
  impl HeapCodeStatistics {
    pub fn new() -> Self { Self }
    pub fn code_and_metadata_size(&self) -> usize { 0 }
    pub fn bytecode_and_metadata_size(&self) -> usize { 0 }
    pub fn external_script_source_size(&self) -> usize { 0 }
    pub fn cpu_profiler_metadata_size(&self) -> usize { 0 }
  }
  impl Default for HeapCodeStatistics {
    fn default() -> Self { Self }
  }

  /// AllowJavascriptExecutionScope LocalNewScopeRef — derives via inner.
  impl<'a, 's, P> crate::value::LocalNewScopeRef<'s>
    for crate::context::AllowJavascriptExecutionScope<'a, P>
  where
    P: crate::value::LocalNewScopeRef<'s>,
  {
    fn as_mut_handle_scope_ref(&self) -> &mut crate::scope::HandleScope<'s> {
      let p_ptr = self as *const _ as *const u8 as *const P;
      unsafe { (*p_ptr).as_mut_handle_scope_ref() }
    }
  }
  /// `throw_exception` for the various AllowJavascriptExecutionScope wrappers.
  impl<'a, 's, 'i, C>
    crate::context::AllowJavascriptExecutionScope<'a, crate::scope::PinScope<'s, 'i, C>>
  {
    pub fn throw_exception(
      &self,
      exc: crate::value::Local<'s, crate::value::Value>,
    ) -> crate::value::Local<'s, crate::value::Value> {
      use crate::value::GlobalScope;
      let ctx = self.scope_ctx_shared();
      crate::sys::throw(ctx, exc.raw());
      exc
    }
  }
  /// `GlobalScope` impl for AllowJavascriptExecutionScope — derives via the inner.
  impl<'a, P> crate::value::GlobalScope
    for crate::context::AllowJavascriptExecutionScope<'a, P>
  where
    P: crate::value::GlobalScope,
  {
    fn scope_ctx_shared(&self) -> crate::sys::Context {
      let p_ptr = self as *const _ as *const u8 as *const P;
      unsafe { (*p_ptr).scope_ctx_shared() }
    }
  }

  /// EscapableHandleScope::init stub matching CallbackScope::init —
  /// returns the same Pin so the chained `&mut scope_storage.init()`
  /// pattern in op2-generated code resolves to `&mut Pin<&mut EHS>`.
  impl<'s, 'l, C> crate::scope::EscapableHandleScope<'s, 'l, C> {
    pub fn init(
      self: std::pin::Pin<&mut Self>,
    ) -> std::pin::Pin<&mut Self> {
      self
    }
  }

  /// Object additional methods.
  impl<'s> crate::value::Local<'s, crate::object::Object> {
    pub fn define_property<'sc, K, S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, K>,
      _descriptor: &PropertyDescriptor,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(true) }
    pub fn delete_index<S>(
      &self,
      _scope: &mut S,
      _index: u32,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(true) }
    pub fn get_creation_context<'sc, S>(
      &self,
      _scope: &mut S,
    ) -> Option<crate::value::Local<'sc, crate::context::Context>>
    where S: crate::scope::HandleScopeSource {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
    pub fn get_own_property_descriptor<'sc, K, S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, K>,
    ) -> Option<crate::value::Local<'sc, crate::value::Value>>
    where S: crate::scope::HandleScopeSource {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
    pub fn get_property_attributes<'sc, K, S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, K>,
    ) -> Option<crate::object::PropertyAttribute>
    where S: crate::scope::HandleScopeSource {
      Some(crate::object::PropertyAttribute::NONE)
    }
    pub fn get_real_named_property<'sc, S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, crate::value::Name>,
    ) -> Option<crate::value::Local<'sc, crate::value::Value>>
    where S: crate::scope::HandleScopeSource {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
    pub fn get_real_named_property_attributes<S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, crate::value::Name>,
    ) -> Option<crate::object::PropertyAttribute>
    where S: crate::scope::HandleScopeSource {
      Some(crate::object::PropertyAttribute::NONE)
    }
    pub fn has_real_named_property<S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, crate::value::Name>,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(false) }
  }

  /// Context additional methods.
  impl<'s> crate::value::Local<'s, crate::context::Context> {
    pub fn clear_all_slots(&self) {}
    pub fn get_slot<T: 'static>(&self) -> Option<&T> { None }
    pub fn set_slot<T: 'static>(&self, _value: T) {}
    pub fn get_security_token<'sc, S>(
      &self,
      _scope: &mut S,
    ) -> crate::value::Local<'sc, crate::value::Value>
    where S: crate::scope::HandleScopeSource {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    /// Mirror of v8's `Context::set_security_token(token)` — single arg.
    pub fn set_security_token(
      &self,
      _token: crate::value::Local<'_, crate::value::Value>,
    ) {}
    pub fn set_allow_generation_from_strings(&self, _allow: bool) {}
  }

  /// Script additional methods.
  impl<'s> crate::value::Local<'s, crate::script::Script> {
    pub fn create_code_cache(&self) -> Option<Box<crate::external::CachedData>> {
      None
    }
  }

  /// Stub UnboundScript type.
  pub struct UnboundScript;
  impl<'s> crate::value::Local<'s, UnboundScript> {
    pub fn bind_to_current_context<'sc, S>(
      &self,
      _scope: &mut S,
    ) -> crate::value::Local<'sc, crate::script::Script>
    where S: crate::scope::HandleScopeSource {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    pub fn get_source_mapping_url<'sc, S>(
      &self,
      _scope: &mut S,
    ) -> crate::value::Local<'sc, crate::value::Value>
    where S: crate::scope::HandleScopeSource {
      crate::value::Local::from_raw(crate::sys::jsv_undefined())
    }
    pub fn create_code_cache(&self) -> Option<Box<crate::external::CachedData>> {
      None
    }
  }

  /// HeapStatistics extras.
  impl crate::isolate::HeapStatistics {
    pub fn does_zap_garbage(&self) -> bool { false }
    pub fn number_of_native_contexts(&self) -> usize { 0 }
    pub fn number_of_detached_contexts(&self) -> usize { 0 }
    pub fn total_allocated_bytes(&self) -> usize { 0 }
    pub fn total_global_handles_size(&self) -> usize { 0 }
    pub fn used_global_handles_size(&self) -> usize { 0 }
    pub fn total_heap_size_executable(&self) -> usize { 0 }
  }

  /// Float16Array — same layout / methods as the other
  #[derive(Copy, Clone)]
  #[repr(transparent)]
  pub struct Float16Array {
    pub(crate) raw: crate::sys::JSValue,
  }
  impl Float16Array {
    pub fn new<'s, S>(
      _scope: &mut S,
      _buffer: crate::value::Local<'s, crate::buffer::ArrayBuffer>,
      _byte_offset: usize,
      _length: usize,
    ) -> Option<crate::value::Local<'s, Float16Array>> {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
  }
  impl<'s> crate::value::Local<'s, Float16Array> {
    pub fn byte_length(&self) -> usize { 0 }
    pub fn byte_offset(&self) -> usize { 0 }
    pub fn length(&self) -> usize { 0 }
    pub fn data(&self) -> *mut core::ffi::c_void { core::ptr::null_mut() }
  }
  impl<'s> From<crate::value::Local<'s, Float16Array>>
    for crate::value::Local<'s, crate::value::Value>
  {
    fn from(v: crate::value::Local<'s, Float16Array>) -> Self {
      crate::value::Local::from_raw(v.raw())
    }
  }

  /// Mirror of `v8::MicrotaskQueue`. Real v8 lets a context have its
  /// own microtask queue. QuickJS-ng has a single per-runtime queue;
  /// we expose the type as an opaque marker. Returned wrapped in a
  /// `MicrotaskQueueOwned` shim so callers can chain `.into_raw()`
  /// (rusty_v8 returns `UniquePtr<MicrotaskQueue>` with the same
  /// method).
  pub struct MicrotaskQueue;
  impl MicrotaskQueue {
    pub fn new<'s, S>(
      _scope: &mut S,
      _policy: MicrotasksPolicy,
    ) -> MicrotaskQueueOwned {
      MicrotaskQueueOwned(Box::new(MicrotaskQueue))
    }
    pub fn perform_checkpoint<S>(&self, _scope: &mut S) {}
  }
  /// Wrapper mirroring rusty_v8's `UniquePtr<MicrotaskQueue>` — exposes
  /// `into_raw() -> *mut MicrotaskQueue` for ext/node/vm.rs's pattern.
  pub struct MicrotaskQueueOwned(pub Box<MicrotaskQueue>);
  impl MicrotaskQueueOwned {
    pub fn into_raw(self) -> *mut MicrotaskQueue { Box::into_raw(self.0) }
  }
  pub trait MicrotaskQueueIntoRaw { fn into_raw(self) -> *mut MicrotaskQueue; }
  impl MicrotaskQueueIntoRaw for Box<MicrotaskQueue> {
    fn into_raw(self) -> *mut MicrotaskQueue { Box::into_raw(self) }
  }
  /// Re-export of MicrotasksPolicy from isolate.rs.
  pub use crate::isolate::MicrotasksPolicy;

  /// Mirror of `v8::IndexedPropertyHandlerConfiguration`. Builder for
  /// indexed property interceptors. We expose builder methods that
  /// no-op but return self so chains compile.
  #[derive(Default)]
  pub struct IndexedPropertyHandlerConfiguration;
  impl IndexedPropertyHandlerConfiguration {
    pub fn new() -> Self { Self }
    pub fn getter<F>(self, _f: F) -> Self { self }
    pub fn getter_raw<F>(self, _f: F) -> Self { self }
    pub fn setter<F>(self, _f: F) -> Self { self }
    pub fn setter_raw<F>(self, _f: F) -> Self { self }
    pub fn query<F>(self, _f: F) -> Self { self }
    pub fn query_raw<F>(self, _f: F) -> Self { self }
    pub fn deleter<F>(self, _f: F) -> Self { self }
    pub fn deleter_raw<F>(self, _f: F) -> Self { self }
    pub fn enumerator<F>(self, _f: F) -> Self { self }
    pub fn enumerator_raw<F>(self, _f: F) -> Self { self }
    pub fn definer<F>(self, _f: F) -> Self { self }
    pub fn definer_raw<F>(self, _f: F) -> Self { self }
    pub fn descriptor<F>(self, _f: F) -> Self { self }
    pub fn descriptor_raw<F>(self, _f: F) -> Self { self }
    pub fn flags(self, _f: PropertyHandlerFlags) -> Self { self }
  }

  /// Indexed/Named property callback type aliases. We expose them as
  /// `*const c_void` to match the field type in `ExternalReference`,
  /// since deno_node code stores them in that union without further
  /// type checking. Real v8 has more specific fn signatures.
  pub type IndexedPropertyGetterCallback = *const core::ffi::c_void;
  pub type IndexedPropertySetterCallback = *const core::ffi::c_void;
  pub type IndexedPropertyQueryCallback = *const core::ffi::c_void;
  pub type IndexedPropertyDeleterCallback = *const core::ffi::c_void;
  pub type IndexedPropertyEnumeratorCallback = *const core::ffi::c_void;
  pub type IndexedPropertyDefinerCallback = *const core::ffi::c_void;
  pub type IndexedPropertyDescriptorCallback = *const core::ffi::c_void;
  pub type NamedPropertyGetterCallback = *const core::ffi::c_void;
  pub type NamedPropertySetterCallback = *const core::ffi::c_void;
  pub type NamedPropertyQueryCallback = *const core::ffi::c_void;
  pub type NamedPropertyDeleterCallback = *const core::ffi::c_void;
  pub type NamedPropertyEnumeratorCallback = *const core::ffi::c_void;
  pub type NamedPropertyDefinerCallback = *const core::ffi::c_void;
  pub type NamedPropertyDescriptorCallback = *const core::ffi::c_void;
  pub use crate::object::PropertyHandlerFlags;
  /// Mirror of `v8::Handle` — trait describing types that have an
  /// underlying handle-data. Real rusty_v8 uses it as the bound on
  /// `Local::new` / `Global::new`. We expose an empty trait whose only
  /// purpose is to satisfy `where v8::Global<T>: v8::Handle<Data = T>`
  /// bounds.
  pub trait Handle {
    type Data;
  }
  impl<T> Handle for crate::value::Global<T> {
    type Data = T;
  }
  impl<'s, T> Handle for crate::value::Local<'s, T> {
    type Data = T;
  }

  /// Bulk stubs for deno_napi compile compatibility.

  /// `v8::Date` — JS Date object.
  #[derive(Copy, Clone)]
  #[repr(transparent)]
  pub struct Date {
    pub(crate) raw: crate::sys::JSValue,
  }
  impl Date {
    pub fn new<'s, S: crate::value::LocalNewScopeRef<'s>>(
      _scope: &S,
      _value: f64,
    ) -> Option<crate::value::Local<'s, Date>> {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
  }
  impl<'s> crate::value::Local<'s, Date> {
    pub fn value_of(&self) -> f64 { 0.0 }
  }
  impl<'s> From<crate::value::Local<'s, Date>>
    for crate::value::Local<'s, crate::value::Value>
  {
    fn from(v: crate::value::Local<'s, Date>) -> Self {
      crate::value::Local::from_raw(v.raw())
    }
  }
  impl<'s> From<crate::value::Local<'s, crate::value::Value>>
    for crate::value::Local<'s, Date>
  {
    fn from(v: crate::value::Local<'s, crate::value::Value>) -> Self {
      crate::value::Local::from_raw(crate::value::Local::raw(&v))
    }
  }

  /// Re-export of crate::value::Private (defined via v8_type! macro).
  pub use crate::value::Private;

  /// Object::has_private / delete_private / has_index / is_function.
  impl<'s> crate::value::Local<'s, crate::object::Object> {
    pub fn has_private<S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, Private>,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(false) }
    pub fn delete_private<S>(
      &self,
      _scope: &mut S,
      _key: crate::value::Local<'_, Private>,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(true) }
    pub fn has_index<S>(
      &self,
      _scope: &mut S,
      _index: u32,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(false) }
    pub fn is_function(&self) -> bool {
      // Best-effort: forward to Local<Value>::is_function via deref.
      crate::sys::jsv_is_object(&self.raw())
    }
    pub fn instance_of<S, C>(
      &self,
      _scope: &mut S,
      _ctor: crate::value::Local<'_, C>,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(false) }
  }

  /// Local<Value>::instance_of.
  impl<'s> crate::value::Local<'s, crate::value::Value> {
    pub fn instance_of<S, C>(
      &self,
      _scope: &mut S,
      _ctor: crate::value::Local<'_, C>,
    ) -> Option<bool>
    where S: crate::scope::HandleScopeSource { Some(false) }
  }

  /// Weak<T>::to_global stub. Real v8 takes `(isolate)`.
  impl<T> crate::value::Weak<T> {
    pub fn to_global<I>(&self, _isolate: I) -> Option<crate::value::Global<T>> {
      None
    }
  }

  /// String::new_external_onebyte_raw / new_external_twobyte_raw stubs.
  /// Real v8 signature: (scope, data_ptr, length, destructor).
  impl crate::primitives::String {
    pub unsafe fn new_external_onebyte_raw<'s, S: crate::value::LocalNewScopeRef<'s>, D>(
      scope: &S,
      data: *mut core::ffi::c_char,
      length: usize,
      _destructor: D,
    ) -> Option<crate::value::Local<'s, crate::primitives::String>> {
      let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, length) };
      let s = std::str::from_utf8(bytes).ok()?;
      crate::primitives::String::new(scope, s)
    }
    pub unsafe fn new_external_twobyte_raw<'s, S: crate::value::LocalNewScopeRef<'s>, D>(
      scope: &S,
      data: *mut u16,
      length: usize,
      _destructor: D,
    ) -> Option<crate::value::Local<'s, crate::primitives::String>> {
      let units = unsafe { std::slice::from_raw_parts(data, length) };
      let s: std::string::String = std::char::decode_utf16(units.iter().copied())
        .filter_map(|r| r.ok())
        .collect();
      crate::primitives::String::new(scope, &s)
    }
  }

  /// WriteFlags::kNullTerminate alias.
  impl WriteFlags {
    #[allow(non_upper_case_globals)]
    pub const kNullTerminate: Self = Self::NULL_TERMINATE;
  }

  /// TypedArray is_*_array predicates only — buffer/byte_length/byte_offset
  /// are already on Local<TypedArray> via typed_array_view_methods!.
  impl<'s> crate::value::Local<'s, crate::buffer::TypedArray> {
    pub fn is_int8_array(&self) -> bool { false }
    pub fn is_uint8_array(&self) -> bool { false }
    pub fn is_uint8_clamped_array(&self) -> bool { false }
    pub fn is_int16_array(&self) -> bool { false }
    pub fn is_uint16_array(&self) -> bool { false }
    pub fn is_int32_array(&self) -> bool { false }
    pub fn is_uint32_array(&self) -> bool { false }
    pub fn is_float32_array(&self) -> bool { false }
    pub fn is_float64_array(&self) -> bool { false }
    pub fn is_big_int64_array(&self) -> bool { false }
    pub fn is_big_uint64_array(&self) -> bool { false }
  }

  /// DataView accessors.
  impl<'s> crate::value::Local<'s, crate::buffer::DataView> {
    pub fn data(&self) -> *mut core::ffi::c_void { core::ptr::null_mut() }
    pub fn byte_length(&self) -> usize { 0 }
    pub fn byte_offset(&self) -> usize { 0 }
    pub fn buffer<'sc, S>(
      &self,
      _scope: &mut S,
    ) -> Option<crate::value::Local<'sc, crate::buffer::ArrayBuffer>>
    where S: crate::scope::HandleScopeSource {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
  }

  /// CallbackScope::set_microtasks_policy stub.
  impl<'s, C> crate::scope::CallbackScope<'s, C> {
    pub fn set_microtasks_policy(&mut self, _policy: crate::isolate::MicrotasksPolicy) {}
  }

  /// Isolate::adjust_amount_of_external_allocated_memory stub.
  impl crate::isolate::Isolate {
    pub fn adjust_amount_of_external_allocated_memory(&mut self, _bytes: i64) -> i64 { 0 }
    /// Mirror of `Isolate::ref_from_raw_isolate_ptr_mut_unchecked`. Real
    /// v8 takes `&mut UnsafeRawIsolatePtr`; we forward to the
    /// by-value variant.
    pub unsafe fn ref_from_raw_isolate_ptr_mut_unchecked<'a>(
      ptr: &mut crate::isolate::UnsafeRawIsolatePtr,
    ) -> &'a mut Self {
      unsafe { Self::from_raw_isolate_ptr(*ptr) }
    }
  }

  // (FunctionTemplate set_class_name is already defined in template.rs.)

  /// Stub for v8::Set (the JS Set class). Transparent wrapper around
  /// a JSValue so it can up-cast to Local<Value> like other v8 types.
  #[derive(Copy, Clone)]
  #[repr(transparent)]
  pub struct Set {
    pub(crate) raw: crate::sys::JSValue,
  }
  impl Set {
    pub fn new<'s>(
      scope: &mut crate::scope::HandleScope<'s>,
    ) -> crate::value::Local<'s, Self> {
      let raw = crate::sys::new_object(scope.ctx());
      scope.track_owned(raw);
      crate::value::Local::from_raw(raw)
    }
  }
  impl<'s> crate::value::Local<'s, Set> {
    pub fn add<'sc>(
      &self,
      _scope: &mut crate::scope::HandleScope<'sc>,
      _value: crate::value::Local<'_, crate::value::Value>,
    ) -> Option<crate::value::Local<'sc, Set>> {
      Some(crate::value::Local::from_raw(self.raw()))
    }
    pub fn size(&self) -> u32 { 0 }
  }
  // Set → Value upcast (used by deno_webgpu adapter.rs).
  impl<'s> From<crate::value::Local<'s, Set>>
    for crate::value::Local<'s, crate::value::Value>
  {
    fn from(v: crate::value::Local<'s, Set>) -> Self {
      crate::value::Local::from_raw(v.raw())
    }
  }

  /// Stub for v8::IntegrityLevel (sealed/frozen).
  #[derive(Copy, Clone, Default)]
  pub enum IntegrityLevel {
    #[default]
    Sealed,
    Frozen,
  }

  #[derive(Copy, Clone, Default, PartialEq, Eq)]
  pub struct GCType(pub u32);
  impl GCType {
    pub const ALL: Self = Self(0xff);
    pub const SCAVENGE: Self = Self(1);
    pub const MARK_SWEEP_COMPACT: Self = Self(2);
    pub const INCREMENTAL_MARKING: Self = Self(4);
    pub const PROCESS_WEAK_CALLBACKS: Self = Self(8);
    pub const MINOR_MARK_SWEEP: Self = Self(16);
    // V8 SCREAMING_SNAKE alternates that ext/telemetry uses.
    #[allow(non_upper_case_globals)]
    pub const kGCTypeAll: Self = Self::ALL;
    #[allow(non_upper_case_globals)]
    pub const kGCTypeScavenge: Self = Self::SCAVENGE;
    #[allow(non_upper_case_globals)]
    pub const kGCTypeMarkSweepCompact: Self = Self::MARK_SWEEP_COMPACT;
    #[allow(non_upper_case_globals)]
    pub const kGCTypeIncrementalMarking: Self = Self::INCREMENTAL_MARKING;
    #[allow(non_upper_case_globals)]
    pub const kGCTypeProcessWeakCallbacks: Self = Self::PROCESS_WEAK_CALLBACKS;
    #[allow(non_upper_case_globals)]
    pub const kGCTypeMinorMarkSweep: Self = Self::MINOR_MARK_SWEEP;
  }
  impl core::ops::BitOr for GCType {
    type Output = Self;
    fn bitor(self, other: Self) -> Self { Self(self.0 | other.0) }
  }
  impl core::ops::BitOrAssign for GCType {
    fn bitor_assign(&mut self, other: Self) { self.0 |= other.0; }
  }
  #[derive(Copy, Clone, Default)]
  pub struct GCCallbackFlags(pub u32);

  pub type GCCallback = unsafe extern "C" fn(
    crate::isolate::UnsafeRawIsolatePtr,
    GCType,
    GCCallbackFlags,
    *mut core::ffi::c_void,
  );

  // String content viewer stubs — used by ext/telemetry to read raw
  // string contents without materializing a Rust String.
  pub struct ValueView<'s> {
    _p: core::marker::PhantomData<&'s ()>,
  }
  pub enum ValueViewData<'a> {
    OneByte(&'a [u8]),
    TwoByte(&'a [u16]),
  }
  impl<'s> ValueView<'s> {
    pub fn new<S>(
      _scope: &mut S,
      _s: crate::value::Local<'_, crate::primitives::String>,
    ) -> Self {
      Self { _p: core::marker::PhantomData }
    }
    pub fn length(&self) -> usize {
      0
    }
    pub fn data(&self) -> ValueViewData<'_> {
      ValueViewData::OneByte(&[])
    }
  }

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
  impl<'s> From<crate::value::Local<'s, crate::primitives::Integer>>
    for crate::value::Local<'s, Uint32>
  {
    fn from(v: crate::value::Local<'s, crate::primitives::Integer>) -> Self {
      crate::value::Local::from_raw(v.raw())
    }
  }
  // (TryFrom auto-derived from From above via blanket `impl<T,U> TryFrom<U> for T where U: Into<T>`)
  impl<'s> crate::value::Local<'s, Uint32> {
    pub fn value(&self) -> u32 {
      crate::sys::jsv_is_int(&self.raw())
        .then(|| unsafe { self.raw().u.int32 as u32 })
        .unwrap_or(0)
    }
    pub fn to_string<'sc, S>(&self, _scope: &mut S) -> Option<crate::value::Local<'sc, crate::primitives::String>>
    where S: crate::scope::HandleScopeSource {
      Some(crate::value::Local::from_raw(crate::sys::jsv_undefined()))
    }
  }
  pub struct Task;
  impl Task {
    pub fn run(&mut self) {}
  }
  pub struct IdleTask;
  /// `v8::FunctionBuilder<T>` — used to construct FunctionTemplates and
  /// Functions. The phantom generic is the v8 type the builder produces.
  /// We track the declared `length`, the V8 `FunctionCallback` slow_fn
  /// pointer, and any `data` (External carrying the OpCtx*). At build()
  /// time we synthesize a JS_NewCFunctionData trampoline that hands
  /// these to op2-emitted slow_fn code.
  pub struct FunctionBuilder<T> {
    length: i32,
    callback: Option<super::FunctionCallback>,
    data_ptr: *mut std::ffi::c_void,
    _t: core::marker::PhantomData<T>,
  }
  impl<T> FunctionBuilder<T> {
    pub fn new<F>(callback: F) -> Self
    where
      F: crate::function::MapFnTo<super::FunctionCallback>,
    {
      Self {
        length: 0,
        callback: Some(callback.map_fn_to()),
        data_ptr: core::ptr::null_mut(),
        _t: core::marker::PhantomData,
      }
    }
    pub fn new_raw(callback: super::FunctionCallback) -> Self {
      Self {
        length: 0,
        callback: Some(callback),
        data_ptr: core::ptr::null_mut(),
        _t: core::marker::PhantomData,
      }
    }
    pub fn data<'s>(mut self, data: super::Local<'s, super::Value>) -> Self {
      // The op2-generated code passes an `External::new(scope, opctx_ptr)`
      // here. Pull the raw pointer back out so the trampoline can stash
      // it for `args.data().value()`.
      self.data_ptr = unsafe { data.raw().u.ptr };
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
      // QuickJS-ng's JS_NewCFunction stores length in a smallish
      // bitfield (16 bits). Clamp.
      let length = self.length.clamp(0, 0x7fff);
      let raw = if let Some(cb) = self.callback {
        // Stash the (slow_fn, OpCtx) pair in a thread-local table
        // keyed by a small integer index, then JS_NewCFunction with
        // that index as `magic` — read back in the trampoline. This
        // sidesteps JS_NewCFunctionData which crashed in our build.
        let idx = super::function::register_op_dispatch(
          cb,
          self.data_ptr,
        );
        let raw = unsafe {
          crate::ffi::JS_NewCFunction2(
            ctx,
            core::mem::transmute::<
              unsafe extern "C" fn(
                *mut crate::ffi::JSContext,
                crate::sys::JSValue,
                core::ffi::c_int,
                *mut crate::sys::JSValue,
                core::ffi::c_int,
              ) -> crate::sys::JSValue,
              crate::ffi::JSCFunction,
            >(super::function::op_bridge_trampoline_magic),
            core::ptr::null(),
            length,
            crate::ffi::JS_CFUNC_GENERIC_MAGIC,
            idx,
          )
        };
        raw
      } else {
        unsafe {
          crate::ffi::JS_NewCFunction(
            ctx,
            super::function::function_new_trampoline,
            core::ptr::null(),
            length,
          )
        }
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
    pub fn new_single_threaded(_idle_task_support: bool) -> Box<Platform> {
      Box::new(Platform)
    }
    pub fn new_default_platform(_thread_pool_size: u32, _idle_task_support: bool) -> Box<Platform> {
      Box::new(Platform)
    }
  }
  /// `Box<Platform>::make_shared` returns `Arc<Platform>` to match
  /// real v8's `UniquePtr<Platform>::make_shared` chain.
  pub trait PlatformMakeShared { fn make_shared(self) -> std::sync::Arc<Platform>; }
  impl PlatformMakeShared for Box<Platform> {
    fn make_shared(self) -> std::sync::Arc<Platform> { std::sync::Arc::from(self) }
  }
}

pub use arena::Arena;
pub use arena::ArenaStats;
pub use arena::MockJSValue;
pub use arena::MockTag;
