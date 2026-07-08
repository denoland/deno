// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::f64::consts::PI;
use std::f64::consts::TAU;

use deno_core::GarbageCollected;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use vello::kurbo::Arc;
use vello::kurbo::BezPath;
use vello::kurbo::PathEl;
use vello::kurbo::Point;
use vello::kurbo::Vec2;

use crate::canvas2d::error::Canvas2DError;

pub struct Path2D {
  pub(super) path: RefCell<BezPath>,
}

/// `ToNumber(v)` guarded by a `TryCatch`: a user `valueOf` may throw, and an
/// unguarded `number_value` would leave that exception pending on the isolate.
fn to_number_guarded(
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

// SAFETY: Path2D is only accessed from the JS thread (same as context).
unsafe impl GarbageCollected for Path2D {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Path2D"
  }
}

#[op2]
impl Path2D {
  #[constructor]
  #[cppgc]
  fn new(
    scope: &mut v8::PinScope<'_, '_>,
    path: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<Path2D, Canvas2DError> {
    let bez = match path {
      Some(v) if v.is_string() => {
        let s = v.to_rust_string_lossy(scope);
        BezPath::from_svg(&s).unwrap_or_else(|_| BezPath::new())
      }
      Some(v) => {
        if let Some(p) = cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v) {
          p.path.borrow().clone()
        } else {
          BezPath::new()
        }
      }
      None => BezPath::new(),
    };
    Ok(Path2D {
      path: RefCell::new(bez),
    })
  }

  // CanvasPath methods (duplicated logic from context for Path2D; can be refactored later)
  #[fast]
  #[undefined]
  fn close_path(&self) {
    let mut path = self.path.borrow_mut();
    if !path.elements().is_empty() {
      path.close_path();
    }
  }

  #[undefined]
  fn move_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      self.path.borrow_mut().move_to((*x, *y));
    }
  }

  #[undefined]
  fn line_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      let mut path = self.path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*x, *y));
      } else {
        path.line_to((*x, *y));
      }
    }
  }

  #[undefined]
  fn bezier_curve_to(
    &self,
    #[webidl] cp1x: UnrestrictedDouble,
    #[webidl] cp1y: UnrestrictedDouble,
    #[webidl] cp2x: UnrestrictedDouble,
    #[webidl] cp2y: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if cp1x.is_finite()
      && cp1y.is_finite()
      && cp2x.is_finite()
      && cp2y.is_finite()
      && x.is_finite()
      && y.is_finite()
    {
      let mut path = self.path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*cp1x, *cp1y));
      }
      path.curve_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
    }
  }

  #[undefined]
  fn quadratic_curve_to(
    &self,
    #[webidl] cpx: UnrestrictedDouble,
    #[webidl] cpy: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if cpx.is_finite() && cpy.is_finite() && x.is_finite() && y.is_finite() {
      let mut path = self.path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*cpx, *cpy));
      }
      path.quad_to((*cpx, *cpy), (*x, *y));
    }
  }

  #[undefined]
  fn arc(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] end_angle: UnrestrictedDouble,
    counterclockwise: Option<bool>,
  ) -> Result<(), Canvas2DError> {
    let counterclockwise = counterclockwise.unwrap_or(false);
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
    if !x.is_finite()
      || !y.is_finite()
      || !radius.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
    }
    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);
    let mut path = self.path.borrow_mut();
    let arc = Arc {
      center: Point::new(*x, *y),
      radii: Vec2::new(*radius, *radius),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: 0.0,
    };
    let (sin_a, cos_a) = start_angle.sin_cos();
    let start_pt = arc.center + Vec2::new(*radius * cos_a, *radius * sin_a);
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

  #[undefined]
  fn arc_to(
    &self,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] x2: UnrestrictedDouble,
    #[webidl] y2: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
  ) -> Result<(), Canvas2DError> {
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
    if !x1.is_finite()
      || !y1.is_finite()
      || !x2.is_finite()
      || !y2.is_finite()
      || !radius.is_finite()
    {
      return Ok(());
    }
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
    }
    let mut path = self.path.borrow_mut();
    if path.is_empty() {
      path.move_to((*x1, *y1));
      return Ok(());
    }
    arc_to_impl(&mut path, *x1, *y1, *x2, *y2, *radius);
    Ok(())
  }

  #[undefined]
  fn ellipse(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] radius_x: UnrestrictedDouble,
    #[webidl] radius_y: UnrestrictedDouble,
    #[webidl] rotation: UnrestrictedDouble,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] end_angle: UnrestrictedDouble,
    counterclockwise: Option<bool>,
  ) -> Result<(), Canvas2DError> {
    let counterclockwise = counterclockwise.unwrap_or(false);
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
    if !x.is_finite()
      || !y.is_finite()
      || !radius_x.is_finite()
      || !radius_y.is_finite()
      || !rotation.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }
    if *radius_x < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_x));
    }
    if *radius_y < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_y));
    }
    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);
    let mut path = self.path.borrow_mut();
    let arc = Arc {
      center: Point::new(*x, *y),
      radii: Vec2::new(*radius_x, *radius_y),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: *rotation,
    };
    let (sin_a, cos_a) = start_angle.sin_cos();
    let dx = *radius_x * cos_a;
    let dy = *radius_y * sin_a;
    let (sin_r, cos_r) = rotation.sin_cos();
    let start_pt =
      Point::new(*x + dx * cos_r - dy * sin_r, *y + dx * sin_r + dy * cos_r);
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

  #[undefined]
  fn rect(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return;
    }
    let mut p = self.path.borrow_mut();
    p.move_to((*x, *y));
    p.line_to((*x + *w, *y));
    p.line_to((*x + *w, *y + *h));
    p.line_to((*x, *y + *h));
    p.close_path();
  }

  #[reentrant]
  #[undefined]
  fn round_rect(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
    radii: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return Ok(());
    }
    let radii_val = radii.unwrap_or_else(|| v8::undefined(scope).into());
    let corner_radii = parse_round_rect_radii(scope, radii_val)?;
    let mut path = self.path.borrow_mut();
    build_round_rect_path(&mut path, *x, *y, *w, *h, &corner_radii);
    Ok(())
  }

  #[fast]
  #[undefined]
  fn add_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    other: v8::Local<'_, v8::Value>,
  ) {
    if let Some(p) = cppgc::try_unwrap_cppgc_object::<Path2D>(scope, other) {
      let other_path = p.path.borrow();
      self.path.borrow_mut().extend(other_path.iter());
    }
  }
}

pub(super) fn compute_arc_sweep(
  start_angle: f64,
  end_angle: f64,
  counterclockwise: bool,
) -> f64 {
  // Mirrors the normalization algorithm in the HTML spec for arc()/ellipse():
  // for a clockwise sweep, endAngle is advanced until it is not less than
  // startAngle (sweep in [0, 2*PI)); for a counterclockwise sweep, endAngle
  // is retreated until it is not greater than startAngle (sweep in
  // (-2*PI, 0]). Either way, a request for a full lap in the sweep
  // direction (or more) is clamped to exactly one full turn, and
  // startAngle == endAngle always yields a zero-length sweep.
  //
  // A subtlety: if endAngle is congruent to startAngle mod 2*PI but was not
  // literally equal to it (e.g. startAngle=0, endAngle=2*PI), `rem_euclid`
  // normalizes the difference to exactly 0, which must be treated as a
  // full-circle sweep rather than a zero-length one -- only a *literal*
  // startAngle == endAngle (no wrapping needed) is a zero-length sweep.
  let diff = end_angle - start_angle;
  if diff == 0.0 {
    return 0.0;
  }
  if !counterclockwise {
    if diff >= TAU {
      return TAU;
    }
    let sweep = diff.rem_euclid(TAU);
    if sweep == 0.0 { TAU } else { sweep }
  } else {
    if -diff >= TAU {
      return -TAU;
    }
    let sweep = -(-diff).rem_euclid(TAU);
    if sweep == 0.0 { -TAU } else { sweep }
  }
}

pub(super) fn arc_to_impl(
  path: &mut BezPath,
  x1: f64,
  y1: f64,
  x2: f64,
  y2: f64,
  radius: f64,
) {
  let current = match path.elements().last() {
    Some(PathEl::MoveTo(p)) => *p,
    Some(PathEl::LineTo(p)) => *p,
    Some(PathEl::QuadTo(_, p)) => *p,
    Some(PathEl::CurveTo(_, _, p)) => *p,
    Some(PathEl::ClosePath) => return,
    None => return,
  };

  let p0 = current;
  let p1 = Point::new(x1, y1);
  let p2 = Point::new(x2, y2);

  if p0 == p1 || p1 == p2 || radius == 0.0 {
    path.line_to(p1);
    return;
  }

  let v0 = p0 - p1;
  let v1 = p2 - p1;

  let cross = v0.x * v1.y - v0.y * v1.x;
  if cross.abs() < 1e-10 {
    path.line_to(p1);
    return;
  }

  let d0 = v0.hypot();
  let d1 = v1.hypot();
  let u0 = Vec2::new(v0.x / d0, v0.y / d0);
  let u1 = Vec2::new(v1.x / d1, v1.y / d1);

  let cos_half = ((1.0 + u0.dot(u1)) / 2.0).sqrt();
  if cos_half == 0.0 {
    path.line_to(p1);
    return;
  }
  let d = radius / ((1.0 - cos_half * cos_half).sqrt() / cos_half);

  let t0 = Point::new(p1.x + u0.x * d, p1.y + u0.y * d);
  let t1 = Point::new(p1.x + u1.x * d, p1.y + u1.y * d);

  let cx_dir = Vec2::new(u0.x + u1.x, u0.y + u1.y);
  let cx_len = cx_dir.hypot();
  if cx_len == 0.0 {
    path.line_to(p1);
    return;
  }

  let center = Point::new(
    p1.x + cx_dir.x / cx_len * (d * d + radius * radius).sqrt(),
    p1.y + cx_dir.y / cx_len * (d * d + radius * radius).sqrt(),
  );

  let start_angle = (t0.y - center.y).atan2(t0.x - center.x);
  let end_angle = (t1.y - center.y).atan2(t1.x - center.x);

  // Unlike arc()/ellipse(), a corner fillet never needs to wrap more than
  // half a turn: the swept angle is always `PI` minus the angle between the
  // two tangent lines, i.e. strictly between 0 and PI in magnitude. So the
  // correct sweep is simply the shortest signed angular distance from
  // start_angle to end_angle, normalized to (-PI, PI] -- no need for
  // compute_arc_sweep's full-turn/direction-clamping semantics, which are
  // specific to the public arc()/ellipse() `counterclockwise` argument.
  let mut sweep = end_angle - start_angle;
  while sweep <= -PI {
    sweep += TAU;
  }
  while sweep > PI {
    sweep -= TAU;
  }

  path.line_to(t0);
  let arc = Arc {
    center,
    radii: Vec2::new(radius, radius),
    start_angle,
    sweep_angle: sweep,
    x_rotation: 0.0,
  };
  arc.to_cubic_beziers(0.1, |p1, p2, p3| {
    path.curve_to(p1, p2, p3);
  });
}

/// Per-corner radii for roundRect, each with (rx, ry).
pub(super) struct CornerRadii {
  top_left: (f64, f64),
  top_right: (f64, f64),
  bottom_right: (f64, f64),
  bottom_left: (f64, f64),
}

fn parse_single_radius(
  scope: &mut v8::PinScope<'_, '_>,
  val: v8::Local<'_, v8::Value>,
) -> Result<(f64, f64), Canvas2DError> {
  // WebIDL `(unrestricted double or DOMPointInit)`: `undefined` selects the
  // dictionary branch with all-default members.
  if val.is_undefined() {
    return Ok((0.0, 0.0));
  }
  if val.is_object() {
    let obj = val.cast::<v8::Object>();
    let rx = dom_point_init_member(scope, obj, "x")?;
    let ry = dom_point_init_member(scope, obj, "y")?;
    if !rx.is_finite() || !ry.is_finite() {
      return Err(Canvas2DError::NonFinite);
    }
    if rx < 0.0 || ry < 0.0 {
      return Err(Canvas2DError::NegativeRoundRectRadius);
    }
    Ok((rx, ry))
  } else {
    let n = to_number_guarded(scope, val)
      .ok_or(Canvas2DError::CannotConvertToNumber)?;
    if !n.is_finite() {
      return Err(Canvas2DError::NonFinite);
    }
    if n < 0.0 {
      return Err(Canvas2DError::NegativeRoundRectRadius);
    }
    Ok((n, n))
  }
}

/// Reads a `DOMPointInit` member: a missing or `undefined` member falls back
/// to its default (0), while a value that cannot be converted to a number
/// (e.g. a BigInt) throws a TypeError. Note this is distinct from
/// `Canvas2DError::NonFinite`, which per spec is silently ignored by
/// `roundRect()` rather than thrown.
fn dom_point_init_member(
  scope: &mut v8::PinScope<'_, '_>,
  obj: v8::Local<'_, v8::Object>,
  key: &str,
) -> Result<f64, Canvas2DError> {
  let key = v8::String::new(scope, key).unwrap();
  let value = obj
    .get(scope, key.into())
    .ok_or(Canvas2DError::CannotConvertToNumber)?;
  if value.is_undefined() {
    return Ok(0.0);
  }
  to_number_guarded(scope, value).ok_or(Canvas2DError::CannotConvertToNumber)
}

pub(super) fn parse_round_rect_radii(
  scope: &mut v8::PinScope<'_, '_>,
  val: v8::Local<'_, v8::Value>,
) -> Result<CornerRadii, Canvas2DError> {
  if val.is_undefined() {
    return Ok(CornerRadii {
      top_left: (0.0, 0.0),
      top_right: (0.0, 0.0),
      bottom_right: (0.0, 0.0),
      bottom_left: (0.0, 0.0),
    });
  }

  if val.is_array() {
    let arr: v8::Local<'_, v8::Array> = val.try_into().unwrap();
    let len = arr.length() as usize;
    if len == 0 || len > 4 {
      return Err(Canvas2DError::InvalidRadiiLength(len));
    }
    let mut radii = Vec::with_capacity(len);
    for i in 0..len {
      let elem = arr.get_index(scope, i as u32).unwrap();
      radii.push(parse_single_radius(scope, elem)?);
    }
    match len {
      1 => Ok(CornerRadii {
        top_left: radii[0],
        top_right: radii[0],
        bottom_right: radii[0],
        bottom_left: radii[0],
      }),
      2 => Ok(CornerRadii {
        top_left: radii[0],
        top_right: radii[1],
        bottom_right: radii[0],
        bottom_left: radii[1],
      }),
      3 => Ok(CornerRadii {
        top_left: radii[0],
        top_right: radii[1],
        bottom_right: radii[2],
        bottom_left: radii[1],
      }),
      4 => Ok(CornerRadii {
        top_left: radii[0],
        top_right: radii[1],
        bottom_right: radii[2],
        bottom_left: radii[3],
      }),
      _ => unreachable!(),
    }
  } else {
    let r = parse_single_radius(scope, val)?;
    Ok(CornerRadii {
      top_left: r,
      top_right: r,
      bottom_right: r,
      bottom_left: r,
    })
  }
}

/// Build a roundRect path per the spec algorithm.
/// https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-roundrect
pub(super) fn build_round_rect_path(
  path: &mut BezPath,
  x: f64,
  y: f64,
  w: f64,
  h: f64,
  radii: &CornerRadii,
) {
  let abs_w = w.abs();
  let abs_h = h.abs();

  let mut tl = radii.top_left;
  let mut tr = radii.top_right;
  let mut br = radii.bottom_right;
  let mut bl = radii.bottom_left;

  // If width is negative, swap left/right radii
  if w < 0.0 {
    std::mem::swap(&mut tl, &mut tr);
    std::mem::swap(&mut bl, &mut br);
  }
  // If height is negative, swap top/bottom radii
  if h < 0.0 {
    std::mem::swap(&mut tl, &mut bl);
    std::mem::swap(&mut tr, &mut br);
  }

  // Clamp radii: scale down if adjacent radii exceed dimension
  let top = tl.0 + tr.0;
  let right = tr.1 + br.1;
  let bottom = br.0 + bl.0;
  let left = bl.1 + tl.1;
  let mut scale = 1.0f64;
  if top > 0.0 {
    scale = scale.min(abs_w / top);
  }
  if right > 0.0 {
    scale = scale.min(abs_h / right);
  }
  if bottom > 0.0 {
    scale = scale.min(abs_w / bottom);
  }
  if left > 0.0 {
    scale = scale.min(abs_h / left);
  }
  if scale < 1.0 {
    tl = (tl.0 * scale, tl.1 * scale);
    tr = (tr.0 * scale, tr.1 * scale);
    br = (br.0 * scale, br.1 * scale);
    bl = (bl.0 * scale, bl.1 * scale);
  }

  let (cx, cy) = if w >= 0.0 && h >= 0.0 {
    (x, y)
  } else if w < 0.0 && h >= 0.0 {
    (x + w, y)
  } else if w >= 0.0 && h < 0.0 {
    (x, y + h)
  } else {
    (x + w, y + h)
  };

  let cw = abs_w;
  let ch = abs_h;

  // Per spec, the path winds clockwise when the signs of `w` and `h` agree
  // and counterclockwise when exactly one of them is negative, so that
  // opposite windings cancel out under the nonzero fill rule. Each corner
  // sweeps a full quarter-turn (90 degrees): angles are measured in
  // quarter-turns with 0 = +x axis, increasing towards +y.
  if (w < 0.0) == (h < 0.0) {
    // Clockwise: top-right from 270deg to 360deg, bottom-right from 0deg to
    // 90deg, bottom-left from 90deg to 180deg, top-left from 180deg to 270deg.
    path.move_to((cx + tl.0, cy));
    path.line_to((cx + cw - tr.0, cy));
    if tr.0 > 0.0 || tr.1 > 0.0 {
      add_elliptical_arc(path, cx + cw - tr.0, cy + tr.1, tr.0, tr.1, 3.0, 4.0);
    }
    path.line_to((cx + cw, cy + ch - br.1));
    if br.0 > 0.0 || br.1 > 0.0 {
      add_elliptical_arc(
        path,
        cx + cw - br.0,
        cy + ch - br.1,
        br.0,
        br.1,
        0.0,
        1.0,
      );
    }
    path.line_to((cx + bl.0, cy + ch));
    if bl.0 > 0.0 || bl.1 > 0.0 {
      add_elliptical_arc(path, cx + bl.0, cy + ch - bl.1, bl.0, bl.1, 1.0, 2.0);
    }
    path.line_to((cx, cy + tl.1));
    if tl.0 > 0.0 || tl.1 > 0.0 {
      add_elliptical_arc(path, cx + tl.0, cy + tl.1, tl.0, tl.1, 2.0, 3.0);
    }
  } else {
    // Counterclockwise: the same corners traversed in the opposite order,
    // each arc swept backwards.
    path.move_to((cx + tl.0, cy));
    if tl.0 > 0.0 || tl.1 > 0.0 {
      add_elliptical_arc(path, cx + tl.0, cy + tl.1, tl.0, tl.1, 3.0, 2.0);
    }
    path.line_to((cx, cy + ch - bl.1));
    if bl.0 > 0.0 || bl.1 > 0.0 {
      add_elliptical_arc(path, cx + bl.0, cy + ch - bl.1, bl.0, bl.1, 2.0, 1.0);
    }
    path.line_to((cx + cw - br.0, cy + ch));
    if br.0 > 0.0 || br.1 > 0.0 {
      add_elliptical_arc(
        path,
        cx + cw - br.0,
        cy + ch - br.1,
        br.0,
        br.1,
        1.0,
        0.0,
      );
    }
    path.line_to((cx + cw, cy + tr.1));
    if tr.0 > 0.0 || tr.1 > 0.0 {
      add_elliptical_arc(path, cx + cw - tr.0, cy + tr.1, tr.0, tr.1, 4.0, 3.0);
    }
  }
  path.close_path();
}

/// Add an elliptical arc from start_quarter to end_quarter (in quarter-turns from 3 o'clock).
fn add_elliptical_arc(
  path: &mut BezPath,
  cx: f64,
  cy: f64,
  rx: f64,
  ry: f64,
  start_quarter: f64,
  end_quarter: f64,
) {
  let start_angle = start_quarter * std::f64::consts::FRAC_PI_2;
  let sweep = (end_quarter - start_quarter) * std::f64::consts::FRAC_PI_2;
  let arc = Arc {
    center: Point::new(cx, cy),
    radii: Vec2::new(rx, ry),
    start_angle,
    sweep_angle: sweep,
    x_rotation: 0.0,
  };
  arc.to_cubic_beziers(0.1, |p1, p2, p3| {
    path.curve_to(p1, p2, p3);
  });
}
