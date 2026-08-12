// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::too_many_arguments, reason = "not code we control")]

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::ops::Index;
use std::ops::IndexMut;
use std::ptr;
use std::rc::Rc;

use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::SequenceLengthOneOf;
use deno_core::webidl::UnrestrictedDouble;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_core::webidl::try_convert_sequence_with_policy;

use crate::css_value::CSSValueError;
use crate::css_value::ParserInput;
use crate::css_value::Transform;
use crate::css_value::TransformListParser;
use crate::f64::maximum;
use crate::f64::minimum;

macro_rules! define_obj {
  ($scope:ident => { $( $modifier:ident $key:literal: $value:expr ),*, }) => {
    {
      let obj = v8::Object::new($scope);
      $(
        let key = v8::String::new($scope, $key).unwrap().into();
        let value = define_obj!(@modifier $modifier $scope => $value);
        obj.create_data_property($scope, key, value);
      )*
      obj
    }
  };
  (@modifier bool $scope:ident => $value:expr) => {
    v8::Boolean::new($scope, $value).into()
  };
  (@modifier num $scope:ident => $value:expr) => {
    v8::Number::new($scope, $value).into()
  };
  (@modifier raw $_scope:ident => $value:expr) => {
    $value.into()
  };
}

pub(crate) struct State {
  enable_css_parser_features: bool,
}

impl State {
  pub(crate) fn new(enable_css_parser_features: bool) -> Self {
    Self {
      enable_css_parser_features,
    }
  }
}

#[op2(fast)]
pub fn op_geometry_get_enable_css_parser_features(state: &OpState) -> bool {
  state.borrow::<State>().enable_css_parser_features
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum GeometryError {
  #[class(type)]
  #[error("Illegal invocation")]
  IllegalInvocation,
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
  #[class(type)]
  #[error("Inconsistent 2d matrix value")]
  Inconsistent2DMatrix,
  #[class(type)]
  #[error(
    "The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix"
  )]
  InvalidSequenceSize,
  #[class("DOMExceptionInvalidStateError")]
  #[error("Cannot be serialized with NaN or Infinity values")]
  InvalidState,
  #[class(type)]
  #[error("Cannot parse CSS <transform-list> on Workers")]
  DisallowWindowFeatures,
  #[class("DOMExceptionSyntaxError")]
  #[error("Failed to parse as CSS <transform-list>: {0}")]
  FailedToParse(String),
}

impl<'i> From<CSSValueError<'i>> for GeometryError {
  fn from(error: CSSValueError) -> Self {
    use cssparser::BasicParseErrorKind;
    use cssparser::ParseErrorKind;

    // Suppress Debug output for cssparser::Token
    let message: String = match error.kind {
      ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(_)) => {
        "unexpected token".into()
      }
      _ => format!("{}", error),
    };
    GeometryError::FailedToParse(message)
  }
}

#[derive(Clone, Copy)]
struct Vector4 {
  x: f64,
  y: f64,
  z: f64,
  w: f64,
}

impl Vector4 {
  #[inline]
  fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
    Self { x, y, z, w }
  }

  #[inline]
  fn zeros() -> Self {
    Self::new(0.0, 0.0, 0.0, 0.0)
  }
}

#[derive(Clone)]
struct Matrix4 {
  data: [f64; 16],
}

impl Matrix4 {
  #[inline]
  #[rustfmt::skip]
  fn new(
    m11: f64, m21: f64, m31: f64, m41: f64,
    m12: f64, m22: f64, m32: f64, m42: f64,
    m13: f64, m23: f64, m33: f64, m43: f64,
    m14: f64, m24: f64, m34: f64, m44: f64,
  ) -> Self {
    Self {
      data: [
        m11, m12, m13, m14,
        m21, m22, m23, m24,
        m31, m32, m33, m34,
        m41, m42, m43, m44,
      ],
    }
  }

  #[inline]
  fn from_column_slice(slice: &[f64; 16]) -> Self {
    Self { data: *slice }
  }

  #[inline]
  fn identity() -> Self {
    let mut matrix = Self { data: [0.0; 16] };
    matrix.fill_diagonal(1.0);
    matrix
  }

  #[inline]
  fn iter(&self) -> std::slice::Iter<'_, f64> {
    self.data.iter()
  }

  #[inline]
  fn as_slice(&self) -> &[f64] {
    &self.data
  }

  #[inline]
  fn fill(&mut self, value: f64) {
    self.data.fill(value);
  }

  #[inline]
  fn fill_diagonal(&mut self, value: f64) {
    self[(0, 0)] = value;
    self[(1, 1)] = value;
    self[(2, 2)] = value;
    self[(3, 3)] = value;
  }

  #[inline]
  fn prepend_translation_mut(&mut self, tx: f64, ty: f64, tz: f64) {
    let x = self[(0, 0)] * tx + self[(0, 1)] * ty + self[(0, 2)] * tz;
    let y = self[(1, 0)] * tx + self[(1, 1)] * ty + self[(1, 2)] * tz;
    let z = self[(2, 0)] * tx + self[(2, 1)] * ty + self[(2, 2)] * tz;
    let w = self[(3, 0)] * tx + self[(3, 1)] * ty + self[(3, 2)] * tz;
    self[(0, 3)] += x;
    self[(1, 3)] += y;
    self[(2, 3)] += z;
    self[(3, 3)] += w;
  }

  #[inline]
  fn prepend_nonuniform_scaling_mut(&mut self, sx: f64, sy: f64, sz: f64) {
    self.scale_column(0, sx);
    self.scale_column(1, sy);
    self.scale_column(2, sz);
  }

  #[inline]
  fn scale_column(&mut self, col: usize, scale: f64) {
    for row in 0..4 {
      self[(row, col)] *= scale;
    }
  }

  #[inline]
  fn neg_column(&mut self, col: usize) {
    self.scale_column(col, -1.0);
  }

  #[inline]
  fn multiply(lhs: &Self, rhs: &Self) -> Self {
    let mut out = Self { data: [0.0; 16] };
    for col in 0..4 {
      for row in 0..4 {
        out[(row, col)] = lhs[(row, 0)] * rhs[(0, col)]
          + lhs[(row, 1)] * rhs[(1, col)]
          + lhs[(row, 2)] * rhs[(2, col)]
          + lhs[(row, 3)] * rhs[(3, col)];
      }
    }
    out
  }

  #[inline]
  fn multiply_vector(&self, rhs: &Vector4) -> Vector4 {
    Vector4::new(
      self[(0, 0)] * rhs.x
        + self[(0, 1)] * rhs.y
        + self[(0, 2)] * rhs.z
        + self[(0, 3)] * rhs.w,
      self[(1, 0)] * rhs.x
        + self[(1, 1)] * rhs.y
        + self[(1, 2)] * rhs.z
        + self[(1, 3)] * rhs.w,
      self[(2, 0)] * rhs.x
        + self[(2, 1)] * rhs.y
        + self[(2, 2)] * rhs.z
        + self[(2, 3)] * rhs.w,
      self[(3, 0)] * rhs.x
        + self[(3, 1)] * rhs.y
        + self[(3, 2)] * rhs.z
        + self[(3, 3)] * rhs.w,
    )
  }

  #[inline]
  fn post_multiply_first3(&mut self, rhs: &Self) {
    let lhs = self.clone();
    for col in 0..3 {
      for row in 0..4 {
        self[(row, col)] = lhs[(row, 0)] * rhs[(0, col)]
          + lhs[(row, 1)] * rhs[(1, col)]
          + lhs[(row, 2)] * rhs[(2, col)]
          + lhs[(row, 3)] * rhs[(3, col)];
      }
    }
  }

  #[inline]
  fn post_skew_mut(&mut self, x: f64, y: f64) {
    let lhs = self.clone();
    let tan_x = x.tan();
    let tan_y = y.tan();
    for row in 0..4 {
      self[(row, 0)] = lhs[(row, 0)] + lhs[(row, 1)] * tan_y;
      self[(row, 1)] = lhs[(row, 0)] * tan_x + lhs[(row, 1)];
    }
  }

  #[inline]
  fn post_perspective_mut(&mut self, d: f64) {
    let lhs = self.clone();
    for row in 0..4 {
      self[(row, 2)] = lhs[(row, 2)] - lhs[(row, 3)] / d;
      self[(row, 3)] = lhs[(row, 3)];
    }
  }

  #[inline]
  fn try_inverse_mut(&mut self) -> bool {
    let m = self.data;

    let cofactor00 =
      m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
        + m[9] * m[7] * m[14]
        + m[13] * m[6] * m[11]
        - m[13] * m[7] * m[10];

    let cofactor01 =
      -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
        - m[8] * m[7] * m[14]
        - m[12] * m[6] * m[11]
        + m[12] * m[7] * m[10];

    let cofactor02 =
      m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
        + m[8] * m[7] * m[13]
        + m[12] * m[5] * m[11]
        - m[12] * m[7] * m[9];

    let cofactor03 =
      -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
        - m[8] * m[6] * m[13]
        - m[12] * m[5] * m[10]
        + m[12] * m[6] * m[9];

    let det = m[0] * cofactor00
      + m[1] * cofactor01
      + m[2] * cofactor02
      + m[3] * cofactor03;

    if det == 0.0 {
      return false;
    }

    self[(0, 0)] = cofactor00;

    self[(1, 0)] =
      -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
        - m[9] * m[3] * m[14]
        - m[13] * m[2] * m[11]
        + m[13] * m[3] * m[10];

    self[(2, 0)] =
      m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
        + m[5] * m[3] * m[14]
        + m[13] * m[2] * m[7]
        - m[13] * m[3] * m[6];

    self[(3, 0)] =
      -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
        - m[5] * m[3] * m[10]
        - m[9] * m[2] * m[7]
        + m[9] * m[3] * m[6];

    self[(0, 1)] = cofactor01;

    self[(1, 1)] =
      m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
        + m[8] * m[3] * m[14]
        + m[12] * m[2] * m[11]
        - m[12] * m[3] * m[10];

    self[(2, 1)] =
      -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
        - m[4] * m[3] * m[14]
        - m[12] * m[2] * m[7]
        + m[12] * m[3] * m[6];

    self[(3, 1)] =
      m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
        + m[4] * m[3] * m[10]
        + m[8] * m[2] * m[7]
        - m[8] * m[3] * m[6];

    self[(0, 2)] = cofactor02;

    self[(1, 2)] =
      -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
        - m[8] * m[3] * m[13]
        - m[12] * m[1] * m[11]
        + m[12] * m[3] * m[9];

    self[(2, 2)] =
      m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
        + m[4] * m[3] * m[13]
        + m[12] * m[1] * m[7]
        - m[12] * m[3] * m[5];

    self[(0, 3)] = cofactor03;

    self[(3, 2)] =
      -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
        - m[4] * m[3] * m[9]
        - m[8] * m[1] * m[7]
        + m[8] * m[3] * m[5];

    self[(1, 3)] =
      m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
        + m[8] * m[2] * m[13]
        + m[12] * m[1] * m[10]
        - m[12] * m[2] * m[9];

    self[(2, 3)] =
      -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
        - m[4] * m[2] * m[13]
        - m[12] * m[1] * m[6]
        + m[12] * m[2] * m[5];

    self[(3, 3)] =
      m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
        + m[4] * m[2] * m[9]
        + m[8] * m[1] * m[6]
        - m[8] * m[2] * m[5];

    let inv_det = 1.0 / det;
    for col in 0..4 {
      for row in 0..4 {
        self[(row, col)] *= inv_det;
      }
    }
    true
  }
}

impl Index<usize> for Matrix4 {
  type Output = f64;

  #[inline]
  fn index(&self, index: usize) -> &Self::Output {
    &self.data[index]
  }
}

impl IndexMut<usize> for Matrix4 {
  #[inline]
  fn index_mut(&mut self, index: usize) -> &mut Self::Output {
    &mut self.data[index]
  }
}

impl Index<(usize, usize)> for Matrix4 {
  type Output = f64;

  #[inline]
  fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
    &self.data[col * 4 + row]
  }
}

impl IndexMut<(usize, usize)> for Matrix4 {
  #[inline]
  fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Self::Output {
    &mut self.data[col * 4 + row]
  }
}

#[inline]
#[rustfmt::skip]
fn rotation_from_euler_angles(roll: f64, pitch: f64, yaw: f64) -> Matrix4 {
  let (sr, cr) = roll.sin_cos();
  let (sp, cp) = pitch.sin_cos();
  let (sy, cy) = yaw.sin_cos();
  Matrix4::new(
    cy * cp, cy * sp * sr - sy * cr, cy * sp * cr + sy * sr, 0.0,
    sy * cp, sy * sp * sr + cy * cr, sy * sp * cr - cy * sr, 0.0,
        -sp,                         cp * sr,                         cp * cr, 0.0,
        0.0,                             0.0,                             0.0, 1.0,
  )
}

#[inline]
#[rustfmt::skip]
fn rotation_from_axis_angle(x: f64, y: f64, z: f64, angle: f64) -> Matrix4 {
  if angle == 0.0 {
    return Matrix4::identity();
  }

  let length = (x * x + y * y + z * z).sqrt();
  let inv_length = 1.0 / length;
  let x = x * inv_length;
  let y = y * inv_length;
  let z = z * inv_length;
  let (sin, cos) = angle.sin_cos();
  let one_m_cos = 1.0 - cos;
  Matrix4::new(
    x * x + (1.0 - x * x) * cos,
    x * y * one_m_cos - z * sin,
    x * z * one_m_cos + y * sin,
    0.0,
    x * y * one_m_cos + z * sin,
    y * y + (1.0 - y * y) * cos,
    y * z * one_m_cos - x * sin,
    0.0,
    x * z * one_m_cos - y * sin,
    y * z * one_m_cos + x * sin,
    z * z + (1.0 - z * z) * cos,
    0.0,
    0.0,
    0.0,
    0.0,
    1.0,
  )
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMPointInit {
  #[webidl(default = UnrestrictedDouble(0.0))]
  x: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  y: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  z: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(1.0))]
  w: UnrestrictedDouble,
}

#[derive(CppgcBase)]
#[repr(C)]
pub struct DOMPointReadOnly {
  inner: RefCell<Vector4>,
}

// SAFETY: we're sure `DOMPointReadOnly` can be GCed
unsafe impl GarbageCollected for DOMPointReadOnly {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMPointReadOnly"
  }
}

impl DOMPointReadOnly {
  #[inline]
  fn from_point_inner(init: DOMPointInit) -> DOMPointReadOnly {
    DOMPointReadOnly {
      inner: RefCell::new(Vector4::new(*init.x, *init.y, *init.z, *init.w)),
    }
  }
}

#[op2(base)]
impl DOMPointReadOnly {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] z: Option<UnrestrictedDouble>,
    #[webidl] w: Option<UnrestrictedDouble>,
  ) -> DOMPointReadOnly {
    DOMPointReadOnly {
      inner: RefCell::new(Vector4::new(
        *x.unwrap_or(UnrestrictedDouble(0.0)),
        *y.unwrap_or(UnrestrictedDouble(0.0)),
        *z.unwrap_or(UnrestrictedDouble(0.0)),
        *w.unwrap_or(UnrestrictedDouble(1.0)),
      )),
    }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  #[cppgc]
  fn from_point(#[webidl] init: DOMPointInit) -> DOMPointReadOnly {
    DOMPointReadOnly::from_point_inner(init)
  }

  #[fast]
  #[getter]
  fn x(&self) -> f64 {
    self.inner.borrow().x
  }

  #[fast]
  #[getter]
  fn y(&self) -> f64 {
    self.inner.borrow().y
  }

  #[fast]
  #[getter]
  fn z(&self) -> f64 {
    self.inner.borrow().z
  }

  #[fast]
  #[getter]
  fn w(&self) -> f64 {
    self.inner.borrow().w
  }

  #[rename("toJSON")]
  #[required(0)]
  fn to_json<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    define_obj!(scope => {
      num "x": self.inner.borrow().x,
      num "y": self.inner.borrow().y,
      num "z": self.inner.borrow().z,
      num "w": self.inner.borrow().w,
    })
  }

  #[reentrant]
  #[required(0)]
  fn matrix_transform<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] value: DOMMatrixInit,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let matrix = DOMMatrixReadOnly::from_matrix_inner(&value)?;
    let ro = DOMPointReadOnly {
      inner: RefCell::new(Vector4::zeros()),
    };
    matrix_transform_point(&matrix, self, &ro);
    let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMPoint { base: ro }))
  }
}

#[derive(CppgcInherits, CppgcBase)]
#[cppgc_inherits_from(DOMPointReadOnly)]
#[repr(C)]
pub struct DOMPoint {
  base: DOMPointReadOnly,
}

// SAFETY: we're sure `DOMPoint` can be GCed
unsafe impl GarbageCollected for DOMPoint {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMPoint"
  }
}

#[op2(base, inherit = DOMPointReadOnly)]
impl DOMPoint {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] z: Option<UnrestrictedDouble>,
    #[webidl] w: Option<UnrestrictedDouble>,
  ) -> DOMPoint {
    let ro = DOMPointReadOnly {
      inner: RefCell::new(Vector4::new(
        *x.unwrap_or(UnrestrictedDouble(0.0)),
        *y.unwrap_or(UnrestrictedDouble(0.0)),
        *z.unwrap_or(UnrestrictedDouble(0.0)),
        *w.unwrap_or(UnrestrictedDouble(1.0)),
      )),
    };
    DOMPoint { base: ro }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_point<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] init: DOMPointInit,
  ) -> v8::Local<'a, v8::Object> {
    let ro = DOMPointReadOnly::from_point_inner(init);
    let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
    cppgc::wrap_object(scope, obj, DOMPoint { base: ro })
  }

  #[fast]
  #[getter]
  fn x(&self) -> f64 {
    self.base.inner.borrow().x
  }

  #[setter]
  fn x(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut().x = *value
  }

  #[fast]
  #[getter]
  fn y(&self) -> f64 {
    self.base.inner.borrow().y
  }

  #[setter]
  fn y(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut().y = *value
  }

  #[fast]
  #[getter]
  fn z(&self) -> f64 {
    self.base.inner.borrow().z
  }

  #[setter]
  fn z(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut().z = *value
  }

  #[fast]
  #[getter]
  fn w(&self) -> f64 {
    self.base.inner.borrow().w
  }

  #[setter]
  fn w(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut().w = *value
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMRectInit {
  #[webidl(default = UnrestrictedDouble(0.0))]
  x: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  y: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  width: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  height: UnrestrictedDouble,
}

#[derive(CppgcBase)]
#[repr(C)]
pub struct DOMRectReadOnly {
  x: Cell<f64>,
  y: Cell<f64>,
  width: Cell<f64>,
  height: Cell<f64>,
}

// SAFETY: we're sure `DOMRectReadOnly` can be GCed
unsafe impl GarbageCollected for DOMRectReadOnly {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMRectReadOnly"
  }
}

impl DOMRectReadOnly {
  #[inline]
  fn from_rect_inner(init: DOMRectInit) -> DOMRectReadOnly {
    DOMRectReadOnly {
      x: Cell::new(*init.x),
      y: Cell::new(*init.y),
      width: Cell::new(*init.width),
      height: Cell::new(*init.height),
    }
  }

  #[inline]
  fn get_top(&self) -> f64 {
    let y = self.y.get();
    let height = self.height.get();
    minimum(y, y + height)
  }

  #[inline]
  fn get_right(&self) -> f64 {
    let x = self.x.get();
    let width = self.width.get();
    maximum(x, x + width)
  }

  #[inline]
  fn get_bottom(&self) -> f64 {
    let y = self.y.get();
    let height = self.height.get();
    maximum(y, y + height)
  }

  #[inline]
  fn get_left(&self) -> f64 {
    let x = self.x.get();
    let width = self.width.get();
    minimum(x, x + width)
  }
}

#[op2(base)]
impl DOMRectReadOnly {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] width: Option<UnrestrictedDouble>,
    #[webidl] height: Option<UnrestrictedDouble>,
  ) -> DOMRectReadOnly {
    DOMRectReadOnly {
      x: Cell::new(*x.unwrap_or(UnrestrictedDouble(0.0))),
      y: Cell::new(*y.unwrap_or(UnrestrictedDouble(0.0))),
      width: Cell::new(*width.unwrap_or(UnrestrictedDouble(0.0))),
      height: Cell::new(*height.unwrap_or(UnrestrictedDouble(0.0))),
    }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  #[cppgc]
  fn from_rect(#[webidl] init: DOMRectInit) -> DOMRectReadOnly {
    DOMRectReadOnly::from_rect_inner(init)
  }

  #[fast]
  #[getter]
  fn x(&self) -> f64 {
    self.x.get()
  }

  #[fast]
  #[getter]
  fn y(&self) -> f64 {
    self.y.get()
  }

  #[fast]
  #[getter]
  fn width(&self) -> f64 {
    self.width.get()
  }

  #[fast]
  #[getter]
  fn height(&self) -> f64 {
    self.height.get()
  }

  #[fast]
  #[getter]
  fn top(&self) -> f64 {
    self.get_top()
  }

  #[fast]
  #[getter]
  fn right(&self) -> f64 {
    self.get_right()
  }

  #[fast]
  #[getter]
  fn bottom(&self) -> f64 {
    self.get_bottom()
  }

  #[fast]
  #[getter]
  fn left(&self) -> f64 {
    self.get_left()
  }

  #[rename("toJSON")]
  #[required(0)]
  fn to_json<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    define_obj!(scope => {
      num "x": self.x.get(),
      num "y": self.y.get(),
      num "width": self.width.get(),
      num "height": self.height.get(),
      num "top": self.get_top(),
      num "right": self.get_right(),
      num "bottom": self.get_bottom(),
      num "left": self.get_left(),
    })
  }
}

#[derive(CppgcInherits, CppgcBase)]
#[cppgc_inherits_from(DOMRectReadOnly)]
#[repr(C)]
pub struct DOMRect {
  base: DOMRectReadOnly,
}

// SAFETY: we're sure `DOMRect` can be GCed
unsafe impl GarbageCollected for DOMRect {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMRect"
  }
}

#[op2(base, inherit = DOMRectReadOnly)]
impl DOMRect {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] width: Option<UnrestrictedDouble>,
    #[webidl] height: Option<UnrestrictedDouble>,
  ) -> DOMRect {
    let ro = DOMRectReadOnly {
      x: Cell::new(*x.unwrap_or(UnrestrictedDouble(0.0))),
      y: Cell::new(*y.unwrap_or(UnrestrictedDouble(0.0))),
      width: Cell::new(*width.unwrap_or(UnrestrictedDouble(0.0))),
      height: Cell::new(*height.unwrap_or(UnrestrictedDouble(0.0))),
    };
    DOMRect { base: ro }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_rect<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] init: DOMRectInit,
  ) -> v8::Local<'a, v8::Object> {
    let ro = DOMRectReadOnly::from_rect_inner(init);
    let obj = cppgc::make_cppgc_empty_object::<DOMRect>(scope);
    cppgc::wrap_object(scope, obj, DOMRect { base: ro })
  }

  #[fast]
  #[getter]
  fn x(&self) -> f64 {
    self.base.x.get()
  }

  #[setter]
  fn x(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.x.set(*value)
  }

  #[fast]
  #[getter]
  fn y(&self) -> f64 {
    self.base.y.get()
  }

  #[setter]
  fn y(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.y.set(*value)
  }

  #[fast]
  #[getter]
  fn width(&self) -> f64 {
    self.base.width.get()
  }

  #[setter]
  fn width(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.width.set(*value)
  }

  #[fast]
  #[getter]
  fn height(&self) -> f64 {
    self.base.height.get()
  }

  #[setter]
  fn height(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.height.set(*value)
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMQuadInit {
  p1: DOMPointInit,
  p2: DOMPointInit,
  p3: DOMPointInit,
  p4: DOMPointInit,
}

pub struct DOMQuad {
  p1: v8::TracedReference<v8::Object>,
  p2: v8::TracedReference<v8::Object>,
  p3: v8::TracedReference<v8::Object>,
  p4: v8::TracedReference<v8::Object>,
}

// SAFETY: we're sure `DOMQuad` can be GCed
unsafe impl GarbageCollected for DOMQuad {
  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    visitor.trace(&self.p1);
    visitor.trace(&self.p2);
    visitor.trace(&self.p3);
    visitor.trace(&self.p4);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMQuad"
  }
}

#[op2]
impl DOMQuad {
  #[constructor]
  #[reentrant]
  #[required(0)]
  #[cppgc]
  fn constructor(
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] p1: DOMPointInit,
    #[webidl] p2: DOMPointInit,
    #[webidl] p3: DOMPointInit,
    #[webidl] p4: DOMPointInit,
  ) -> DOMQuad {
    #[inline]
    fn from_point(
      scope: &mut v8::PinScope<'_, '_>,
      point: DOMPointInit,
    ) -> v8::TracedReference<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object(scope, obj, DOMPoint { base: ro });
      v8::TracedReference::new(scope, obj)
    }

    DOMQuad {
      p1: from_point(scope, p1),
      p2: from_point(scope, p2),
      p3: from_point(scope, p3),
      p4: from_point(scope, p4),
    }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  #[cppgc]
  fn from_rect(
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] rect: DOMRectInit,
  ) -> DOMQuad {
    #[inline]
    fn create_point(
      scope: &mut v8::PinScope<'_, '_>,
      x: f64,
      y: f64,
      z: f64,
      w: f64,
    ) -> v8::TracedReference<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(x, y, z, w)),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object(scope, obj, DOMPoint { base: ro });
      v8::TracedReference::new(scope, obj)
    }

    let DOMRectInit {
      x,
      y,
      width,
      height,
    } = rect;
    DOMQuad {
      p1: create_point(scope, *x, *y, 0.0, 1.0),
      p2: create_point(scope, *x + *width, *y, 0.0, 1.0),
      p3: create_point(scope, *x + *width, *y + *height, 0.0, 1.0),
      p4: create_point(scope, *x, *y + *height, 0.0, 1.0),
    }
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  #[cppgc]
  fn from_quad(
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] quad: DOMQuadInit,
  ) -> DOMQuad {
    #[inline]
    fn from_point(
      scope: &mut v8::PinScope<'_, '_>,
      point: DOMPointInit,
    ) -> v8::TracedReference<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object(scope, obj, DOMPoint { base: ro });
      v8::TracedReference::new(scope, obj)
    }

    DOMQuad {
      p1: from_point(scope, quad.p1),
      p2: from_point(scope, quad.p2),
      p3: from_point(scope, quad.p3),
      p4: from_point(scope, quad.p4),
    }
  }

  #[getter]
  fn p1<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.p1.get(scope).unwrap()
  }

  #[getter]
  fn p2<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.p2.get(scope).unwrap()
  }

  #[getter]
  fn p3<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.p3.get(scope).unwrap()
  }

  #[getter]
  fn p4<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.p4.get(scope).unwrap()
  }

  #[required(0)]
  fn get_bounds<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    #[inline]
    fn get_ptr(
      scope: &mut v8::PinScope<'_, '_>,
      value: &v8::TracedReference<v8::Object>,
    ) -> cppgc::UnsafePtr<DOMPointReadOnly> {
      let value = value.get(scope).unwrap();
      cppgc::try_unwrap_cppgc_base_object::<DOMPointReadOnly>(
        scope,
        value.into(),
      )
      .unwrap()
    }

    let p1 = get_ptr(scope, &self.p1);
    let p2 = get_ptr(scope, &self.p2);
    let p3 = get_ptr(scope, &self.p3);
    let p4 = get_ptr(scope, &self.p4);
    let p1 = *p1.inner.borrow();
    let p2 = *p2.inner.borrow();
    let p3 = *p3.inner.borrow();
    let p4 = *p4.inner.borrow();
    let left = minimum(minimum(p1.x, p2.x), minimum(p3.x, p4.x));
    let top = minimum(minimum(p1.y, p2.y), minimum(p3.y, p4.y));
    let right = maximum(maximum(p1.x, p2.x), maximum(p3.x, p4.x));
    let bottom = maximum(maximum(p1.y, p2.y), maximum(p3.y, p4.y));
    let ro = DOMRectReadOnly {
      x: Cell::new(left),
      y: Cell::new(top),
      width: Cell::new(right - left),
      height: Cell::new(bottom - top),
    };
    let obj = cppgc::make_cppgc_empty_object::<DOMRect>(scope);
    cppgc::wrap_object(scope, obj, DOMRect { base: ro })
  }

  #[rename("toJSON")]
  #[required(0)]
  fn to_json<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    define_obj!(scope => {
      raw "p1": self.p1.get(scope).unwrap(),
      raw "p2": self.p2.get(scope).unwrap(),
      raw "p3": self.p3.get(scope).unwrap(),
      raw "p4": self.p4.get(scope).unwrap(),
    })
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMMatrixInit {
  // Need to place the inherited DOMMatrixInit2D first
  #[webidl(default = None)]
  a: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  b: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  c: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  d: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  e: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  f: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m11: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m12: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m21: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m22: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m41: Option<UnrestrictedDouble>,
  #[webidl(default = None)]
  m42: Option<UnrestrictedDouble>,

  #[webidl(default = UnrestrictedDouble(0.0))]
  m13: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m14: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m23: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m24: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m31: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m32: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(1.0))]
  m33: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m34: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(0.0))]
  m43: UnrestrictedDouble,
  #[webidl(default = UnrestrictedDouble(1.0))]
  m44: UnrestrictedDouble,
  #[webidl(default = None)]
  is_2d: Option<bool>,
}

#[derive(CppgcBase, Clone)]
#[repr(C)]
pub struct DOMMatrixReadOnly {
  inner: RefCell<Matrix4>,
  is_2d: Cell<bool>,
}

// SAFETY: we're sure `DOMMatrixReadOnly` can be GCed
unsafe impl GarbageCollected for DOMMatrixReadOnly {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMMatrixReadOnly"
  }
}

/*
 * NOTE: column-major order
 *
 * For a 2D 2x3 matrix, the index of properties in
 * | a c 0 e |    | 0 4 _ 12 |
 * | b d 0 f |    | 1 5 _ 13 |
 * | 0 0 1 0 | is | _ _ _  _ |
 * | 0 0 0 1 |    | _ _ _  _ |
 */
const INDEX_A: usize = 0;
const INDEX_B: usize = 1;
const INDEX_C: usize = 4;
const INDEX_D: usize = 5;
const INDEX_E: usize = 12;
const INDEX_F: usize = 13;

/*
 * NOTE: column-major order
 *
 * The index of properties in
 * | m11 m21 m31 m41 |    | 0 4  8 12 |
 * | m12 m22 m32 m42 |    | 1 5  9 13 |
 * | m13 m23 m33 m43 | is | 2 6 10 14 |
 * | m14 m24 m34 m44 |    | 3 7 11 15 |
 */
const INDEX_M11: usize = 0;
const INDEX_M12: usize = 1;
const INDEX_M13: usize = 2;
const INDEX_M14: usize = 3;
const INDEX_M21: usize = 4;
const INDEX_M22: usize = 5;
const INDEX_M23: usize = 6;
const INDEX_M24: usize = 7;
const INDEX_M31: usize = 8;
const INDEX_M32: usize = 9;
const INDEX_M33: usize = 10;
const INDEX_M34: usize = 11;
const INDEX_M41: usize = 12;
const INDEX_M42: usize = 13;
const INDEX_M43: usize = 14;
const INDEX_M44: usize = 15;

trait ToF64 {
  fn to_f64(&self) -> f64;
}

impl ToF64 for UnrestrictedDouble {
  fn to_f64(&self) -> f64 {
    **self
  }
}

impl ToF64 for f32 {
  fn to_f64(&self) -> f64 {
    *self as f64
  }
}

impl ToF64 for f64 {
  fn to_f64(&self) -> f64 {
    *self
  }
}

impl DOMMatrixReadOnly {
  fn new<'a>(
    state: Rc<RefCell<OpState>>,
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'_>,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    // omitted (undefined)
    if value.is_undefined() {
      return Ok(DOMMatrixReadOnly::identity());
    }

    // sequence
    if value.is_object()
      && let Some(seq) = try_convert_sequence_with_policy::<
        UnrestrictedDouble,
        SequenceLengthOneOf<6, 16>,
        16,
      >(
        scope, value, prefix, context, &Default::default()
      )
      .map_err(|err| {
        if matches!(&err.kind, WebIdlErrorKind::InvalidSequenceLength { .. }) {
          GeometryError::InvalidSequenceSize
        } else {
          GeometryError::from(err)
        }
      })?
    {
      return DOMMatrixReadOnly::from_sequence_inner(&seq);
    }

    // DOMString
    if let Some(value) = value.to_string(scope) {
      if !state.borrow().borrow::<State>().enable_css_parser_features {
        return Err(GeometryError::DisallowWindowFeatures);
      }

      let matrix = DOMMatrixReadOnly::identity();
      let string = value.to_rust_string_lossy(scope);
      if !string.is_empty() {
        let mut input = ParserInput::new(&string);
        for result in TransformListParser::new(&mut input) {
          let transform = result?;
          matrix.exec_css_transform(&transform)?;
        }
      }
      return Ok(matrix);
    }

    Ok(DOMMatrixReadOnly::identity())
  }

  fn from_matrix_inner(
    init: &DOMMatrixInit,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    macro_rules! fixup {
      ($value3d:expr, $value2d:expr, $default:expr) => {{
        if let Some(value3d) = $value3d {
          if let Some(value2d) = $value2d {
            if !(*value3d == *value2d || value3d.is_nan() && value2d.is_nan()) {
              return Err(GeometryError::Inconsistent2DMatrix);
            }
          }
          value3d
        } else if let Some(value2d) = $value2d {
          value2d
        } else {
          UnrestrictedDouble($default)
        }
      }};
    }

    let m11 = fixup!(init.m11, init.a, 1.0);
    let m12 = fixup!(init.m12, init.b, 0.0);
    let m21 = fixup!(init.m21, init.c, 0.0);
    let m22 = fixup!(init.m22, init.d, 1.0);
    let m41 = fixup!(init.m41, init.e, 0.0);
    let m42 = fixup!(init.m42, init.f, 0.0);
    let is_2d = {
      let is_2d_can_be_true = *init.m13 == 0.0
        && *init.m14 == 0.0
        && *init.m23 == 0.0
        && *init.m24 == 0.0
        && *init.m31 == 0.0
        && *init.m32 == 0.0
        && *init.m33 == 1.0
        && *init.m34 == 0.0
        && *init.m43 == 0.0
        && *init.m44 == 1.0;
      if let Some(is_2d) = init.is_2d {
        if is_2d && !is_2d_can_be_true {
          return Err(GeometryError::Inconsistent2DMatrix);
        } else {
          is_2d
        }
      } else {
        is_2d_can_be_true
      }
    };

    if is_2d {
      Ok(DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
          *m11, *m21, 0.0, *m41,
          *m12, *m22, 0.0, *m42,
           0.0,  0.0, 1.0,  0.0,
           0.0,  0.0, 0.0,  1.0,
        )),
        is_2d: Cell::new(true),
      })
    } else {
      let DOMMatrixInit {
        m13,
        m14,
        m23,
        m24,
        m31,
        m32,
        m33,
        m34,
        m43,
        m44,
        ..
      } = init;
      Ok(DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
           *m11,  *m21, **m31,  *m41,
           *m12,  *m22, **m32,  *m42,
          **m13, **m23, **m33, **m43,
          **m14, **m24, **m34, **m44,
        )),
        is_2d: Cell::new(false),
      })
    }
  }

  fn from_sequence_inner<T: ToF64>(
    seq: &[T],
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    if let [a, b, c, d, e, f] = seq {
      Ok(DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
          a.to_f64(), c.to_f64(), 0.0, e.to_f64(),
          b.to_f64(), d.to_f64(), 0.0, f.to_f64(),
                 0.0,        0.0, 1.0,        0.0,
                 0.0,        0.0, 0.0,        1.0,
        )),
        is_2d: Cell::new(true),
      })
    } else if seq.len() == 16 {
      let seq: [f64; 16] = std::array::from_fn(|i| seq[i].to_f64());
      Ok(DOMMatrixReadOnly {
        inner: RefCell::new(Matrix4::from_column_slice(&seq)),
        is_2d: Cell::new(false),
      })
    } else {
      Err(GeometryError::InvalidSequenceSize)
    }
  }

  #[inline]
  fn identity() -> DOMMatrixReadOnly {
    DOMMatrixReadOnly {
      inner: RefCell::new(Matrix4::identity()),
      is_2d: Cell::new(true),
    }
  }

  #[inline]
  fn translate_self_inner(&self, tx: f64, ty: f64, tz: f64) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    inner.prepend_translation_mut(tx, ty, tz);
    self.is_2d.set(is_2d && tz == 0.0);
  }

  #[inline]
  fn scale_without_origin_self_inner(&self, sx: f64, sy: f64, sz: f64) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    inner.prepend_nonuniform_scaling_mut(sx, sy, sz);
    self.is_2d.set(is_2d && sz == 1.0);
  }

  #[inline]
  fn scale_with_origin_self_inner(
    &self,
    sx: f64,
    sy: f64,
    sz: f64,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
  ) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    inner.prepend_translation_mut(origin_x, origin_y, origin_z);
    inner.prepend_nonuniform_scaling_mut(sx, sy, sz);
    inner.prepend_translation_mut(-origin_x, -origin_y, -origin_z);
    self.is_2d.set(is_2d && sz == 1.0 && origin_z == 0.0);
  }

  #[inline]
  fn rotate_self_inner(&self, roll: f64, pitch: f64, yaw: f64) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    let rotation = rotation_from_euler_angles(roll, pitch, yaw);
    inner.post_multiply_first3(&rotation);
    self.is_2d.set(is_2d && roll == 0.0 && pitch == 0.0);
  }

  #[inline]
  fn rotate_from_vector_self_inner(&self, x: f64, y: f64) {
    if x == 0.0 && y == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    let rotation = rotation_from_axis_angle(0.0, 0.0, 1.0, y.atan2(x));
    inner.post_multiply_first3(&rotation);
  }

  #[inline]
  fn rotate_axis_angle_self_inner(&self, x: f64, y: f64, z: f64, angle: f64) {
    if x == 0.0 && y == 0.0 && z == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    let rotation = rotation_from_axis_angle(x, y, z, angle);
    inner.post_multiply_first3(&rotation);
    self.is_2d.set(is_2d && x == 0.0 && y == 0.0);
  }

  #[inline]
  fn skew_self_inner(&self, x: f64, y: f64) {
    let mut inner = self.inner.borrow_mut();
    inner.post_skew_mut(x, y);
  }

  #[inline]
  fn perspective_self_inner(&self, d: f64) {
    if d == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    inner.post_perspective_mut(d);
    self.is_2d.set(false);
  }

  #[inline]
  fn multiply_self_inner(
    &self,
    lhs: &DOMMatrixReadOnly,
    rhs: &DOMMatrixReadOnly,
  ) {
    let lhs_inner = lhs.inner.borrow();
    let lhs_is_2d = lhs.is_2d.get();
    let rhs_inner = rhs.inner.borrow();
    let rhs_is_2d = rhs.is_2d.get();
    let mut out_inner = self.inner.borrow_mut();
    *out_inner = Matrix4::multiply(&lhs_inner, &rhs_inner);
    self.is_2d.set(lhs_is_2d && rhs_is_2d);
  }

  #[inline]
  fn flip_x_inner(&self) {
    let mut inner = self.inner.borrow_mut();
    inner.neg_column(0);
  }

  #[inline]
  fn flip_y_inner(&self) {
    let mut inner = self.inner.borrow_mut();
    inner.neg_column(1);
  }

  #[inline]
  fn invert_self_inner(&self) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    if inner.iter().any(|&x| x.is_infinite()) {
      inner.fill(f64::NAN);
      self.is_2d.set(false);
      return;
    }
    if is_2d {
      let a = inner[INDEX_A];
      let b = inner[INDEX_B];
      let c = inner[INDEX_C];
      let d = inner[INDEX_D];
      let e = inner[INDEX_E];
      let f = inner[INDEX_F];
      let determinant = a * d - b * c;
      if determinant == 0.0 {
        inner.fill(f64::NAN);
        self.is_2d.set(false);
        return;
      }
      let inv_det = 1.0 / determinant;
      inner[INDEX_A] = d * inv_det;
      inner[INDEX_B] = -b * inv_det;
      inner[INDEX_C] = -c * inv_det;
      inner[INDEX_D] = a * inv_det;
      inner[INDEX_E] = (c * f - d * e) * inv_det;
      inner[INDEX_F] = (b * e - a * f) * inv_det;
    } else if !inner.try_inverse_mut() {
      inner.fill(f64::NAN);
    }
  }

  #[inline]
  fn a_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_A]
  }

  #[inline]
  fn b_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_B]
  }

  #[inline]
  fn c_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_C]
  }

  #[inline]
  fn d_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_D]
  }

  #[inline]
  fn e_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_E]
  }

  #[inline]
  fn f_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_F]
  }

  #[inline]
  fn m11_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M11]
  }

  #[inline]
  fn m12_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M12]
  }

  #[inline]
  fn m13_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M13]
  }

  #[inline]
  fn m14_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M14]
  }

  #[inline]
  fn m21_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M21]
  }

  #[inline]
  fn m22_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M22]
  }

  #[inline]
  fn m23_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M23]
  }

  #[inline]
  fn m24_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M24]
  }

  #[inline]
  fn m31_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M31]
  }

  #[inline]
  fn m32_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M32]
  }

  #[inline]
  fn m33_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M33]
  }

  #[inline]
  fn m34_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M34]
  }

  #[inline]
  fn m41_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M41]
  }

  #[inline]
  fn m42_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M42]
  }

  #[inline]
  fn m43_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M43]
  }

  #[inline]
  fn m44_inner(&self) -> f64 {
    self.inner.borrow()[INDEX_M44]
  }

  #[inline]
  fn is_identity_inner(&self) -> bool {
    let inner = self.inner.borrow();
    inner[INDEX_M11] == 1.0
      && inner[INDEX_M12] == 0.0
      && inner[INDEX_M13] == 0.0
      && inner[INDEX_M14] == 0.0
      && inner[INDEX_M21] == 0.0
      && inner[INDEX_M22] == 1.0
      && inner[INDEX_M23] == 0.0
      && inner[INDEX_M24] == 0.0
      && inner[INDEX_M31] == 0.0
      && inner[INDEX_M32] == 0.0
      && inner[INDEX_M33] == 1.0
      && inner[INDEX_M34] == 0.0
      && inner[INDEX_M41] == 0.0
      && inner[INDEX_M42] == 0.0
      && inner[INDEX_M43] == 0.0
      && inner[INDEX_M44] == 1.0
  }

  #[inline]
  fn is_finite_inner(&self) -> bool {
    self.inner.borrow().iter().all(|&item| item.is_finite())
  }

  fn exec_css_transform(
    &self,
    transform: &Transform,
  ) -> Result<(), GeometryError> {
    match transform {
      Transform::Translate(x, y) => {
        let x = x.to_pixels();
        let y = if let Some(y) = y { y.to_pixels() } else { 0.0 };
        self.translate_self_inner(x, y, 0.0);
      }
      Transform::TranslateX(x) => {
        let x = x.to_pixels();
        self.translate_self_inner(x, 0.0, 0.0);
      }
      Transform::TranslateY(y) => {
        let y = y.to_pixels();
        self.translate_self_inner(0.0, y, 0.0);
      }
      Transform::TranslateZ(z) => {
        let z = z.to_pixels();
        self.translate_self_inner(0.0, 0.0, z);
      }
      Transform::Translate3d(x, y, z) => {
        let x = x.to_pixels();
        let y = y.to_pixels();
        let z = z.to_pixels();
        self.translate_self_inner(x, y, z);
        self.is_2d.set(false);
      }
      Transform::Scale(x, y) => {
        let x = *x;
        let y = if let Some(y) = y { *y } else { x };
        self.scale_without_origin_self_inner(x, y, 1.0);
      }
      Transform::ScaleX(x) => {
        let x = *x;
        self.scale_without_origin_self_inner(x, 1.0, 1.0);
      }
      Transform::ScaleY(y) => {
        let y = *y;
        self.scale_without_origin_self_inner(1.0, y, 1.0);
      }
      Transform::ScaleZ(z) => {
        let z = *z;
        self.scale_without_origin_self_inner(1.0, 1.0, z);
        self.is_2d.set(false);
      }
      Transform::Scale3d(x, y, z) => {
        let x = *x;
        let y = *y;
        let z = *z;
        self.scale_without_origin_self_inner(x, y, z);
        self.is_2d.set(false);
      }
      Transform::Rotate(angle) => {
        self.rotate_axis_angle_self_inner(0.0, 0.0, 1.0, angle.to_radians());
      }
      Transform::RotateX(angle) => {
        self.rotate_axis_angle_self_inner(1.0, 0.0, 0.0, angle.to_radians());
        self.is_2d.set(false);
      }
      Transform::RotateY(angle) => {
        self.rotate_axis_angle_self_inner(0.0, 1.0, 0.0, angle.to_radians());
        self.is_2d.set(false);
      }
      Transform::RotateZ(angle) => {
        self.rotate_axis_angle_self_inner(0.0, 0.0, 1.0, angle.to_radians());
        self.is_2d.set(false);
      }
      Transform::Rotate3d(x, y, z, angle) => {
        let x = *x;
        let y = *y;
        let z = *z;
        self.rotate_axis_angle_self_inner(x, y, z, angle.to_radians());
        self.is_2d.set(false);
      }
      Transform::Skew(x, y) => {
        let x = x.to_radians();
        let y = if let Some(y) = y { y.to_radians() } else { 0.0 };
        self.skew_self_inner(x, y);
      }
      Transform::SkewX(angle) => {
        self.skew_self_inner(angle.to_radians(), 0.0);
      }
      Transform::SkewY(angle) => {
        self.skew_self_inner(0.0, angle.to_radians());
      }
      Transform::Perspective(length) => {
        if let Some(length) = length {
          self.perspective_self_inner(length.to_pixels());
        }
        self.is_2d.set(false);
      }
      Transform::Matrix([a, b, c, d, e, f]) => {
        let lhs = self.clone();
        let rhs = DOMMatrixReadOnly {
          #[rustfmt::skip]
          inner: RefCell::new(Matrix4::new(
            *a,  *c, 0.0,  *e,
            *b,  *d, 0.0,  *f,
           0.0, 0.0, 1.0, 0.0,
           0.0, 0.0, 0.0, 1.0,
          )),
          is_2d: Cell::new(true),
        };
        self.multiply_self_inner(&lhs, &rhs);
      }
      Transform::Matrix3d(array) => {
        let lhs = self.clone();
        let rhs = DOMMatrixReadOnly {
          #[rustfmt::skip]
          inner: RefCell::new(Matrix4::from_column_slice(array)),
          is_2d: Cell::new(false),
        };
        self.multiply_self_inner(&lhs, &rhs);
      }
    }
    Ok(())
  }
}

#[op2(base)]
impl DOMMatrixReadOnly {
  #[constructor]
  #[reentrant]
  #[required(0)]
  #[cppgc]
  fn constructor<'a>(
    state: Rc<RefCell<OpState>>,
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    DOMMatrixReadOnly::new(
      state,
      scope,
      value,
      "Failed to construct 'DOMMatrixReadOnly'".into(),
      ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
    )
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  #[cppgc]
  fn from_matrix(
    #[webidl] init: DOMMatrixInit,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    DOMMatrixReadOnly::from_matrix_inner(&init)
  }

  #[rename("fromFloat32Array")]
  #[required(1)]
  #[static_method]
  #[cppgc]
  fn from_float32_array(
    #[buffer] seq: &[f32],
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    DOMMatrixReadOnly::from_sequence_inner(seq)
  }

  #[rename("fromFloat64Array")]
  #[required(1)]
  #[static_method]
  #[cppgc]
  fn from_float64_array(
    #[buffer] seq: &[f64],
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    DOMMatrixReadOnly::from_sequence_inner(seq)
  }

  #[rename("toFloat32Array")]
  #[buffer]
  fn to_float32_array(&self) -> Vec<f32> {
    self.inner.borrow().iter().map(|&f| f as f32).collect()
  }

  #[rename("toFloat64Array")]
  #[buffer]
  fn to_float64_array(&self) -> Vec<f64> {
    self.inner.borrow().as_slice().to_vec()
  }

  #[fast]
  #[getter]
  fn a(&self) -> f64 {
    self.a_inner()
  }

  #[fast]
  #[getter]
  fn b(&self) -> f64 {
    self.b_inner()
  }

  #[fast]
  #[getter]
  fn c(&self) -> f64 {
    self.c_inner()
  }

  #[fast]
  #[getter]
  fn d(&self) -> f64 {
    self.d_inner()
  }

  #[fast]
  #[getter]
  fn e(&self) -> f64 {
    self.e_inner()
  }

  #[fast]
  #[getter]
  fn f(&self) -> f64 {
    self.f_inner()
  }

  #[fast]
  #[getter]
  fn m11(&self) -> f64 {
    self.m11_inner()
  }

  #[fast]
  #[getter]
  fn m12(&self) -> f64 {
    self.m12_inner()
  }

  #[fast]
  #[getter]
  fn m13(&self) -> f64 {
    self.m13_inner()
  }

  #[fast]
  #[getter]
  fn m14(&self) -> f64 {
    self.m14_inner()
  }

  #[fast]
  #[getter]
  fn m21(&self) -> f64 {
    self.m21_inner()
  }

  #[fast]
  #[getter]
  fn m22(&self) -> f64 {
    self.m22_inner()
  }

  #[fast]
  #[getter]
  fn m23(&self) -> f64 {
    self.m23_inner()
  }

  #[fast]
  #[getter]
  fn m24(&self) -> f64 {
    self.m24_inner()
  }

  #[fast]
  #[getter]
  fn m31(&self) -> f64 {
    self.m31_inner()
  }

  #[fast]
  #[getter]
  fn m32(&self) -> f64 {
    self.m32_inner()
  }

  #[fast]
  #[getter]
  fn m33(&self) -> f64 {
    self.m33_inner()
  }

  #[fast]
  #[getter]
  fn m34(&self) -> f64 {
    self.m34_inner()
  }

  #[fast]
  #[getter]
  fn m41(&self) -> f64 {
    self.m41_inner()
  }

  #[fast]
  #[getter]
  fn m42(&self) -> f64 {
    self.m42_inner()
  }

  #[fast]
  #[getter]
  fn m43(&self) -> f64 {
    self.m43_inner()
  }

  #[fast]
  #[getter]
  fn m44(&self) -> f64 {
    self.m44_inner()
  }

  #[fast]
  #[getter]
  fn is_2d(&self) -> bool {
    self.is_2d.get()
  }

  #[fast]
  #[getter]
  fn is_identity(&self) -> bool {
    self.is_identity_inner()
  }

  #[required(0)]
  fn translate<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] tx: Option<UnrestrictedDouble>,
    #[webidl] ty: Option<UnrestrictedDouble>,
    #[webidl] tz: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let tx = *tx.unwrap_or(UnrestrictedDouble(0.0));
    let ty = *ty.unwrap_or(UnrestrictedDouble(0.0));
    let tz = *tz.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    out.translate_self_inner(tx, ty, tz);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn scale<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] sx: Option<UnrestrictedDouble>,
    #[webidl] sy: Option<UnrestrictedDouble>,
    #[webidl] sz: Option<UnrestrictedDouble>,
    #[webidl] origin_x: Option<UnrestrictedDouble>,
    #[webidl] origin_y: Option<UnrestrictedDouble>,
    #[webidl] origin_z: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let sx = *sx.unwrap_or(UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(UnrestrictedDouble(sx));
    let sz = *sz.unwrap_or(UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      out.scale_without_origin_self_inner(sx, sy, sz);
    } else {
      out
        .scale_with_origin_self_inner(sx, sy, sz, origin_x, origin_y, origin_z);
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn scale_non_uniform<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] sx: Option<UnrestrictedDouble>,
    #[webidl] sy: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let sx = *sx.unwrap_or(UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(UnrestrictedDouble(1.0));
    let out = self.clone();
    out.scale_without_origin_self_inner(sx, sy, 1.0);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[rename("scale3d")]
  #[required(0)]
  fn scale3d<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] scale: Option<UnrestrictedDouble>,
    #[webidl] origin_x: Option<UnrestrictedDouble>,
    #[webidl] origin_y: Option<UnrestrictedDouble>,
    #[webidl] origin_z: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let scale = *scale.unwrap_or(UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      out.scale_without_origin_self_inner(scale, scale, scale);
    } else {
      out.scale_with_origin_self_inner(
        scale, scale, scale, origin_x, origin_y, origin_z,
      );
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn rotate<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] rotate_x: Option<UnrestrictedDouble>,
    #[webidl] rotate_y: Option<UnrestrictedDouble>,
    #[webidl] rotate_z: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let rotate_x = *rotate_x.unwrap_or(UnrestrictedDouble(0.0));
    let (roll_deg, pitch_deg, yaw_deg) =
      if rotate_y.is_none() && rotate_z.is_none() {
        (0.0, 0.0, rotate_x)
      } else {
        (
          rotate_x,
          *rotate_y.unwrap_or(UnrestrictedDouble(0.0)),
          *rotate_z.unwrap_or(UnrestrictedDouble(0.0)),
        )
      };
    let out = self.clone();
    out.rotate_self_inner(
      roll_deg.to_radians(),
      pitch_deg.to_radians(),
      yaw_deg.to_radians(),
    );
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn rotate_from_vector<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x = *x.unwrap_or(UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    out.rotate_from_vector_self_inner(x, y);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn rotate_axis_angle<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] z: Option<UnrestrictedDouble>,
    #[webidl] angle_deg: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x = *x.unwrap_or(UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(UnrestrictedDouble(0.0));
    let z = *z.unwrap_or(UnrestrictedDouble(0.0));
    let angle_deg = *angle_deg.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    out.rotate_axis_angle_self_inner(x, y, z, angle_deg.to_radians());
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn skew_x<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] x_deg: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x_deg = *x_deg.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    out.skew_self_inner(x_deg.to_radians(), 0.0);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn skew_y<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] y_deg: Option<UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let y_deg = *y_deg.unwrap_or(UnrestrictedDouble(0.0));
    let out = self.clone();
    out.skew_self_inner(0.0, y_deg.to_radians());
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn multiply<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    other: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let out = self.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_base_object::<DOMMatrixReadOnly>(scope, other)
    {
      out.multiply_self_inner(self, &other);
    } else {
      let other = DOMMatrixInit::convert(
        scope,
        other,
        "Failed to execute 'multiply' on 'DOMMatrixReadOnly'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let other = DOMMatrixReadOnly::from_matrix_inner(&other)?;
      out.multiply_self_inner(self, &other);
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMMatrix { base: out }))
  }

  #[required(0)]
  fn flip_x<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.flip_x_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn flip_y<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.flip_y_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[required(0)]
  fn inverse<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.invert_self_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object(scope, obj, DOMMatrix { base: out })
  }

  #[reentrant]
  #[required(0)]
  fn transform_point<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    point: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let out = DOMPointReadOnly {
      inner: RefCell::new(Vector4::zeros()),
    };
    if let Some(point) =
      cppgc::try_unwrap_cppgc_base_object::<DOMPointReadOnly>(scope, point)
    {
      matrix_transform_point(self, &point, &out);
    } else {
      let point = DOMPointInit::convert(
        scope,
        point,
        "Failed to execute 'transformPoint' on 'DOMMatrixReadOnly'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let point = DOMPointReadOnly::from_point_inner(point);
      matrix_transform_point(self, &point, &out);
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMPoint { base: out }))
  }

  #[rename("toJSON")]
  #[required(0)]
  fn to_json<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    define_obj!(scope => {
      num "a": self.a_inner(),
      num "b": self.b_inner(),
      num "c": self.c_inner(),
      num "d": self.d_inner(),
      num "e": self.e_inner(),
      num "f": self.f_inner(),
      num "m11": self.m11_inner(),
      num "m12": self.m12_inner(),
      num "m13": self.m13_inner(),
      num "m14": self.m14_inner(),
      num "m21": self.m21_inner(),
      num "m22": self.m22_inner(),
      num "m23": self.m23_inner(),
      num "m24": self.m24_inner(),
      num "m31": self.m31_inner(),
      num "m32": self.m32_inner(),
      num "m33": self.m33_inner(),
      num "m34": self.m34_inner(),
      num "m41": self.m41_inner(),
      num "m42": self.m42_inner(),
      num "m43": self.m43_inner(),
      num "m44": self.m44_inner(),
      bool "is2D": self.is_2d.get(),
      bool "isIdentity": self.is_identity_inner(),
    })
  }
}

#[derive(CppgcInherits, CppgcBase)]
#[cppgc_inherits_from(DOMMatrixReadOnly)]
#[repr(C)]
pub struct DOMMatrix {
  base: DOMMatrixReadOnly,
}

// SAFETY: we're sure `DOMMatrix` can be GCed
unsafe impl GarbageCollected for DOMMatrix {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMMatrix"
  }
}

#[op2(inherit = DOMMatrixReadOnly)]
impl DOMMatrix {
  #[constructor]
  #[reentrant]
  #[required(0)]
  #[cppgc]
  fn constructor<'a>(
    state: Rc<RefCell<OpState>>,
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
    // TODO(petamoriken): Error when deleting next line. proc-macro bug?
    #[webidl] _: bool,
  ) -> Result<DOMMatrix, GeometryError> {
    let ro = DOMMatrixReadOnly::new(
      state,
      scope,
      value,
      "Failed to construct 'DOMMatrix'".into(),
      ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
    )?;
    Ok(DOMMatrix { base: ro })
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_matrix<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] init: DOMMatrixInit,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let ro = DOMMatrixReadOnly::from_matrix_inner(&init)?;
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMMatrix { base: ro }))
  }

  #[rename("fromFloat32Array")]
  #[required(1)]
  #[static_method]
  fn from_float32_array<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[buffer] seq: &[f32],
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let ro = DOMMatrixReadOnly::from_sequence_inner(seq)?;
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMMatrix { base: ro }))
  }

  #[rename("fromFloat64Array")]
  #[required(1)]
  #[static_method]
  fn from_float64_array<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[buffer] seq: &[f64],
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let ro = DOMMatrixReadOnly::from_sequence_inner(seq)?;
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object(scope, obj, DOMMatrix { base: ro }))
  }

  #[fast]
  #[getter]
  fn a(&self) -> f64 {
    self.base.a_inner()
  }

  #[setter]
  fn a(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_A] = *value;
  }

  #[fast]
  #[getter]
  fn b(&self) -> f64 {
    self.base.b_inner()
  }

  #[setter]
  fn b(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_B] = *value;
  }

  #[fast]
  #[getter]
  fn c(&self) -> f64 {
    self.base.c_inner()
  }

  #[setter]
  fn c(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_C] = *value;
  }

  #[fast]
  #[getter]
  fn d(&self) -> f64 {
    self.base.d_inner()
  }

  #[setter]
  fn d(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_D] = *value;
  }

  #[fast]
  #[getter]
  fn e(&self) -> f64 {
    self.base.e_inner()
  }

  #[setter]
  fn e(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_E] = *value;
  }

  #[fast]
  #[getter]
  fn f(&self) -> f64 {
    self.base.f_inner()
  }

  #[setter]
  fn f(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_F] = *value;
  }

  #[fast]
  #[getter]
  fn m11(&self) -> f64 {
    self.base.m11_inner()
  }

  #[setter]
  fn m11(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M11] = *value;
  }

  #[fast]
  #[getter]
  fn m12(&self) -> f64 {
    self.base.m12_inner()
  }

  #[setter]
  fn m12(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M12] = *value;
  }

  #[fast]
  #[getter]
  fn m13(&self) -> f64 {
    self.base.m13_inner()
  }

  #[setter]
  fn m13(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M13] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m14(&self) -> f64 {
    self.base.m14_inner()
  }

  #[setter]
  fn m14(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M14] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m21(&self) -> f64 {
    self.base.m21_inner()
  }

  #[setter]
  fn m21(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M21] = *value;
  }

  #[fast]
  #[getter]
  fn m22(&self) -> f64 {
    self.base.m22_inner()
  }

  #[setter]
  fn m22(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M22] = *value;
  }

  #[fast]
  #[getter]
  fn m23(&self) -> f64 {
    self.base.m23_inner()
  }

  #[setter]
  fn m23(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M23] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m24(&self) -> f64 {
    self.base.m24_inner()
  }

  #[setter]
  fn m24(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M24] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m31(&self) -> f64 {
    self.base.m31_inner()
  }

  #[setter]
  fn m31(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M31] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m32(&self) -> f64 {
    self.base.m32_inner()
  }

  #[setter]
  fn m32(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M32] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m33(&self) -> f64 {
    self.base.m33_inner()
  }

  #[setter]
  fn m33(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M33] = *value;
    if *value != 1.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m34(&self) -> f64 {
    self.base.m34_inner()
  }

  #[setter]
  fn m34(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M34] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m41(&self) -> f64 {
    self.base.m41_inner()
  }

  #[setter]
  fn m41(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M41] = *value;
  }

  #[fast]
  #[getter]
  fn m42(&self) -> f64 {
    self.base.m42_inner()
  }

  #[setter]
  fn m42(&self, #[webidl] value: UnrestrictedDouble) {
    self.base.inner.borrow_mut()[INDEX_M42] = *value;
  }

  #[fast]
  #[getter]
  fn m43(&self) -> f64 {
    self.base.m43_inner()
  }

  #[setter]
  fn m43(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M43] = *value;
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m44(&self) -> f64 {
    self.base.m44_inner()
  }

  #[setter]
  fn m44(&self, #[webidl] value: UnrestrictedDouble) {
    let ro = &self.base;
    ro.inner.borrow_mut()[INDEX_M44] = *value;
    if *value != 1.0 {
      ro.is_2d.set(false);
    }
  }

  #[required(0)]
  fn translate_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] tx: Option<UnrestrictedDouble>,
    #[webidl] ty: Option<UnrestrictedDouble>,
    #[webidl] tz: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let tx = *tx.unwrap_or(UnrestrictedDouble(0.0));
    let ty = *ty.unwrap_or(UnrestrictedDouble(0.0));
    let tz = *tz.unwrap_or(UnrestrictedDouble(0.0));
    self.base.translate_self_inner(tx, ty, tz);
    this
  }

  #[required(0)]
  fn scale_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] sx: Option<UnrestrictedDouble>,
    #[webidl] sy: Option<UnrestrictedDouble>,
    #[webidl] sz: Option<UnrestrictedDouble>,
    #[webidl] origin_x: Option<UnrestrictedDouble>,
    #[webidl] origin_y: Option<UnrestrictedDouble>,
    #[webidl] origin_z: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let sx = *sx.unwrap_or(UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(UnrestrictedDouble(sx));
    let sz = *sz.unwrap_or(UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(UnrestrictedDouble(0.0));
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      self.base.scale_without_origin_self_inner(sx, sy, sz);
    } else {
      self
        .base
        .scale_with_origin_self_inner(sx, sy, sz, origin_x, origin_y, origin_z);
    }
    this
  }

  #[rename("scale3dSelf")]
  #[required(0)]
  fn scale3d_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] scale: Option<UnrestrictedDouble>,
    #[webidl] origin_x: Option<UnrestrictedDouble>,
    #[webidl] origin_y: Option<UnrestrictedDouble>,
    #[webidl] origin_z: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let scale = *scale.unwrap_or(UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(UnrestrictedDouble(0.0));
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      self
        .base
        .scale_without_origin_self_inner(scale, scale, scale);
    } else {
      self.base.scale_with_origin_self_inner(
        scale, scale, scale, origin_x, origin_y, origin_z,
      );
    }
    this
  }

  #[required(0)]
  fn rotate_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] rotate_x: Option<UnrestrictedDouble>,
    #[webidl] rotate_y: Option<UnrestrictedDouble>,
    #[webidl] rotate_z: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let rotate_x = *rotate_x.unwrap_or(UnrestrictedDouble(0.0));
    let (roll_deg, pitch_deg, yaw_deg) =
      if rotate_y.is_none() && rotate_z.is_none() {
        (0.0, 0.0, rotate_x)
      } else {
        (
          rotate_x,
          *rotate_y.unwrap_or(UnrestrictedDouble(0.0)),
          *rotate_z.unwrap_or(UnrestrictedDouble(0.0)),
        )
      };
    self.base.rotate_self_inner(
      roll_deg.to_radians(),
      pitch_deg.to_radians(),
      yaw_deg.to_radians(),
    );
    this
  }

  #[required(0)]
  fn rotate_from_vector_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let x = *x.unwrap_or(UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(UnrestrictedDouble(0.0));
    self.base.rotate_from_vector_self_inner(x, y);
    this
  }

  #[required(0)]
  fn rotate_axis_angle_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x: Option<UnrestrictedDouble>,
    #[webidl] y: Option<UnrestrictedDouble>,
    #[webidl] z: Option<UnrestrictedDouble>,
    #[webidl] angle_deg: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let x = *x.unwrap_or(UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(UnrestrictedDouble(0.0));
    let z = *z.unwrap_or(UnrestrictedDouble(0.0));
    let angle_deg = *angle_deg.unwrap_or(UnrestrictedDouble(0.0));
    self
      .base
      .rotate_axis_angle_self_inner(x, y, z, angle_deg.to_radians());
    this
  }

  #[required(0)]
  fn skew_x_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x_deg: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let x_deg = *x_deg.unwrap_or(UnrestrictedDouble(0.0));
    self.base.skew_self_inner(x_deg.to_radians(), 0.0);
    this
  }

  #[required(0)]
  fn skew_y_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] y_deg: Option<UnrestrictedDouble>,
  ) -> v8::Global<v8::Object> {
    let y_deg = *y_deg.unwrap_or(UnrestrictedDouble(0.0));
    self.base.skew_self_inner(0.0, y_deg.to_radians());
    this
  }

  #[required(0)]
  fn multiply_self<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'a, '_>,
    other: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Global<v8::Object>, GeometryError> {
    let lhs = self.base.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_base_object::<DOMMatrixReadOnly>(scope, other)
    {
      if ptr::eq(&self.base, &*other) {
        self.base.multiply_self_inner(&lhs, &other.clone());
      } else {
        self.base.multiply_self_inner(&lhs, &other);
      };
    } else {
      let other = DOMMatrixInit::convert(
        scope,
        other,
        "Failed to execute 'multiplySelf' on 'DOMMatrix'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let other = DOMMatrixReadOnly::from_matrix_inner(&other)?;
      self.base.multiply_self_inner(&lhs, &other);
    }
    Ok(this)
  }

  #[required(0)]
  fn pre_multiply_self<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'a, '_>,
    other: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Global<v8::Object>, GeometryError> {
    let rhs = self.base.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_base_object::<DOMMatrixReadOnly>(scope, other)
    {
      if ptr::eq(&self.base, &*other) {
        self.base.multiply_self_inner(&other.clone(), &rhs);
      } else {
        self.base.multiply_self_inner(&other, &rhs);
      }
    } else {
      let other = DOMMatrixInit::convert(
        scope,
        other,
        "Failed to execute 'preMultiplySelf' on 'DOMMatrix'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let other = DOMMatrixReadOnly::from_matrix_inner(&other)?;
      self.base.multiply_self_inner(&other, &rhs);
    }
    Ok(this)
  }

  #[required(0)]
  fn invert_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
  ) -> v8::Global<v8::Object> {
    self.base.invert_self_inner();
    this
  }
}

#[inline]
fn matrix_transform_point(
  matrix: &DOMMatrixReadOnly,
  point: &DOMPointReadOnly,
  out: &DOMPointReadOnly,
) {
  let inner = matrix.inner.borrow();
  let point = point.inner.borrow();
  let mut result = out.inner.borrow_mut();
  *result = inner.multiply_vector(&point);
}

#[op2]
#[string]
pub fn op_geometry_matrix_to_string<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  matrix: v8::Local<'a, v8::Value>,
) -> Result<String, GeometryError> {
  #[inline]
  fn to_string(scope: &mut v8::PinScope<'_, '_>, value: f64) -> String {
    let number = v8::Number::new(scope, value);
    number.to_string(scope).unwrap().to_rust_string_lossy(scope)
  }

  let Some(matrix) =
    cppgc::try_unwrap_cppgc_base_object::<DOMMatrixReadOnly>(scope, matrix)
  else {
    return Err(GeometryError::IllegalInvocation);
  };
  if !matrix.is_finite_inner() {
    return Err(GeometryError::InvalidState);
  }
  if matrix.is_2d.get() {
    Ok(format!(
      "matrix({}, {}, {}, {}, {}, {})",
      to_string(scope, matrix.a_inner()),
      to_string(scope, matrix.b_inner()),
      to_string(scope, matrix.c_inner()),
      to_string(scope, matrix.d_inner()),
      to_string(scope, matrix.e_inner()),
      to_string(scope, matrix.f_inner()),
    ))
  } else {
    Ok(format!(
      "matrix3d({})",
      matrix
        .inner
        .borrow()
        .iter()
        .map(|item| to_string(scope, *item))
        .collect::<Vec::<String>>()
        .join(", ")
    ))
  }
}

#[op2(fast)]
pub fn op_geometry_matrix_set_matrix_value<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  input: v8::Local<'a, v8::Value>,
  transform_list: v8::Local<'a, v8::Value>,
) -> Result<(), GeometryError> {
  let Some(matrix) =
    cppgc::try_unwrap_cppgc_base_object::<DOMMatrix>(scope, input)
  else {
    return Err(GeometryError::IllegalInvocation);
  };
  let transform_list = String::convert(
    scope,
    transform_list,
    "Failed to execute 'setMatrixValue' on 'DOMMatrix'".into(),
    (|| Cow::Borrowed("Argument 1")).into(),
    &Default::default(),
  )?;

  if transform_list.is_empty() {
    // Make it an identity matrix
    let mut inner = matrix.base.inner.borrow_mut();
    inner.fill(0.0);
    inner.fill_diagonal(1.0);
    matrix.base.is_2d.set(true);
    Ok(())
  } else {
    let result = DOMMatrixReadOnly::identity();
    let mut input = ParserInput::new(&transform_list);
    for transform_result in TransformListParser::new(&mut input) {
      let transform = transform_result?;
      result.exec_css_transform(&transform)?;
    }
    matrix.base.inner.swap(&result.inner);
    matrix.base.is_2d.swap(&result.is_2d);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  //! Differential tests for the hand-rolled matrix math that replaced
  //! `nalgebra` (see #34771). Each production routine is checked against an
  //! independent reference implementation written here (a different algorithm,
  //! not just a copy), so a transposed index or flipped sign in the production
  //! code fails the test. Matrices are stored column-major: element `(row,
  //! col)` lives at index `col * 4 + row`, matching `Matrix4`.

  use approx::assert_relative_eq;

  use super::Matrix4;
  use super::Vector4;
  use super::rotation_from_axis_angle;
  use super::rotation_from_euler_angles;

  /// Deterministic SplitMix64 PRNG so the test is reproducible without pulling
  /// in `rand`.
  struct Rng(u64);

  impl Rng {
    fn next_u64(&mut self) -> u64 {
      self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
      let mut z = self.0;
      z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
      z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
      z ^ (z >> 31)
    }

    /// Uniform `f64` in `[lo, hi)`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
      let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
      lo + (hi - lo) * u
    }

    fn matrix(&mut self, lo: f64, hi: f64) -> [f64; 16] {
      let mut m = [0.0; 16];
      for v in &mut m {
        *v = self.range(lo, hi);
      }
      m
    }
  }

  #[inline]
  fn at(m: &[f64; 16], row: usize, col: usize) -> f64 {
    m[col * 4 + row]
  }

  /// Naive triple-loop matrix product, `lhs * rhs`, independent of
  /// `Matrix4::multiply`'s indexing.
  fn ref_mul(lhs: &[f64; 16], rhs: &[f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for col in 0..4 {
      for row in 0..4 {
        let mut sum = 0.0;
        for k in 0..4 {
          sum += at(lhs, row, k) * at(rhs, k, col);
        }
        out[col * 4 + row] = sum;
      }
    }
    out
  }

  /// 4x4 inverse via Gauss-Jordan elimination with partial pivoting. This is a
  /// completely different algorithm from the production adjugate/cofactor
  /// expansion, so agreement between the two is strong evidence of
  /// correctness. Returns the inverse plus the smallest pivot magnitude
  /// encountered (a cheap conditioning proxy used to skip ill-conditioned
  /// samples).
  #[allow(
    clippy::needless_range_loop,
    reason = "elimination mutates one row while reading another; iterator borrows can't express it"
  )]
  fn ref_inverse(m: &[f64; 16]) -> Option<([f64; 16], f64)> {
    // Augmented [A | I], row-major working storage.
    let mut a = [[0.0f64; 8]; 4];
    for row in 0..4 {
      for col in 0..4 {
        a[row][col] = at(m, row, col);
      }
      a[row][4 + row] = 1.0;
    }

    let mut min_pivot = f64::INFINITY;
    for col in 0..4 {
      // Partial pivot: pick the row at or below `col` with the largest
      // magnitude in this column.
      let mut pivot_row = col;
      for row in (col + 1)..4 {
        if a[row][col].abs() > a[pivot_row][col].abs() {
          pivot_row = row;
        }
      }
      let pivot = a[pivot_row][col];
      if pivot == 0.0 {
        return None;
      }
      min_pivot = min_pivot.min(pivot.abs());
      a.swap(col, pivot_row);

      let inv_pivot = 1.0 / a[col][col];
      for v in &mut a[col] {
        *v *= inv_pivot;
      }
      for row in 0..4 {
        if row == col {
          continue;
        }
        let factor = a[row][col];
        if factor == 0.0 {
          continue;
        }
        for k in 0..8 {
          a[row][k] -= factor * a[col][k];
        }
      }
    }

    let mut inv = [0.0; 16];
    for row in 0..4 {
      for col in 0..4 {
        inv[col * 4 + row] = a[row][4 + col];
      }
    }
    Some((inv, min_pivot))
  }

  /// Homogeneous 4x4 from a 3x3 rotation given as `r[row][col]`.
  fn homogeneous(r: [[f64; 3]; 3]) -> [f64; 16] {
    let mut m = [0.0; 16];
    for row in 0..3 {
      for col in 0..3 {
        m[col * 4 + row] = r[row][col];
      }
    }
    m[15] = 1.0;
    m
  }

  fn rot_x(a: f64) -> [f64; 16] {
    let (s, c) = a.sin_cos();
    homogeneous([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])
  }

  fn rot_y(a: f64) -> [f64; 16] {
    let (s, c) = a.sin_cos();
    homogeneous([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])
  }

  fn rot_z(a: f64) -> [f64; 16] {
    let (s, c) = a.sin_cos();
    homogeneous([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])
  }

  fn assert_matrix_eq(actual: &[f64], expected: &[f64], max_relative: f64) {
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
      assert!(
        approx::relative_eq!(a, e, max_relative = max_relative, epsilon = 1e-9),
        "mismatch at index {i} (row {}, col {}): {a} != {e}",
        i % 4,
        i / 4,
      );
    }
  }

  #[test]
  fn multiply_matches_reference() {
    let mut rng = Rng(0x1234_5678);
    for _ in 0..2000 {
      let a = rng.matrix(-5.0, 5.0);
      let b = rng.matrix(-5.0, 5.0);
      let got = Matrix4::multiply(
        &Matrix4::from_column_slice(&a),
        &Matrix4::from_column_slice(&b),
      );
      assert_matrix_eq(got.as_slice(), &ref_mul(&a, &b), 1e-12);
    }
  }

  #[test]
  fn multiply_vector_matches_reference() {
    let mut rng = Rng(0xC0FF_EE42);
    for _ in 0..2000 {
      let m = rng.matrix(-5.0, 5.0);
      let v = [
        rng.range(-5.0, 5.0),
        rng.range(-5.0, 5.0),
        rng.range(-5.0, 5.0),
        rng.range(-5.0, 5.0),
      ];
      let got = Matrix4::from_column_slice(&m)
        .multiply_vector(&Vector4::new(v[0], v[1], v[2], v[3]));
      let expected = [
        at(&m, 0, 0) * v[0]
          + at(&m, 0, 1) * v[1]
          + at(&m, 0, 2) * v[2]
          + at(&m, 0, 3) * v[3],
        at(&m, 1, 0) * v[0]
          + at(&m, 1, 1) * v[1]
          + at(&m, 1, 2) * v[2]
          + at(&m, 1, 3) * v[3],
        at(&m, 2, 0) * v[0]
          + at(&m, 2, 1) * v[1]
          + at(&m, 2, 2) * v[2]
          + at(&m, 2, 3) * v[3],
        at(&m, 3, 0) * v[0]
          + at(&m, 3, 1) * v[1]
          + at(&m, 3, 2) * v[2]
          + at(&m, 3, 3) * v[3],
      ];
      for (a, e) in [got.x, got.y, got.z, got.w].iter().zip(expected.iter()) {
        assert_relative_eq!(a, e, max_relative = 1e-12, epsilon = 1e-9);
      }
    }
  }

  #[test]
  fn inverse_matches_gauss_jordan_and_round_trips() {
    let mut rng = Rng(0xDEAD_BEEF);
    let identity = Matrix4::identity();
    let mut well_conditioned = 0;
    for _ in 0..5000 {
      let m = rng.matrix(-1.0, 1.0);
      let Some((ref_inv, min_pivot)) = ref_inverse(&m) else {
        continue;
      };
      // Skip ill-conditioned matrices: with a tiny pivot the two algorithms
      // legitimately diverge in the last digits, which says nothing about
      // correctness.
      if min_pivot < 1e-3 {
        continue;
      }
      well_conditioned += 1;

      let mut prod = Matrix4::from_column_slice(&m);
      assert!(
        prod.try_inverse_mut(),
        "production inverse failed on a matrix the reference inverted"
      );

      // Cross-check against the independent Gauss-Jordan result.
      assert_matrix_eq(prod.as_slice(), &ref_inv, 1e-6);

      // Invariant: M * M^-1 == I.
      let round_trip =
        Matrix4::multiply(&Matrix4::from_column_slice(&m), &prod);
      assert_matrix_eq(round_trip.as_slice(), identity.as_slice(), 1e-6);
    }
    // Make sure the conditioning filter didn't skip ~everything.
    assert!(
      well_conditioned > 500,
      "only {well_conditioned} well-conditioned samples; filter too strict"
    );
  }

  #[test]
  fn singular_matrix_is_not_inverted() {
    // Column 3 == column 1, so the matrix is rank-deficient.
    let mut singular = Matrix4::identity();
    for row in 0..4 {
      singular[(row, 3)] = singular[(row, 1)];
    }
    assert!(!singular.try_inverse_mut());
  }

  #[test]
  fn euler_matches_composed_axis_rotations() {
    let mut rng = Rng(0x00C0_1DEE);
    for _ in 0..2000 {
      let roll = rng.range(-3.2, 3.2);
      let pitch = rng.range(-3.2, 3.2);
      let yaw = rng.range(-3.2, 3.2);
      // Production composition is R = Rz(yaw) * Ry(pitch) * Rx(roll).
      let expected =
        ref_mul(&ref_mul(&rot_z(yaw), &rot_y(pitch)), &rot_x(roll));
      let got = rotation_from_euler_angles(roll, pitch, yaw);
      assert_matrix_eq(got.as_slice(), &expected, 1e-12);
    }
  }

  #[test]
  fn axis_angle_matches_quaternion() {
    let mut rng = Rng(0xFACE_F00D);
    for _ in 0..2000 {
      let mut x = rng.range(-1.0, 1.0);
      let mut y = rng.range(-1.0, 1.0);
      let mut z = rng.range(-1.0, 1.0);
      let len = (x * x + y * y + z * z).sqrt();
      if len < 1e-3 {
        continue; // degenerate axis
      }
      x /= len;
      y /= len;
      z /= len;
      let angle = rng.range(-3.2, 3.2);

      // Independent reference: build the rotation from a unit quaternion.
      let (s, c) = (angle / 2.0).sin_cos();
      let (qw, qx, qy, qz) = (c, s * x, s * y, s * z);
      let expected = homogeneous([
        [
          1.0 - 2.0 * (qy * qy + qz * qz),
          2.0 * (qx * qy - qz * qw),
          2.0 * (qx * qz + qy * qw),
        ],
        [
          2.0 * (qx * qy + qz * qw),
          1.0 - 2.0 * (qx * qx + qz * qz),
          2.0 * (qy * qz - qx * qw),
        ],
        [
          2.0 * (qx * qz - qy * qw),
          2.0 * (qy * qz + qx * qw),
          1.0 - 2.0 * (qx * qx + qy * qy),
        ],
      ]);

      let got = rotation_from_axis_angle(x, y, z, angle);
      assert_matrix_eq(got.as_slice(), &expected, 1e-9);
    }
  }
}
