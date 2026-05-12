// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(not(feature = "disable"))]
pub static CLI_SNAPSHOT: Option<&[u8]> = Some(include_bytes!(concat!(
  env!("OUT_DIR"),
  "/CLI_SNAPSHOT.bin"
)));
#[cfg(feature = "disable")]
pub static CLI_SNAPSHOT: Option<&[u8]> = None;

/// Pre-baked (specifier, transpiled-source) pairs captured by the snapshot
/// build's extension transpiler. Format: a sequence of records, each
/// `len(u32 LE) | specifier | len(u32 LE) | source`. Used by the cli at
/// startup to install a lookup-only `extension_transpiler` so TS files in
/// `lazy_loaded_js`/`lazy_loaded_esm` can be lazily loaded at runtime
/// without `deno_ast` being a runtime dependency.
#[cfg(not(feature = "disable"))]
pub static CLI_TRANSPILED_LAZY: &[u8] = include_bytes!(concat!(
  env!("OUT_DIR"),
  "/CLI_TRANSPILED_LAZY.bin"
));
#[cfg(feature = "disable")]
pub static CLI_TRANSPILED_LAZY: &[u8] = &[];

/// Decode `CLI_TRANSPILED_LAZY` into an iterator of `(specifier, source)`
/// borrowed pairs. Cheap; just walks the byte stream.
pub fn decode_transpiled_lazy(
  bytes: &'static [u8],
) -> impl Iterator<Item = (&'static str, &'static str)> {
  let mut cursor = 0usize;
  std::iter::from_fn(move || {
    if cursor + 4 > bytes.len() {
      return None;
    }
    let spec_len = u32::from_le_bytes(
      bytes[cursor..cursor + 4].try_into().unwrap(),
    ) as usize;
    cursor += 4;
    let specifier = std::str::from_utf8(&bytes[cursor..cursor + spec_len])
      .expect("transpiled specifier was not utf-8");
    cursor += spec_len;
    let src_len = u32::from_le_bytes(
      bytes[cursor..cursor + 4].try_into().unwrap(),
    ) as usize;
    cursor += 4;
    let source = std::str::from_utf8(&bytes[cursor..cursor + src_len])
      .expect("transpiled source was not utf-8");
    cursor += src_len;
    Some((specifier, source))
  })
}

mod shared;

pub use shared::TS_VERSION;
