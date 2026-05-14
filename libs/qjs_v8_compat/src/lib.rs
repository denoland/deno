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
    pub struct GarbageCollected;
    pub struct Member<T>(core::marker::PhantomData<T>);
    pub struct Ptr<T>(core::marker::PhantomData<T>);
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
      pub const fn new(ty: Type, seq: SequenceType, flags: Flags) -> Self {
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
      _return_info: *const CTypeInfo,
      _args: *const CTypeInfo,
      _len: usize,
      _i64: Int64Representation,
    }

    impl CFunctionInfo {
      pub const fn new(
        return_info: *const CTypeInfo,
        args: *const CTypeInfo,
        len: usize,
        i64: Int64Representation,
      ) -> Self {
        Self {
          _return_info: return_info,
          _args: args,
          _len: len,
          _i64: i64,
        }
      }
    }

    pub struct CFunction {
      _addr: *const core::ffi::c_void,
      _info: *const CFunctionInfo,
    }

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
  }

  pub mod inspector {
    //! Stub — QuickJS has no CDP inspector. The QuickJS backend ships with
    //! the inspector disabled; debugger features are not available.
    pub struct V8Inspector;
    pub struct V8InspectorClientBase;
    pub struct V8InspectorSession;
    pub struct ChannelBase;
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

    pub struct Source;
    pub enum CompileOptions {
      NoCompileOptions,
      ConsumeCodeCache,
      EagerCompile,
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

  pub fn undefined<'s>(
    _scope: &mut crate::scope::HandleScope<'s>,
  ) -> crate::value::Local<'s, crate::value::Primitive> {
    crate::value::Local::from_raw(crate::sys::jsv_undefined())
  }
  pub fn null<'s>(
    _scope: &mut crate::scope::HandleScope<'s>,
  ) -> crate::value::Local<'s, crate::value::Primitive> {
    crate::value::Local::from_raw(crate::sys::jsv_null())
  }

  pub struct V8;
  impl V8 {
    pub fn initialize_platform(_p: std::rc::Rc<()>) {}
    pub fn initialize() {}
    pub fn dispose() -> bool {
      true
    }
    pub fn dispose_platform() {}
    pub fn set_flags_from_string(_s: &str) {}
    pub fn set_flags_from_command_line(args: Vec<String>) -> Vec<String> {
      args
    }
  }
}

pub use arena::Arena;
pub use arena::ArenaStats;
pub use arena::MockJSValue;
pub use arena::MockTag;
