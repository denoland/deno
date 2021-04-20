mod js_runtime;
mod profiling;

pub use bencher;
pub use js_runtime::*;
pub use profiling::*; // Exports bench_or_profile! macro
