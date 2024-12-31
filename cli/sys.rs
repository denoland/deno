// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[cfg(denort)]
pub type CliDenoFs = standalone::DenoCompileFileSystem;
#[cfg(denort)]
pub type CliSys = standalone::DenoCompileFileSystem;

#[cfg(not(denort))]
pub type CliDenoFs = deno_runtime::deno_fs::RealFs;
#[cfg(not(denort))]
pub type CliSys = sys_traits::impls::RealSys;
