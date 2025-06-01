// Copyright 2018-2025 the Deno authors. MIT license.
mod js_runtime;
mod profiling;

pub use bencher;
pub use js_runtime::*;
pub use profiling::*; // Exports bench_or_profile! macro
