// Copyright 2018-2026 the Deno authors. MIT license.

//! Helpers for the V8 `host_defined_options` PrimitiveArray attached to a
//! script's origin. Index 0 of the array stores a `Uint32` "kind" the
//! runtime can read back from the dynamic-import host callback to decide
//! how to handle `import()` calls originating from the script.

use v8::PinScope;

/// Index of the kind tag inside the host-defined-options PrimitiveArray.
pub const HOST_DEFINED_OPTIONS_KIND_INDEX: usize = 0;

/// Kind tags written at [`HOST_DEFINED_OPTIONS_KIND_INDEX`].
pub mod host_defined_options_kind {
  /// Script created by `node:vm` (`vm.Script`, `vm.runInThisContext`,
  /// `vm.compileFunction`, `vm.SourceTextModule`) without an
  /// `importModuleDynamically` callback. When the dynamic-import host
  /// callback sees this marker it rejects the import with
  /// `ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING`.
  pub const VM_DYNAMIC_IMPORT_MISSING: u32 = 1;
}

/// Build a host-defined-options PrimitiveArray with the given kind tag.
pub fn create_host_defined_options_with_kind<'s>(
  scope: &mut PinScope<'s, '_>,
  kind: u32,
) -> v8::Local<'s, v8::Data> {
  let arr = v8::PrimitiveArray::new(scope, 1);
  let value = v8::Integer::new_from_unsigned(scope, kind);
  arr.set(scope, HOST_DEFINED_OPTIONS_KIND_INDEX, value.into());
  arr.into()
}

/// Read the kind tag from a host-defined-options value. Returns `None`
/// when the value isn't a non-empty PrimitiveArray whose first element
/// is a numeric primitive (matching what [`create_host_defined_options_with_kind`]
/// writes).
pub fn read_host_defined_options_kind(
  scope: &mut PinScope<'_, '_>,
  host_defined_options: v8::Local<v8::Data>,
) -> Option<u32> {
  // V8's HostImportModuleDynamicallyCallback contract is that
  // `host_defined_options` is always a `v8::PrimitiveArray` (V8 supplies an
  // empty one when the embedder didn't set any). rusty_v8 lacks a checked
  // `TryFrom<Data> for PrimitiveArray` impl, so we cast unchecked; the
  // resulting `length()` is 0 when V8 supplied the empty fallback, and the
  // `Uint32` check below safely returns `None` for the embedder's other
  // PrimitiveArray shapes (e.g. `[Boolean(true)]`).
  // SAFETY: `Local<PrimitiveArray>` is layout-compatible with `Local<Data>`
  // (see `impl_deref!` / `impl_from!` in the v8 crate), and V8 guarantees
  // the input is a PrimitiveArray.
  let arr: v8::Local<v8::PrimitiveArray> = unsafe {
    std::mem::transmute::<v8::Local<v8::Data>, v8::Local<v8::PrimitiveArray>>(
      host_defined_options,
    )
  };
  if arr.length() == HOST_DEFINED_OPTIONS_KIND_INDEX {
    return None;
  }
  let primitive = arr.get(scope, HOST_DEFINED_OPTIONS_KIND_INDEX);
  let value: v8::Local<v8::Value> = primitive.into();
  let int = v8::Local::<v8::Uint32>::try_from(value).ok()?;
  Some(int.value())
}
