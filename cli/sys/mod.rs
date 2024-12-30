// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[cfg(denort)]
mod compile;
#[cfg(denort)]
pub use compile::DenoCompileFileSystem as CliDenoFs;

#[cfg(not(denort))]
mod real;
#[cfg(not(denort))]
pub use real::CliDenoFs;
#[cfg(not(denort))]
pub use real::CliSys;
