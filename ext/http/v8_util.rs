// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::v8;

pub(crate) fn v8_string_to_utf8_bytes<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  text: v8::Local<'s, v8::String>,
) -> Vec<u8> {
  if text.is_onebyte() || text.contains_only_onebyte() {
    let len = text.length();
    let mut bytes = Vec::with_capacity(len);
    text.write_one_byte_uninit_v2(
      scope,
      0,
      &mut bytes.spare_capacity_mut()[..len],
      v8::WriteFlags::empty(),
    );
    // SAFETY: write_one_byte_uninit_v2 initialized exactly `len` bytes in
    // the spare capacity above.
    unsafe { bytes.set_len(len) };

    if v8::simdutf::validate_ascii(&bytes) {
      return bytes;
    }

    let mut utf8 = Vec::with_capacity(len * 2);
    let written =
      // SAFETY: Latin-1 expands to at most two UTF-8 bytes per input byte.
      unsafe { v8::latin1_to_utf8(len, bytes.as_ptr(), utf8.as_mut_ptr()) };
    debug_assert!(written <= utf8.capacity());
    // SAFETY: latin1_to_utf8 initialized exactly `written` bytes.
    unsafe { utf8.set_len(written) };
    return utf8;
  }

  let len = text.utf8_length(scope);
  let mut bytes = Vec::with_capacity(len);
  let written = text.write_utf8_uninit_v2(
    scope,
    bytes.spare_capacity_mut(),
    v8::WriteFlags::kReplaceInvalidUtf8,
    None,
  );
  debug_assert!(written <= len);
  // SAFETY: write_utf8_uninit_v2 initialized exactly `written` bytes.
  unsafe { bytes.set_len(written) };
  bytes
}
