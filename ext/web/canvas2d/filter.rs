// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::v8;

use crate::canvas2d::error::Canvas2DError;
use crate::css::color::Color;
use crate::css::color::is_css_system_color;
use crate::css::color::parse_css_color;

#[allow(
  dead_code,
  reason = "layer filter primitives are retained in state before rendering support"
)]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CanvasLayerFilterPrimitive {
  GaussianBlur {
    std_deviation_x: f64,
    std_deviation_y: f64,
  },
  ColorMatrix {
    values: [f64; 20],
  },
  ComponentTransfer {
    func_r: CanvasLayerComponentTransferFunc,
    func_g: CanvasLayerComponentTransferFunc,
    func_b: CanvasLayerComponentTransferFunc,
    func_a: CanvasLayerComponentTransferFunc,
  },
  ConvolveMatrix {
    kernel_matrix: Vec<Vec<f64>>,
  },
  DropShadow {
    dx: f64,
    dy: f64,
    std_deviation_x: f64,
    std_deviation_y: f64,
    flood_color: Color,
    flood_opacity: f64,
  },
  Turbulence {
    base_frequency_x: f64,
    base_frequency_y: f64,
    num_octaves: f64,
    seed: f64,
    stitch_tiles: bool,
    kind: CanvasLayerTurbulenceKind,
  },
}

#[allow(
  dead_code,
  reason = "layer filter primitives are retained in state before rendering support"
)]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CanvasLayerComponentTransferFunc {
  Identity,
  Table(Vec<f64>),
  Discrete(Vec<f64>),
  Linear {
    slope: f64,
    intercept: f64,
  },
  Gamma {
    amplitude: f64,
    exponent: f64,
    offset: f64,
  },
}

#[allow(
  dead_code,
  reason = "layer filter primitives are retained in state before rendering support"
)]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CanvasLayerTurbulenceKind {
  FractalNoise,
  Turbulence,
}

/// Parses the object-form `beginLayer({ filter })` value.
/// https://github.com/whatwg/html/pull/9537
#[inline]
pub(super) fn parse_filter_input<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  value: v8::Local<'a, v8::Value>,
) -> Result<Vec<CanvasLayerFilterPrimitive>, Canvas2DError> {
  if value.is_array() {
    let arr = value.cast::<v8::Array>();
    let len = arr.length();
    let mut primitives = Vec::with_capacity(len as usize);
    for i in 0..len {
      let elem = arr.get_index(scope, i).filter(|v| v.is_object()).ok_or(
        Canvas2DError::InvalidFilterPrimitive("filter must be an object"),
      )?;
      if let Some(primitive) =
        parse_filter_primitive(scope, elem.cast::<v8::Object>())?
      {
        primitives.push(primitive);
      }
    }
    Ok(primitives)
  } else if value.is_object() {
    Ok(
      parse_filter_primitive(scope, value.cast::<v8::Object>())?
        .into_iter()
        .collect(),
    )
  } else {
    Err(Canvas2DError::InvalidFilterPrimitive(
      "filter input must be an object or a sequence of objects",
    ))
  }
}

#[inline]
fn parse_filter_primitive<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<Option<CanvasLayerFilterPrimitive>, Canvas2DError> {
  let Some(name) = get_property(scope, obj, "name") else {
    return Ok(None);
  };
  let Some(name) = to_string_guarded(scope, name) else {
    return Err(Canvas2DError::InvalidFilterPrimitive(
      "filter name must be a string",
    ));
  };

  let primitive = match name.as_str() {
    "gaussianBlur" => Some(parse_gaussian_blur(scope, obj)?),
    "colorMatrix" => Some(parse_color_matrix(scope, obj)?),
    "componentTransfer" => Some(parse_component_transfer(scope, obj)?),
    "convolveMatrix" => Some(parse_convolve_matrix(scope, obj)?),
    "dropShadow" => Some(parse_drop_shadow(scope, obj)?),
    "turbulence" => Some(parse_turbulence(scope, obj)?),
    _ => None,
  };
  Ok(primitive)
}

#[inline]
fn parse_gaussian_blur<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  let Some(std_deviation) = get_property(scope, obj, "stdDeviation") else {
    return Err(Canvas2DError::InvalidFilterPrimitive(
      "gaussianBlur requires a stdDeviation",
    ));
  };
  let (std_deviation_x, std_deviation_y) = parse_number_or_pair(
    scope,
    std_deviation,
    false,
    "gaussianBlur stdDeviation must be a finite number or a sequence of at most two finite numbers",
  )?;
  Ok(CanvasLayerFilterPrimitive::GaussianBlur {
    std_deviation_x,
    std_deviation_y,
  })
}

#[inline]
fn parse_color_matrix<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  if let Some(matrix_type) =
    get_property(scope, obj, "type").and_then(|v| to_string_guarded(scope, v))
    && matches!(
      matrix_type.as_str(),
      "hueRotate" | "saturate" | "luminanceToAlpha"
    )
  {
    return Ok(CanvasLayerFilterPrimitive::ColorMatrix {
      values: identity_color_matrix(),
    });
  }

  const DETAIL: &str =
    "colorMatrix requires a values sequence of 20 finite numbers";
  let values = get_property(scope, obj, "values")
    .filter(|v| v.is_array())
    .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
  let arr = values.cast::<v8::Array>();
  if arr.length() != 20 {
    return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
  }
  let mut matrix = [0.0; 20];
  for i in 0..20 {
    let elem = arr
      .get_index(scope, i)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    matrix[i as usize] = require_finite_number(scope, elem, false, DETAIL)?;
  }
  Ok(CanvasLayerFilterPrimitive::ColorMatrix { values: matrix })
}

#[inline]
fn parse_component_transfer<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  Ok(CanvasLayerFilterPrimitive::ComponentTransfer {
    func_r: parse_component_transfer_func(scope, obj, "funcR")?,
    func_g: parse_component_transfer_func(scope, obj, "funcG")?,
    func_b: parse_component_transfer_func(scope, obj, "funcB")?,
    func_a: parse_component_transfer_func(scope, obj, "funcA")?,
  })
}

#[inline]
fn parse_component_transfer_func<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
  name: &str,
) -> Result<CanvasLayerComponentTransferFunc, Canvas2DError> {
  let Some(value) = get_property(scope, obj, name) else {
    return Ok(CanvasLayerComponentTransferFunc::Identity);
  };
  if !value.is_object() {
    let Some(func_type) = to_string_guarded(scope, value) else {
      return Err(Canvas2DError::InvalidFilterPrimitive(
        "componentTransfer function type must be a string",
      ));
    };
    return parse_component_transfer_func_by_type(scope, None, &func_type);
  }

  let func_obj = value.cast::<v8::Object>();
  let Some(func_type) = get_property(scope, func_obj, "type")
    .and_then(|v| to_string_guarded(scope, v))
  else {
    return Ok(CanvasLayerComponentTransferFunc::Identity);
  };
  parse_component_transfer_func_by_type(scope, Some(func_obj), &func_type)
}

#[inline]
fn parse_component_transfer_func_by_type<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: Option<v8::Local<'a, v8::Object>>,
  func_type: &str,
) -> Result<CanvasLayerComponentTransferFunc, Canvas2DError> {
  match func_type {
    "identity" => Ok(CanvasLayerComponentTransferFunc::Identity),
    "table" | "discrete" => {
      let Some(obj) = obj else {
        return Err(Canvas2DError::InvalidFilterPrimitive(
          "componentTransfer tableValues must be a sequence of finite numbers",
        ));
      };
      let values = parse_number_array(
        scope,
        obj,
        "tableValues",
        "componentTransfer tableValues must be a sequence of finite numbers",
      )?;
      if func_type == "table" {
        Ok(CanvasLayerComponentTransferFunc::Table(values))
      } else {
        Ok(CanvasLayerComponentTransferFunc::Discrete(values))
      }
    }
    "linear" => {
      let Some(obj) = obj else {
        return Err(Canvas2DError::InvalidFilterPrimitive(
          "componentTransfer linear function requires slope and intercept",
        ));
      };
      Ok(CanvasLayerComponentTransferFunc::Linear {
        slope: optional_finite_number(scope, obj, "slope", 1.0, false)?,
        intercept: optional_finite_number(scope, obj, "intercept", 0.0, false)?,
      })
    }
    "gamma" => {
      let Some(obj) = obj else {
        return Err(Canvas2DError::InvalidFilterPrimitive(
          "componentTransfer gamma function requires amplitude, exponent, and offset",
        ));
      };
      Ok(CanvasLayerComponentTransferFunc::Gamma {
        amplitude: optional_finite_number(scope, obj, "amplitude", 1.0, false)?,
        exponent: optional_finite_number(scope, obj, "exponent", 1.0, false)?,
        offset: optional_finite_number(scope, obj, "offset", 0.0, false)?,
      })
    }
    _ => Err(Canvas2DError::InvalidFilterPrimitive(
      "componentTransfer type must be identity, table, discrete, linear, or gamma",
    )),
  }
}

#[inline]
fn parse_convolve_matrix<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  const DETAIL: &str = "convolveMatrix requires a kernelMatrix of equal-length rows of finite numbers";
  let kernel_matrix = get_property(scope, obj, "kernelMatrix")
    .filter(|v| v.is_array())
    .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
  let rows = kernel_matrix.cast::<v8::Array>();
  let num_rows = rows.length();
  if num_rows == 0 {
    return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
  }

  let mut num_cols = None;
  let mut matrix = Vec::with_capacity(num_rows as usize);
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
    if num_rows > 1 && len == 0 {
      return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL));
    }
    let mut parsed_row = Vec::with_capacity(len as usize);
    for j in 0..len {
      let elem = row
        .get_index(scope, j)
        .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
      parsed_row.push(require_finite_number(scope, elem, false, DETAIL)?);
    }
    matrix.push(parsed_row);
  }

  Ok(CanvasLayerFilterPrimitive::ConvolveMatrix {
    kernel_matrix: matrix,
  })
}

#[inline]
fn parse_drop_shadow<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  let dx = optional_finite_number(scope, obj, "dx", 0.0, false)?;
  let dy = optional_finite_number(scope, obj, "dy", 0.0, false)?;
  let (std_deviation_x, std_deviation_y) = if let Some(std_deviation) =
    get_property(scope, obj, "stdDeviation")
  {
    parse_number_or_pair(
      scope,
      std_deviation,
      false,
      "dropShadow stdDeviation must be a finite number or a sequence of at most two finite numbers",
    )?
  } else {
    (0.0, 0.0)
  };
  let flood_opacity =
    optional_finite_number(scope, obj, "floodOpacity", 1.0, false)?;
  let flood_color =
    if let Some(flood_color) = get_property(scope, obj, "floodColor") {
      let Some(color) = to_string_guarded(scope, flood_color) else {
        return Err(Canvas2DError::InvalidFilterPrimitive(
          "dropShadow floodColor must be a CSS color",
        ));
      };
      if is_css_system_color(&color) {
        Color::BLACK
      } else {
        parse_css_color(&color)
          .map_err(|_| {
            Canvas2DError::InvalidFilterPrimitive(
              "dropShadow floodColor must be a CSS color",
            )
          })?
          .to_srgb8()
      }
    } else {
      Color::BLACK
    };

  Ok(CanvasLayerFilterPrimitive::DropShadow {
    dx,
    dy,
    std_deviation_x,
    std_deviation_y,
    flood_color,
    flood_opacity,
  })
}

#[inline]
fn parse_turbulence<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
) -> Result<CanvasLayerFilterPrimitive, Canvas2DError> {
  let (base_frequency_x, base_frequency_y) = if let Some(base_frequency) =
    get_property(scope, obj, "baseFrequency")
  {
    parse_number_or_pair(
      scope,
      base_frequency,
      true,
      "turbulence baseFrequency must be a non-negative finite number or a sequence of at most two non-negative finite numbers",
    )?
  } else {
    (0.0, 0.0)
  };
  let num_octaves =
    optional_finite_number(scope, obj, "numOctaves", 1.0, true)?;
  let seed = optional_finite_number(scope, obj, "seed", 0.0, false)?;
  let stitch_tiles =
    if let Some(stitch_tiles) = get_property(scope, obj, "stitchTiles") {
      const DETAIL: &str =
        "turbulence stitchTiles must be either 'stitch' or 'noStitch'";
      let stitch_tiles = to_string_guarded(scope, stitch_tiles)
        .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
      match stitch_tiles.as_str() {
        "stitch" => true,
        "noStitch" => false,
        _ => return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL)),
      }
    } else {
      false
    };
  let kind = if let Some(turbulence_type) = get_property(scope, obj, "type") {
    const DETAIL: &str =
      "turbulence type must be either 'fractalNoise' or 'turbulence'";
    let turbulence_type = to_string_guarded(scope, turbulence_type)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(DETAIL))?;
    match turbulence_type.as_str() {
      "fractalNoise" => CanvasLayerTurbulenceKind::FractalNoise,
      "turbulence" => CanvasLayerTurbulenceKind::Turbulence,
      _ => return Err(Canvas2DError::InvalidFilterPrimitive(DETAIL)),
    }
  } else {
    CanvasLayerTurbulenceKind::Turbulence
  };

  Ok(CanvasLayerFilterPrimitive::Turbulence {
    base_frequency_x,
    base_frequency_y,
    num_octaves,
    seed,
    stitch_tiles,
    kind,
  })
}

#[inline]
fn parse_number_or_pair<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  value: v8::Local<'a, v8::Value>,
  non_negative: bool,
  detail: &'static str,
) -> Result<(f64, f64), Canvas2DError> {
  if value.is_array() {
    let arr = value.cast::<v8::Array>();
    let len = arr.length();
    if len > 2 {
      return Err(Canvas2DError::InvalidFilterPrimitive(detail));
    }
    if len == 0 {
      return Ok((0.0, 0.0));
    }
    let first = arr
      .get_index(scope, 0)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
    let first = require_finite_number(scope, first, non_negative, detail)?;
    let second = if len == 2 {
      let second = arr
        .get_index(scope, 1)
        .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
      require_finite_number(scope, second, non_negative, detail)?
    } else {
      first
    };
    Ok((first, second))
  } else {
    let number = require_finite_number(scope, value, non_negative, detail)?;
    Ok((number, number))
  }
}

#[inline]
fn parse_number_array<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
  property: &str,
  detail: &'static str,
) -> Result<Vec<f64>, Canvas2DError> {
  let values = get_property(scope, obj, property)
    .filter(|v| v.is_array())
    .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
  let arr = values.cast::<v8::Array>();
  if arr.length() == 0 {
    return Err(Canvas2DError::InvalidFilterPrimitive(detail));
  }
  let len = arr.length();
  let mut result = Vec::with_capacity(len as usize);
  for i in 0..len {
    let elem = arr
      .get_index(scope, i)
      .ok_or(Canvas2DError::InvalidFilterPrimitive(detail))?;
    result.push(require_finite_number(scope, elem, false, detail)?);
  }
  Ok(result)
}

#[inline]
fn optional_finite_number<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  obj: v8::Local<'a, v8::Object>,
  property: &str,
  default: f64,
  non_negative: bool,
) -> Result<f64, Canvas2DError> {
  let Some(value) = get_property(scope, obj, property) else {
    return Ok(default);
  };
  require_finite_number(
    scope,
    value,
    non_negative,
    "filter property must be a finite number",
  )
}

fn require_finite_number<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  value: v8::Local<'a, v8::Value>,
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

#[inline]
fn get_property<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  name: &str,
) -> Option<v8::Local<'s, v8::Value>> {
  let key = v8::String::new(scope, name)?;
  if !obj.has_own_property(scope, key.into()).unwrap_or(false) {
    return None;
  }
  obj.get(scope, key.into())
}

/// `ToNumber(v)` guarded by a `TryCatch`: if user code throws during
/// conversion, treat it as an invalid primitive instead of leaking the
/// exception through `beginLayer()`.
#[inline]
fn to_number_guarded<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  value: v8::Local<'a, v8::Value>,
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
fn to_string_guarded<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  value: v8::Local<'a, v8::Value>,
) -> Option<String> {
  v8::tc_scope!(tc, scope);
  let s = value.to_string(tc);
  if tc.has_caught() {
    tc.reset();
    return None;
  }
  s.map(|s| s.to_rust_string_lossy(tc))
}

#[rustfmt::skip]
#[inline]
fn identity_color_matrix() -> [f64; 20] {
  [
    1.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 1.0, 0.0,
  ]
}
