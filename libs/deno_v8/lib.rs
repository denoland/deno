// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(all(feature = "v8", feature = "quickjs"))]
compile_error!("features `v8` and `quickjs` are mutually exclusive");

#[cfg(not(any(feature = "v8", feature = "quickjs")))]
compile_error!("either feature `v8` or `quickjs` must be enabled");

#[cfg(all(feature = "v8", not(feature = "quickjs")))]
pub use rusty_v8::*;
#[cfg(all(feature = "quickjs", not(feature = "v8")))]
pub use v8x_backend::*;

/// The `ReturnValue` handed to the named and indexed property *setter* and
/// *definer* interceptors.
///
/// V8 152 changed these two interceptors from `ReturnValue<()>` to
/// `ReturnValue<Boolean>`, bringing them in line with the deleter interceptor,
/// which already carried a `Boolean`. The QuickJS backend vendors an older
/// rusty_v8 and still expects `ReturnValue<()>`, so interceptor callbacks name
/// this alias rather than writing the payload type out and only compiling
/// against one backend.
#[cfg(all(feature = "v8", not(feature = "quickjs")))]
pub type PropertyInterceptorReturnValue<'cb> =
  rusty_v8::ReturnValue<'cb, rusty_v8::Boolean>;
#[cfg(all(feature = "quickjs", not(feature = "v8")))]
pub type PropertyInterceptorReturnValue<'cb> =
  v8x_backend::ReturnValue<'cb, ()>;

/// What `MicrotaskQueue::new` hands back: an owning RAII handle that releases
/// the queue on drop and derefs to [`MicrotaskQueue`].
///
/// V8 152 returns a `MicrotaskQueueHandle`, which owns a root into the
/// isolate's heap rather than the queue itself. The QuickJS backend vendors an
/// older rusty_v8 that returns `UniqueRef<MicrotaskQueue>`, which owns the
/// queue allocation directly. Both drop correctly on their own, so holders can
/// name this alias and let ordinary ownership do the cleanup.
#[cfg(all(feature = "v8", not(feature = "quickjs")))]
pub type OwnedMicrotaskQueue = rusty_v8::MicrotaskQueueHandle;
#[cfg(all(feature = "quickjs", not(feature = "v8")))]
pub type OwnedMicrotaskQueue =
  v8x_backend::UniqueRef<v8x_backend::MicrotaskQueue>;
