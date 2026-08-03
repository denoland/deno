// Copyright 2018-2026 the Deno authors. MIT license.

pub mod width_128;

pub use width_128::Simd8x64;

// #[cfg(any(target_arch = "x86_64"))]
// pub mod width_256;

// #[cfg(any(target_arch = "x86_64"))]
// pub use width_256::Simd8x64;
