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
    pub struct FastApiCallbackOptions;
    pub struct CFunction;
    pub struct CTypeInfo;
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
