// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;

use crate::canvas2d::error::Canvas2DError;
use crate::css::color::is_css_system_color;
use crate::css::color::parse_css_color;

/// Validation-only implementation of the proposed `CanvasFilter` interface
/// (https://github.com/whatwg/html/issues/5621, tested as tentative in WPT).
///
/// Constructing a `CanvasFilter` validates the given filter primitives, but
/// no filter effect is applied when rendering: the Vello backend does not
/// support filter effects yet, so the object is only carried through
/// `ctx.filter` and `beginLayer()` for API-shape compatibility.
pub struct CanvasFilter {}

// SAFETY: CanvasFilter is only accessed from the JS thread.
unsafe impl GarbageCollected for CanvasFilter {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CanvasFilter"
  }
}

#[op2]
impl CanvasFilter {
  #[constructor]
  #[cppgc]
  #[reentrant]
  fn new(
    scope: &mut v8::PinScope<'_, '_>,
    init: v8::Local<'_, v8::Value>,
  ) -> Result<CanvasFilter, Canvas2DError> {
    validate_filter_input(scope, init)?;
    Ok(CanvasFilter {})
  }
}

/// Validates a `(CanvasFilterInput or sequence<CanvasFilterInput>)` value.
pub(super) fn validate_filter_input(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
) -> Result<(), Canvas2DError> {
  if value.is_array() {
    let arr: v8::Local<'_, v8::Array> = value.try_into().unwrap();
    for i in 0..arr.length() {
      let elem = arr.get_index(scope, i).filter(|v| v.is_object()).ok_or(
        Canvas2DError::InvalidFilterPrimitive("filter must be an object"),
      )?;
      validate_filter_primitive(scope, elem.cast::<v8::Object>())?;
    }
    Ok(())
  } else if value.is_object() {
    validate_filter_primitive(scope, value.cast::<v8::Object>())
  } else {
    Err(Canvas2DError::InvalidFilterPrimitive(
      "filter input must be an object or a sequence of objects",
    ))
  }
}

/// Validates a single filter primitive dictionary, dispatching on its `name`
/// member. Unknown (or missing) filter names are tolerated per the WPT
/// beginLayer-options expectations; only the parameters of recognized
/// primitives are checked.
pub(super) fn validate_filter_primitive(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  let Some(name) = get_own_property(scope, obj, "name") else {
    return Ok(());
  };
  let Some(name) = to_string_guarded(scope, name) else {
    return Err(Canvas2DError::InvalidFilterPrimitive(
      "filter name cannot be converted to a string",
    ));
  };
  match name.as_str() {
    "gaussianBlur" => validate_gaussian_blur(scope, obj),
    "colorMatrix" => validate_color_matrix(scope, obj),
    "convolveMatrix" => validate_convolve_matrix(scope, obj),
    "dropShadow" => validate_drop_shadow(scope, obj),
    "turbulence" => validate_turbulence(scope, obj),
    _ => Ok(()),
  }
}

fn validate_gaussian_blur(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  let Some(std_deviation) = get_own_property(scope, obj, "stdDeviation") else {
    return Err(Canvas2DError::InvalidFilterPrimitive(
      "gaussianBlur requires a stdDeviation",
    ));
  };
  validate_number_or_pair(
    scope,
    std_deviation,
    false,
    "gaussianBlur stdDeviation must be a finite number or a sequence of at most two finite numbers",
  )
}

fn validate_color_matrix(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  if let Some(matrix_type) = get_own_property(scope, obj, "type")
    && let Some(matrix_type) = to_string_guarded(scope, matrix_type)
    && matches!(
      matrix_type.as_str(),
      "hueRotate" | "saturate" | "luminanceToAlpha"
    )
  {
    return Ok(());
  }
  const DETAIL: &str =
    "colorMatrix requires a values sequence of 20 finite numbers";
  let values = get_own_property(scope, obj, "values")
    .filter(|v| v.is_array())
    .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
  let arr = values.cast::<v8::Array>();
  if arr.length() != 20 {
    return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
  }
  for i in 0..arr.length() {
    let elem = arr
      .get_index(scope, i)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    require_finite_number(scope, elem, false, DETAIL)?;
  }
  Ok(())
}

fn validate_convolve_matrix(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  const DETAIL: &str = "convolveMatrix requires a kernelMatrix of equal-length rows of finite numbers";
  let kernel_matrix = get_own_property(scope, obj, "kernelMatrix")
    .filter(|v| v.is_array())
    .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
  let rows = kernel_matrix.cast::<v8::Array>();
  let num_rows = rows.length();
  if num_rows == 0 {
    return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
  }
  let mut num_cols = None;
  for i in 0..num_rows {
    let row = rows
      .get_index(scope, i)
      .filter(|v| v.is_array())
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    let row = row.cast::<v8::Array>();
    let len = row.length();
    if *num_cols.get_or_insert(len) != len {
      return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
    }
    for j in 0..len {
      let elem = row
        .get_index(scope, j)
        .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
      require_finite_number(scope, elem, false, DETAIL)?;
    }
  }
  // A single empty row is a valid degenerate kernel, but multiple zero-length
  // rows are not.
  if num_rows > 1 && num_cols == Some(0) {
    return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
  }
  Ok(())
}

fn validate_drop_shadow(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  for (key, detail) in [
    ("dx", "dropShadow dx must be a finite number"),
    ("dy", "dropShadow dy must be a finite number"),
    (
      "floodOpacity",
      "dropShadow floodOpacity must be a finite number",
    ),
  ] {
    if let Some(value) = get_own_property(scope, obj, key) {
      require_finite_number(scope, value, false, detail)?;
    }
  }
  if let Some(std_deviation) = get_own_property(scope, obj, "stdDeviation") {
    validate_number_or_pair(
      scope,
      std_deviation,
      false,
      "dropShadow stdDeviation must be a finite number or a sequence of at most two finite numbers",
    )?;
  }
  if let Some(flood_color) = get_own_property(scope, obj, "floodColor") {
    const DETAIL: &str = "dropShadow floodColor must be a valid CSS color";
    let color = to_string_guarded(scope, flood_color)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    if parse_css_color(&color).is_err() && !is_css_system_color(&color) {
      return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
    }
  }
  Ok(())
}

fn validate_turbulence(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
) -> Result<(), Canvas2DError> {
  if let Some(base_frequency) = get_own_property(scope, obj, "baseFrequency") {
    validate_number_or_pair(
      scope,
      base_frequency,
      true,
      "turbulence baseFrequency must be a non-negative finite number or a sequence of at most two non-negative finite numbers",
    )?;
  }
  if let Some(num_octaves) = get_own_property(scope, obj, "numOctaves") {
    require_finite_number(
      scope,
      num_octaves,
      true,
      "turbulence numOctaves must be a non-negative finite number",
    )?;
  }
  if let Some(seed) = get_own_property(scope, obj, "seed") {
    require_finite_number(
      scope,
      seed,
      false,
      "turbulence seed must be a finite number",
    )?;
  }
  if let Some(stitch_tiles) = get_own_property(scope, obj, "stitchTiles") {
    const DETAIL: &str =
      "turbulence stitchTiles must be either 'stitch' or 'noStitch'";
    let stitch_tiles = to_string_guarded(scope, stitch_tiles)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    if !matches!(stitch_tiles.as_str(), "stitch" | "noStitch") {
      return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
    }
  }
  if let Some(turbulence_type) = get_own_property(scope, obj, "type") {
    const DETAIL: &str =
      "turbulence type must be either 'fractalNoise' or 'turbulence'";
    let turbulence_type = to_string_guarded(scope, turbulence_type)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    if !matches!(turbulence_type.as_str(), "fractalNoise" | "turbulence") {
      return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
    }
  }
  Ok(())
}

/// Reads an own property off a filter primitive. Members behave like a
/// `record<DOMString, any>`: an own property whose value is `undefined` is
/// still present (and validated), while an absent property falls back to the
/// primitive's default.
fn get_own_property<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'_, v8::Object>,
  key: &str,
) -> Option<v8::Local<'s, v8::Value>> {
  let key = v8::String::new(scope, key).unwrap();
  if !obj.has_own_property(scope, key.into()).unwrap_or(false) {
    return None;
  }
  obj.get(scope, key.into())
}

/// Validates a scalar number or a sequence of at most two numbers, each of
/// which must be finite (and non-negative when `non_negative` is set).
fn validate_number_or_pair(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
  non_negative: bool,
  detail: &'static str,
) -> Result<(), Canvas2DError> {
  if value.is_array() {
    let arr = value.cast::<v8::Array>();
    if arr.length() > 2 {
      return Err(Canvas2DError::InvalidFilterPrimitive(detail));
    }
    for i in 0..arr.length() {
      let elem = arr
        .get_index(scope, i)
        .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
      require_finite_number(scope, elem, non_negative, detail)?;
    }
    Ok(())
  } else {
    require_finite_number(scope, value, non_negative, detail).map(|_| ())
  }
}

fn require_finite_number(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
  non_negative: bool,
  detail: &'static str,
) -> Result<f64, Canvas2DError> {
  let n = to_number_guarded(scope, value)
    .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
  if !n.is_finite() || (non_negative && n < 0.0) {
    return Err(Canvas2DError::InvalidFilterPrimitive(detail));
  }
  Ok(n)
}

/// `ToNumber(v)` guarded by a `TryCatch`: a user `valueOf` may throw, and an
/// unguarded `number_value` would leave that exception pending on the isolate.
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

/// `ToString(v)` guarded by a `TryCatch`, see [`to_number_guarded`].
fn to_string_guarded(
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
