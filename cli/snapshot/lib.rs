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
///
/// In release the source bytes live in a libsui-appended section ("dnclbk")
/// — a separate Mach-O segment / ELF trailer / PE resource that the kernel
/// keeps demand-paged. Dev builds fall back to a baked `include_bytes!`.
#[cfg(not(feature = "disable"))]
mod residual {
  include!(concat!(env!("OUT_DIR"), "/EXTENSION_RESIDUAL_SOURCES.rs"));
}

#[cfg(not(feature = "disable"))]
pub use residual::residual_lazy_esm;
#[cfg(not(feature = "disable"))]
pub use residual::residual_lazy_js;

#[cfg(feature = "disable")]
pub fn residual_lazy_js() -> &'static [(&'static str, &'static str)] {
  &[]
}
#[cfg(feature = "disable")]
pub fn residual_lazy_esm() -> &'static [(&'static str, &'static str)] {
  &[]
}

mod shared;

pub use shared::TS_VERSION;
