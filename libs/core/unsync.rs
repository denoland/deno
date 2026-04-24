// Copyright 2018-2026 the Deno authors. MIT license.

//! Re-exports for async task spawning primitives.
//!
//! With tokio's `LocalRuntime`, we can use `spawn_local` directly instead of
//! the `MaskFutureAsSend` workaround that `deno_unsync` used. This module
//! re-exports tokio's native task spawning alongside the remaining utilities
//! from `deno_unsync`.

// Task spawning - use tokio's native local spawning instead of deno_unsync's
// MaskFutureAsSend workaround.
pub use deno_unsync::Flag;
pub use deno_unsync::MaskFutureAsSend;
pub use deno_unsync::TaskQueue;
pub use deno_unsync::TaskQueuePermit;
pub use deno_unsync::TaskQueuePermitAcquireFuture;
pub use deno_unsync::UnsendMarker;
pub use deno_unsync::UnsyncWaker;
// Re-export remaining deno_unsync utilities
pub use deno_unsync::future;
pub use deno_unsync::mpsc;
pub use deno_unsync::sync;
pub use tokio::task::JoinHandle;
pub use tokio::task::spawn_blocking;
pub use tokio::task::spawn_local as spawn;
