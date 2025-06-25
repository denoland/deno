// Copyright 2018-2025 the Deno authors. MIT license.

mod cache_db;
mod caches;
mod check;
mod code_cache;
mod deno_dir;
mod disk_cache;
mod emit;
mod fast_check;
mod incremental;
mod module_info;
mod node;
mod parsed_source;

pub use cache_db::CacheDBHash;
pub use caches::Caches;
pub use check::TypeCheckCache;
pub use code_cache::CodeCache;
/// Permissions used to save a file in the disk caches.
pub use deno_cache_dir::CACHE_PERM;
pub use deno_dir::DenoDir;
pub use deno_dir::DenoDirProvider;
pub use disk_cache::DiskCache;
pub use emit::EmitCache;
pub use fast_check::FastCheckCache;
pub use incremental::IncrementalCache;
pub use module_info::ModuleInfoCache;
pub use node::NodeAnalysisCache;
pub use parsed_source::LazyGraphSourceParser;
pub use parsed_source::ParsedSourceCache;

use crate::sys::CliSys;

pub type GlobalHttpCache = deno_cache_dir::GlobalHttpCache<CliSys>;
pub type LocalLspHttpCache = deno_cache_dir::LocalLspHttpCache<CliSys>;
pub use deno_cache_dir::HttpCache;
