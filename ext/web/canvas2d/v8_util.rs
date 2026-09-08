// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::v8;

/// `ToNumber(v)` guarded by a `TryCatch`.
#[inline]
pub(super) fn to_number_guarded(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
) -> Option<f64> {
  v8::tc_scope!(tc, scope);
  let n = value.number_value(tc);
  if tc.has_caught() {
    tc.reset();
    return None;
  }
  n
}

/// `ToString(v)` guarded by a `TryCatch`.
#[inline]
pub(super) fn to_string_guarded(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
) -> Option<String> {
  v8::tc_scope!(tc, scope);
  let s = value.to_string(tc);
  if tc.has_caught() {
    tc.reset();
    return None;
  }
  s.map(|s| s.to_rust_string_lossy(tc))
}

#[inline]
pub(super) fn to_f64(
  scope: &mut v8::PinScope<'_, '_>,
  v: v8::Local<'_, v8::Value>,
) -> f64 {
  v.number_value(scope).unwrap_or(f64::NAN)
}
