// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(not(feature = "disable"))]
pub static CLI_SNAPSHOT: Option<&[u8]> = Some(include_bytes!(concat!(
  env!("OUT_DIR"),
  "/CLI_SNAPSHOT.bin"
)));
#[cfg(feature = "disable")]
pub static CLI_SNAPSHOT: Option<&[u8]> = None;

/// `(specifier, source)` pairs for every `lazy_loaded_js` / `lazy_loaded_esm`
/// file that was *not* consumed during snapshot creation. These still need to
/// be available at runtime for `core.loadExtScript()` / the createLazyLoader
/// factory; consumed files live in the snapshot blob itself.
#[cfg(not(feature = "disable"))]
mod residual {
  include!(concat!(env!("OUT_DIR"), "/EXTENSION_RESIDUAL_SOURCES.rs"));
}

#[cfg(not(feature = "disable"))]
pub use residual::RESIDUAL_LAZY_ESM;
#[cfg(not(feature = "disable"))]
pub use residual::RESIDUAL_LAZY_JS;

#[cfg(feature = "disable")]
pub static RESIDUAL_LAZY_JS: &[(&str, &str)] = &[];
#[cfg(feature = "disable")]
pub static RESIDUAL_LAZY_ESM: &[(&str, &str)] = &[];

mod shared;

pub use shared::TS_VERSION;
