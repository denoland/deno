// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(clippy::disallowed_methods)]

mod cache;
mod common;
mod deno_dir;
#[cfg(feature = "file_fetcher")]
pub mod file_fetcher;
mod global;
mod local;
pub mod memory;
pub mod npm;

/// Permissions used to save a file in the disk caches.
pub const CACHE_PERM: u32 = 0o644;

pub use cache::CacheEntry;
pub use cache::CacheReadFileError;
pub use cache::Checksum;
pub use cache::ChecksumIntegrityError;
pub use cache::GlobalOrLocalHttpCache;
pub use cache::GlobalToLocalCopy;
pub use cache::HttpCache;
pub use cache::HttpCacheItemKey;
pub use cache::HttpCacheRc;
pub use cache::SerializedCachedUrlMetadata;
pub use cache::url_to_filename;
pub use common::HeadersMap;
pub use deno_dir::DenoDirResolutionError;
pub use deno_dir::ResolveDenoDirOptions;
pub use deno_dir::ResolveDenoDirSys;
pub use deno_dir::resolve_deno_dir;
pub use global::GlobalHttpCache;
pub use global::GlobalHttpCacheRc;
pub use global::GlobalHttpCacheSys;
pub use local::LocalHttpCache;
pub use local::LocalHttpCacheRc;
pub use local::LocalHttpCacheSys;
pub use local::LocalLspHttpCache;
