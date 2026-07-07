// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::v8;
use vello::kurbo;
use vello::kurbo::Affine;
use vello::kurbo::BezPath;
use vello::kurbo::ParamCurveNearest;
use vello::kurbo::PathEl;
use vello::kurbo::Point;
use vello::kurbo::Shape;
use vello::kurbo::StrokeOpts;

use super::OffscreenCanvasRenderingContext2D;
use super::draw;
use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::path::Path2D;

#[inline]
pub(super) fn v8_to_f64(
  scope: &mut v8::PinScope<'_, '_>,
  v: v8::Local<'_, v8::Value>,
) -> f64 {
  v.number_value(scope).unwrap_or(f64::NAN)
}

#[inline]
pub(super) fn type_error_not_path2d(
  prefix: &'static str,
  context: &'static str,
) -> Canvas2DError {
  Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
    prefix: prefix.into(),
    context: context.into(),
    kind: deno_core::webidl::WebIdlErrorKind::ConvertToConverterType("Path2D"),
  })
}

#[inline]
pub(super) fn resolve_point_in_path_args(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  a: Option<v8::Local<'_, v8::Value>>,
  b: Option<v8::Local<'_, v8::Value>>,
  c: Option<v8::Local<'_, v8::Value>>,
  d: Option<v8::Local<'_, v8::Value>>,
) -> Result<(BezPath, f64, f64, String, bool), Canvas2DError> {
  const PREFIX: &str =
    "Failed to execute 'isPointInPath' on 'OffscreenCanvasRenderingContext2D'";

  let validate_fill_rule =
    |context: &'static str, rule: &str| -> Result<(), Canvas2DError> {
      match rule {
        "nonzero" | "evenodd" => Ok(()),
        _ => Err(Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
          prefix: PREFIX.into(),
          context: context.into(),
          kind: deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant {
            converter: "CanvasFillRule",
            variant: rule.to_string(),
          },
        })),
      }
    };

  let Some(a) = a else {
    if d.is_some() {
      // 4 args: isPointInPath(path, x, y, fillRule) — null/undefined is not Path2D
      return Err(type_error_not_path2d(PREFIX, "parameter 1"));
    }
    if b.is_some() {
      // 2-3 args with null/undefined first: isPointInPath(x, y [, fillRule])
      let y = b.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let rule = c
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      validate_fill_rule("parameter 3", &rule)?;
      return Ok((
        context.current_path.borrow().clone(),
        f64::NAN,
        y,
        rule,
        false,
      ));
    }
    // Zero arguments: neither the (x, y [, fillRule]) nor the
    // (path, x, y [, fillRule]) overload has enough arguments.
    return Err(Canvas2DError::MissingArgument {
      required: 2,
      provided: 0,
    });
  };
  if let Some(p) = deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, a)
  {
    // isPointInPath(path, x, y [, fillRule])
    let provided = 1 + b.is_some() as u32 + c.is_some() as u32;
    let (Some(b), Some(c)) = (b, c) else {
      return Err(Canvas2DError::MissingArgument {
        required: 3,
        provided,
      });
    };
    let x = v8_to_f64(scope, b);
    let y = v8_to_f64(scope, c);
    // CanvasFillRule is a non-nullable DOMString enum, so an explicit
    // `null` must be stringified to "null" (an invalid enum value)
    // rather than falling back to the "nonzero" default like an omitted
    // argument would.
    let rule = d
      .map(|v| v.to_rust_string_lossy(scope))
      .unwrap_or_else(|| "nonzero".into());
    validate_fill_rule("parameter 4", &rule)?;
    return Ok((p.path.borrow().clone(), x, y, rule, true));
  }
  if a.is_number() {
    // isPointInPath(x, y [, fillRule])
    let Some(b) = b else {
      return Err(Canvas2DError::MissingArgument {
        required: 2,
        provided: 1,
      });
    };
    let x = v8_to_f64(scope, a);
    let y = v8_to_f64(scope, b);
    let rule = c
      .map(|v| v.to_rust_string_lossy(scope))
      .unwrap_or_else(|| "nonzero".into());
    validate_fill_rule("parameter 3", &rule)?;
    return Ok((context.current_path.borrow().clone(), x, y, rule, false));
  }
  Err(type_error_not_path2d(PREFIX, "parameter 1"))
}

#[inline]
pub(super) fn resolve_point_in_stroke_args(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  a: Option<v8::Local<'_, v8::Value>>,
  b: Option<v8::Local<'_, v8::Value>>,
  c: Option<v8::Local<'_, v8::Value>>,
) -> Result<(BezPath, f64, f64, bool), Canvas2DError> {
  const PREFIX: &str = "Failed to execute 'isPointInStroke' on 'OffscreenCanvasRenderingContext2D'";
  let Some(a) = a else {
    if c.is_some() {
      // 3 args: isPointInStroke(path, x, y) — null/undefined is not Path2D
      return Err(type_error_not_path2d(PREFIX, "parameter 1"));
    }
    if b.is_some() {
      // 2 args with null/undefined first: isPointInStroke(x, y)
      let y = b.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((context.current_path.borrow().clone(), f64::NAN, y, false));
    }
    return Ok((
      context.current_path.borrow().clone(),
      f64::NAN,
      f64::NAN,
      false,
    ));
  };
  if let Some(p) = deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, a)
  {
    // isPointInStroke(path, x, y)
    let x = b.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
    let y = c.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
    return Ok((p.path.borrow().clone(), x, y, true));
  }
  if a.is_number() {
    // isPointInStroke(x, y)
    let x = v8_to_f64(scope, a);
    let y = b.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
    return Ok((context.current_path.borrow().clone(), x, y, false));
  }
  Err(type_error_not_path2d(PREFIX, "parameter 1"))
}

/// Returns a copy of `path` with every subpath explicitly closed. Per
/// spec, isPointInPath()/isPointInStroke() (and fill()/clip()) treat each
/// subpath as though it had been closed, regardless of whether
/// closePath() was actually called.
pub(super) fn close_all_subpaths(path: &BezPath) -> BezPath {
  let mut closed = BezPath::new();
  let mut subpath_open = false;
  for el in path.iter() {
    match el {
      PathEl::MoveTo(_) => {
        if subpath_open {
          closed.push(PathEl::ClosePath);
        }
        subpath_open = true;
      }
      PathEl::ClosePath => subpath_open = false,
      _ => {}
    }
    closed.push(el);
  }
  if subpath_open {
    closed.push(PathEl::ClosePath);
  }
  closed
}

/// Returns whether `pt` lies on (within floating-point tolerance of) any
/// segment of `path`. Per spec, points exactly on the path's boundary
/// count as inside for isPointInPath().
pub(super) fn point_on_path_boundary(path: &BezPath, pt: Point) -> bool {
  const EPSILON_SQ: f64 = 1e-9;
  path
    .segments()
    .any(|seg| seg.nearest(pt, 1e-6).distance_sq <= EPSILON_SQ)
}

#[inline]
pub(super) fn test_point_in_path(
  path: BezPath,
  x: f64,
  y: f64,
  rule: String,
) -> bool {
  let path = close_all_subpaths(&path);
  let pt = Point::new(x, y);
  if point_on_path_boundary(&path, pt) {
    return true;
  }
  let w = path.winding(pt);
  match rule.as_str() {
    "evenodd" => w % 2 != 0,
    _ => w != 0,
  }
}

#[inline]
pub(super) fn test_point_in_stroke(
  context: &OffscreenCanvasRenderingContext2D,
  path: BezPath,
  x: f64,
  y: f64,
  transform: Affine,
  is_path2d: bool,
) -> bool {
  if path.is_empty() {
    return false;
  }
  // lineWidth/lineDash are specified in user-space units, so the stroke
  // outline must be built in user space. The default path is stored in
  // canvas space (see append_transformed_path), so map it back; Path2D
  // coordinates are already in user space.
  let path = if is_path2d {
    path
  } else {
    OffscreenCanvasRenderingContext2D::transform_path(
      &path,
      transform.inverse(),
    )
  };
  let state = context.state.borrow();
  let stroke = draw::build_stroke(&state);
  drop(state);
  let outline = kurbo::stroke(
    path.path_elements(0.01),
    &stroke,
    &StrokeOpts::default(),
    0.01,
  );
  outline.contains(Point::new(x, y))
}
