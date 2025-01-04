// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::path::PathBuf;
use std::slice;

use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use nalgebra::Matrix3;
use nalgebra::Matrix4;
use nalgebra::Matrix4x2;
use nalgebra::Matrix4x3;
use nalgebra::Rotation3;
use nalgebra::UnitVector3;
use nalgebra::Vector3;
use nalgebra::Vector4;

deno_core::extension!(
  deno_geometry,
  deps = [deno_webidl, deno_web, deno_console],
  ops = [op_geometry_create_matrix_identity],
  objects = [DOMPointInner, DOMRectInner, DOMMatrixInner],
  esm = ["00_init.js"],
  lazy_loaded_esm = ["01_geometry.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_geometry.d.ts")
}

#[derive(Debug, thiserror::Error)]
pub enum GeometryError {
  #[error("Inconsistent 2d matrix value")]
  Inconsistent2DMatrix, // TypeError
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct DOMPointInit {
  #[webidl(default = 0.0)]
  x: f64,
  #[webidl(default = 0.0)]
  y: f64,
  #[webidl(default = 0.0)]
  z: f64,
  #[webidl(default = 1.0)]
  w: f64,
}

pub struct DOMPointInner {
  x: Cell<f64>,
  y: Cell<f64>,
  z: Cell<f64>,
  w: Cell<f64>,
}

impl GarbageCollected for DOMPointInner {}

#[op2]
impl DOMPointInner {
  #[constructor]
  #[cppgc]
  pub fn constructor(x: f64, y: f64, z: f64, w: f64) -> DOMPointInner {
    DOMPointInner {
      x: Cell::new(x),
      y: Cell::new(y),
      z: Cell::new(z),
      w: Cell::new(w),
    }
  }

  #[static_method]
  #[cppgc]
  pub fn from_point(#[webidl] init: DOMPointInit) -> DOMPointInner {
    DOMPointInner {
      x: Cell::new(init.x),
      y: Cell::new(init.y),
      z: Cell::new(init.z),
      w: Cell::new(init.w),
    }
  }

  #[fast]
  #[getter]
  pub fn x(&self) -> f64 {
    self.x.get()
  }

  #[fast]
  #[setter]
  pub fn x(&self, value: f64) {
    self.x.set(value)
  }

  #[fast]
  #[getter]
  pub fn y(&self) -> f64 {
    self.y.get()
  }

  #[fast]
  #[setter]
  pub fn y(&self, value: f64) {
    self.y.set(value)
  }

  #[fast]
  #[getter]
  pub fn z(&self) -> f64 {
    self.z.get()
  }

  #[fast]
  #[setter]
  pub fn z(&self, value: f64) {
    self.z.set(value)
  }

  #[fast]
  #[getter]
  pub fn w(&self) -> f64 {
    self.w.get()
  }

  #[fast]
  #[setter]
  pub fn w(&self, value: f64) {
    self.w.set(value)
  }

  #[cppgc]
  pub fn matrix_transform(
    &self,
    matrix: v8::Local<v8::Object>,
  ) -> DOMPointInner {
    let matrix = cast_to_matrix(matrix);
    let out = DOMPointInner {
      x: Cell::new(0.0),
      y: Cell::new(0.0),
      z: Cell::new(0.0),
      w: Cell::new(0.0),
    };
    matrix_transform_point(&matrix, self, &out);
    out
  }
}

pub struct DOMRectInner {
  x: Cell<f64>,
  y: Cell<f64>,
  width: Cell<f64>,
  height: Cell<f64>,
}

impl GarbageCollected for DOMRectInner {}

#[op2]
impl DOMRectInner {
  #[constructor]
  #[cppgc]
  pub fn constructor(x: f64, y: f64, width: f64, height: f64) -> DOMRectInner {
    DOMRectInner {
      x: Cell::new(x),
      y: Cell::new(y),
      width: Cell::new(width),
      height: Cell::new(height),
    }
  }

  #[fast]
  #[getter]
  pub fn x(&self) -> f64 {
    self.x.get()
  }

  #[fast]
  #[setter]
  pub fn x(&self, value: f64) {
    self.x.set(value)
  }

  #[fast]
  #[getter]
  pub fn y(&self) -> f64 {
    self.y.get()
  }

  #[fast]
  #[setter]
  pub fn y(&self, value: f64) {
    self.y.set(value)
  }

  #[fast]
  #[getter]
  pub fn width(&self) -> f64 {
    self.width.get()
  }

  #[fast]
  #[setter]
  pub fn width(&self, value: f64) {
    self.width.set(value)
  }

  #[fast]
  #[getter]
  pub fn height(&self) -> f64 {
    self.height.get()
  }

  #[fast]
  #[setter]
  pub fn height(&self, value: f64) {
    self.height.set(value)
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub struct DOMMatrixInit {
  #[webidl(default = None)]
  a: Option<f64>,
  #[webidl(default = None)]
  b: Option<f64>,
  #[webidl(default = None)]
  c: Option<f64>,
  #[webidl(default = None)]
  d: Option<f64>,
  #[webidl(default = None)]
  e: Option<f64>,
  #[webidl(default = None)]
  f: Option<f64>,
  #[webidl(default = None)]
  m11: Option<f64>,
  #[webidl(default = None)]
  m12: Option<f64>,
  #[webidl(default = 0.0)]
  m13: f64,
  #[webidl(default = 0.0)]
  m14: f64,
  #[webidl(default = None)]
  m21: Option<f64>,
  #[webidl(default = None)]
  m22: Option<f64>,
  #[webidl(default = 0.0)]
  m23: f64,
  #[webidl(default = 0.0)]
  m24: f64,
  #[webidl(default = 0.0)]
  m31: f64,
  #[webidl(default = 0.0)]
  m32: f64,
  #[webidl(default = 1.0)]
  m33: f64,
  #[webidl(default = 0.0)]
  m34: f64,
  #[webidl(default = None)]
  m41: Option<f64>,
  #[webidl(default = None)]
  m42: Option<f64>,
  #[webidl(default = 0.0)]
  m43: f64,
  #[webidl(default = 1.0)]
  m44: f64,
  #[webidl(default = None)]
  is_2d: Option<bool>,
}

#[derive(Clone)]
pub struct DOMMatrixInner {
  inner: RefCell<Matrix4<f64>>,
  is_2d: Cell<bool>,
}

impl GarbageCollected for DOMMatrixInner {}

/*
 * NOTE: column-major order
 *
 * For a 2D 3x2 matrix, the index of properties in
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

#[op2]
impl DOMMatrixInner {
  #[constructor]
  #[cppgc]
  pub fn constructor(#[buffer] buffer: &[f64], is_2d: bool) -> DOMMatrixInner {
    DOMMatrixInner {
      inner: RefCell::new(Matrix4::from_column_slice(buffer)),
      is_2d: Cell::new(is_2d),
    }
  }

  #[static_method]
  #[cppgc]
  pub fn from_matrix(
    #[webidl] init: DOMMatrixInit,
  ) -> Result<DOMMatrixInner, GeometryError> {
    macro_rules! fixup {
      ($value3d:expr, $value2d:expr, $default:expr) => {{
        if let Some(value3d) = $value3d {
          if let Some(value2d) = $value2d {
            if !(value3d == value2d || value3d.is_nan() && value2d.is_nan()) {
              return Err(GeometryError::Inconsistent2DMatrix);
            }
          }
          value3d
        } else if let Some(value2d) = $value2d {
          value2d
        } else {
          $default
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
      let is_2d_can_be_true = init.m13 == 0.0
        && init.m14 == 0.0
        && init.m23 == 0.0
        && init.m24 == 0.0
        && init.m31 == 0.0
        && init.m32 == 0.0
        && init.m33 == 1.0
        && init.m34 == 0.0
        && init.m43 == 0.0
        && init.m44 == 1.0;
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
      Ok(DOMMatrixInner {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
          m11, m21, 0.0, m41,
          m12, m22, 0.0, m42,
          0.0, 0.0, 1.0, 0.0,
          0.0, 0.0, 0.0, 1.0,
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
      Ok(DOMMatrixInner {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
          m11, m21, m31, m41,
          m12, m22, m32, m42,
          m13, m23, m33, m43,
          m14, m24, m34, m44,
        )),
        is_2d: Cell::new(false),
      })
    }
  }

  #[cppgc]
  pub fn clone(&self) -> DOMMatrixInner {
    self.clone()
  }

  #[fast]
  #[getter]
  pub fn a(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_A) }
  }

  #[fast]
  #[setter]
  pub fn a(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_A) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn b(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_B) }
  }

  #[fast]
  #[setter]
  pub fn b(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_B) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn c(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_C) }
  }

  #[fast]
  #[setter]
  pub fn c(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_C) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn d(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_D) }
  }

  #[fast]
  #[setter]
  pub fn d(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_D) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn e(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_E) }
  }

  #[fast]
  #[setter]
  pub fn e(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_E) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn f(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_F) }
  }

  #[fast]
  #[setter]
  pub fn f(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_F) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m11(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M11) }
  }

  #[fast]
  #[setter]
  pub fn m11(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M11) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m12(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M12) }
  }

  #[fast]
  #[setter]
  pub fn m12(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M12) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m13(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M13) }
  }

  #[fast]
  #[setter]
  pub fn m13(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M13) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m14(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M14) }
  }

  #[fast]
  #[setter]
  pub fn m14(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M14) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m21(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M21) }
  }

  #[fast]
  #[setter]
  pub fn m21(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M21) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m22(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M22) }
  }

  #[fast]
  #[setter]
  pub fn m22(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M22) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m23(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M23) }
  }

  #[fast]
  #[setter]
  pub fn m23(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M23) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m24(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M24) }
  }

  #[fast]
  #[setter]
  pub fn m24(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M24) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m31(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M31) }
  }

  #[fast]
  #[setter]
  pub fn m31(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M31) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m32(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M32) }
  }

  #[fast]
  #[setter]
  pub fn m32(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M32) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m33(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M33) }
  }

  #[fast]
  #[setter]
  pub fn m33(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M33) = value;
    }
    if value != 1.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m34(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M34) }
  }

  #[fast]
  #[setter]
  pub fn m34(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M34) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m41(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M41) }
  }

  #[fast]
  #[setter]
  pub fn m41(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M41) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m42(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M42) }
  }

  #[fast]
  #[setter]
  pub fn m42(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M42) = value;
    }
  }

  #[fast]
  #[getter]
  pub fn m43(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M43) }
  }

  #[fast]
  #[setter]
  pub fn m43(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M43) = value;
    }
    if value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m44(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M44) }
  }

  #[fast]
  #[setter]
  pub fn m44(&self, value: f64) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M44) = value;
    }
    if value != 1.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn is_2d(&self) -> bool {
    self.is_2d.get()
  }

  #[fast]
  #[getter]
  pub fn is_identity(&self) -> bool {
    let inner = self.inner.borrow();
    // SAFETY: in-range access
    unsafe {
      *inner.get_unchecked(INDEX_M11) == 1.0
        && *inner.get_unchecked(INDEX_M12) == 0.0
        && *inner.get_unchecked(INDEX_M13) == 0.0
        && *inner.get_unchecked(INDEX_M14) == 0.0
        && *inner.get_unchecked(INDEX_M21) == 0.0
        && *inner.get_unchecked(INDEX_M22) == 1.0
        && *inner.get_unchecked(INDEX_M23) == 0.0
        && *inner.get_unchecked(INDEX_M24) == 0.0
        && *inner.get_unchecked(INDEX_M31) == 0.0
        && *inner.get_unchecked(INDEX_M32) == 0.0
        && *inner.get_unchecked(INDEX_M33) == 1.0
        && *inner.get_unchecked(INDEX_M34) == 0.0
        && *inner.get_unchecked(INDEX_M41) == 0.0
        && *inner.get_unchecked(INDEX_M42) == 0.0
        && *inner.get_unchecked(INDEX_M43) == 0.0
        && *inner.get_unchecked(INDEX_M44) == 1.0
    }
  }

  #[fast]
  #[getter]
  pub fn is_finite(&self) -> bool {
    self
      .inner
      .borrow()
      .into_iter()
      .all(|&item| item.is_finite())
  }

  #[arraybuffer]
  pub fn to_buffer(&self) -> Vec<u8> {
    // SAFETY: in-range access
    unsafe {
      slice::from_raw_parts(
        self.inner.borrow().as_slice().as_ptr() as *mut u8,
        128,
      )
    }
    .to_vec()
  }

  #[cppgc]
  pub fn translate(&self, tx: f64, ty: f64, tz: f64) -> DOMMatrixInner {
    let out = self.clone();
    matrix_translate(&out, tx, ty, tz);
    out
  }

  #[fast]
  pub fn translate_self(&self, tx: f64, ty: f64, tz: f64) {
    matrix_translate(self, tx, ty, tz);
  }

  #[cppgc]
  pub fn scale_without_origin(
    &self,
    sx: f64,
    sy: f64,
    sz: f64,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_scale(&out, sx, sy, sz);
    out
  }

  #[fast]
  pub fn scale_without_origin_self(&self, sx: f64, sy: f64, sz: f64) {
    matrix_scale(self, sx, sy, sz);
  }

  #[cppgc]
  pub fn scale_with_origin(
    &self,
    sx: f64,
    sy: f64,
    sz: f64,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_scale_with_origin(&out, sx, sy, sz, origin_x, origin_y, origin_z);
    out
  }

  #[fast]
  pub fn scale_with_origin_self(
    &self,
    sx: f64,
    sy: f64,
    sz: f64,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
  ) {
    matrix_scale_with_origin(self, sx, sy, sz, origin_x, origin_y, origin_z);
  }

  #[cppgc]
  pub fn rotate(
    &self,
    roll_deg: f64,
    pitch_deg: f64,
    yaw_deg: f64,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate(&out, roll_deg, pitch_deg, yaw_deg);
    out
  }

  #[fast]
  pub fn rotate_self(&self, roll_deg: f64, pitch_deg: f64, yaw_deg: f64) {
    matrix_rotate(self, roll_deg, pitch_deg, yaw_deg);
  }

  #[cppgc]
  pub fn rotate_from_vector(&self, x: f64, y: f64) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate_from_vector(&out, x, y);
    out
  }

  #[fast]
  pub fn rotate_from_vector_self(&self, x: f64, y: f64) {
    matrix_rotate_from_vector(self, x, y);
  }

  #[cppgc]
  pub fn rotate_axis_angle(
    &self,
    x: f64,
    y: f64,
    z: f64,
    angle_deg: f64,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate_axis_angle(&out, x, y, z, angle_deg);
    out
  }

  #[fast]
  pub fn rotate_axis_angle_self(&self, x: f64, y: f64, z: f64, angle_deg: f64) {
    matrix_rotate_axis_angle(self, x, y, z, angle_deg);
  }

  #[cppgc]
  pub fn skew_x(&self, x_deg: f64) -> DOMMatrixInner {
    let out = self.clone();
    matrix_skew_x(&out, x_deg);
    out
  }

  #[fast]
  pub fn skew_x_self(&self, x_deg: f64) {
    matrix_skew_x(self, x_deg);
  }

  #[cppgc]
  pub fn skew_y(&self, y_deg: f64) -> DOMMatrixInner {
    let out = self.clone();
    matrix_skew_y(&out, y_deg);
    out
  }

  #[fast]
  pub fn skew_y_self(&self, y_deg: f64) {
    matrix_skew_y(self, y_deg);
  }

  #[cppgc]
  pub fn multiply(&self, other: v8::Local<v8::Object>) -> DOMMatrixInner {
    let other = cast_to_matrix(other);
    let out = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&out, self, &other);
    out
  }

  #[fast]
  pub fn multiply_self(&self, other: v8::Local<v8::Object>) {
    let other = cast_to_matrix(other);
    let result = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&result, self, &other);
    self.inner.borrow_mut().copy_from(&result.inner.borrow());
    self.is_2d.set(result.is_2d.get());
  }

  #[fast]
  pub fn pre_multiply_self(&self, other: v8::Local<v8::Object>) {
    let other = cast_to_matrix(other);
    let result = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&result, &other, self);
    self.inner.borrow_mut().copy_from(&result.inner.borrow());
    self.is_2d.set(result.is_2d.get());
  }

  #[cppgc]
  pub fn flip_x(&self) -> DOMMatrixInner {
    let out = self.clone();
    matrix_flip_x(&out);
    out
  }

  #[cppgc]
  pub fn flip_y(&self) -> DOMMatrixInner {
    let out = self.clone();
    matrix_flip_y(&out);
    out
  }

  #[cppgc]
  pub fn inverse(&self) -> DOMMatrixInner {
    let out = self.clone();
    matrix_inverse(&out);
    out
  }

  #[fast]
  pub fn invert_self(&self) {
    matrix_inverse(self);
  }

  #[cppgc]
  pub fn transform_point(&self, point: v8::Local<v8::Object>) -> DOMPointInner {
    let point = cast_to_point(point);
    let out = DOMPointInner {
      x: Cell::new(0.0),
      y: Cell::new(0.0),
      z: Cell::new(0.0),
      w: Cell::new(0.0),
    };
    matrix_transform_point(self, &point, &out);
    out
  }
}

#[op2]
pub fn op_geometry_create_matrix_identity<'a>(
  scope: &mut v8::HandleScope<'a>,
) -> v8::Local<'a, v8::Object> {
  cppgc::make_cppgc_object(
    scope,
    DOMMatrixInner {
      inner: RefCell::new(Matrix4::identity()),
      is_2d: Cell::new(true),
    },
  )
}

#[inline]
fn cast_to_point(
  obj: v8::Local<'_, v8::Object>,
) -> v8::Local<'_, DOMPointInner> {
  // SAFETY: cast v8::Local
  unsafe { mem::transmute(obj) }
}

#[inline]
fn cast_to_matrix(
  obj: v8::Local<'_, v8::Object>,
) -> v8::Local<'_, DOMMatrixInner> {
  // SAFETY: cast v8::Local
  unsafe { mem::transmute(obj) }
}

#[inline]
fn matrix_translate(matrix: &DOMMatrixInner, tx: f64, ty: f64, tz: f64) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let shift = Vector3::new(tx, ty, tz);
  inner.prepend_translation_mut(&shift);
  matrix.is_2d.set(is_2d && tz == 0.0);
}

#[inline]
fn matrix_scale(matrix: &DOMMatrixInner, sx: f64, sy: f64, sz: f64) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let scaling = Vector3::new(sx, sy, sz);
  inner.prepend_nonuniform_scaling_mut(&scaling);
  matrix.is_2d.set(is_2d && sz == 0.0);
}

#[inline]
fn matrix_scale_with_origin(
  matrix: &DOMMatrixInner,
  sx: f64,
  sy: f64,
  sz: f64,
  origin_x: f64,
  origin_y: f64,
  origin_z: f64,
) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let scaling = Vector3::new(sx, sy, sz);
  let mut shift = Vector3::new(origin_x, origin_y, origin_z);
  inner.prepend_translation_mut(&shift);
  inner.prepend_nonuniform_scaling_mut(&scaling);
  shift.neg_mut();
  inner.prepend_translation_mut(&shift);
  matrix.is_2d.set(is_2d && sz == 0.0 && origin_z == 0.0);
}

#[inline]
fn matrix_rotate(
  matrix: &DOMMatrixInner,
  roll_deg: f64,
  pitch_deg: f64,
  yaw_deg: f64,
) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let rotation = Rotation3::from_euler_angles(
    roll_deg.to_radians(),
    pitch_deg.to_radians(),
    yaw_deg.to_radians(),
  )
  .to_homogeneous();
  let mut result = Matrix4x3::zeros();
  inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inner.set_column(0, &result.column(0));
  inner.set_column(1, &result.column(1));
  inner.set_column(2, &result.column(2));
  matrix
    .is_2d
    .set(is_2d && pitch_deg == 0.0 && yaw_deg == 0.0);
}

#[inline]
fn matrix_rotate_from_vector(matrix: &DOMMatrixInner, x: f64, y: f64) {
  let mut inner = matrix.inner.borrow_mut();
  let rotation =
    Rotation3::from_axis_angle(&Vector3::z_axis(), y.atan2(x)).to_homogeneous();
  let mut result = Matrix4x3::zeros();
  inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inner.set_column(0, &result.column(0));
  inner.set_column(1, &result.column(1));
  inner.set_column(2, &result.column(2));
}

#[inline]
fn matrix_rotate_axis_angle(
  matrix: &DOMMatrixInner,
  x: f64,
  y: f64,
  z: f64,
  angle_deg: f64,
) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let rotation = Rotation3::from_axis_angle(
    &UnitVector3::new_normalize(Vector3::new(x, y, z)),
    angle_deg.to_radians(),
  )
  .to_homogeneous();
  let mut result = Matrix4x3::zeros();
  inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inner.set_column(0, &result.column(0));
  inner.set_column(1, &result.column(1));
  inner.set_column(2, &result.column(2));
  matrix.is_2d.set(is_2d && x == 0.0 && y == 0.0);
}

#[inline]
fn matrix_skew_x(matrix: &DOMMatrixInner, x_deg: f64) {
  let mut inner = matrix.inner.borrow_mut();
  let skew =
    Matrix4x2::new(1.0, x_deg.to_radians().tan(), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
  let mut result = Matrix4x2::zeros();
  inner.mul_to(&skew, &mut result);
  inner.set_column(0, &result.column(0));
  inner.set_column(1, &result.column(1));
}

#[inline]
fn matrix_skew_y(matrix: &DOMMatrixInner, y_deg: f64) {
  let mut inner = matrix.inner.borrow_mut();
  let skew =
    Matrix4x2::new(1.0, 0.0, y_deg.to_radians().tan(), 1.0, 0.0, 0.0, 0.0, 0.0);
  let mut result = Matrix4x2::zeros();
  inner.mul_to(&skew, &mut result);
  inner.set_column(0, &result.column(0));
  inner.set_column(1, &result.column(1));
}

#[inline]
fn matrix_multiply(
  out: &DOMMatrixInner,
  lhs: &DOMMatrixInner,
  rhs: &DOMMatrixInner,
) {
  let lhs_inner = lhs.inner.borrow();
  let lhs_is_2d = lhs.is_2d.get();
  let rhs_inner = rhs.inner.borrow();
  let rhs_is_2d = rhs.is_2d.get();
  let mut out_inner = out.inner.borrow_mut();
  let mut result = Matrix4::zeros();
  lhs_inner.mul_to(&rhs_inner, &mut result);
  out_inner.copy_from(&result);
  out.is_2d.set(lhs_is_2d && rhs_is_2d);
}

#[inline]
fn matrix_flip_x(matrix: &DOMMatrixInner) {
  let mut inner = matrix.inner.borrow_mut();
  inner.column_mut(0).neg_mut();
}

#[inline]
fn matrix_flip_y(matrix: &DOMMatrixInner) {
  let mut inner = matrix.inner.borrow_mut();
  inner.column_mut(1).neg_mut();
}

#[inline]
fn matrix_inverse(matrix: &DOMMatrixInner) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  if inner.iter().any(|&x| x.is_infinite()) {
    inner.fill(f64::NAN);
    matrix.is_2d.set(false);
    return;
  }
  if is_2d {
    // SAFETY: in-range access
    let mut matrix3 = unsafe {
      Matrix3::new(
        *inner.get_unchecked(0),
        *inner.get_unchecked(4),
        *inner.get_unchecked(12),
        *inner.get_unchecked(1),
        *inner.get_unchecked(5),
        *inner.get_unchecked(13),
        0.0,
        0.0,
        1.0,
      )
    };
    if !matrix3.try_inverse_mut() {
      inner.fill(f64::NAN);
      matrix.is_2d.set(false);
      return;
    }
    // SAFETY: in-range access
    unsafe {
      *inner.get_unchecked_mut(0) = *matrix3.get_unchecked(0);
      *inner.get_unchecked_mut(1) = *matrix3.get_unchecked(1);
      *inner.get_unchecked_mut(4) = *matrix3.get_unchecked(3);
      *inner.get_unchecked_mut(5) = *matrix3.get_unchecked(4);
      *inner.get_unchecked_mut(12) = *matrix3.get_unchecked(6);
      *inner.get_unchecked_mut(13) = *matrix3.get_unchecked(7);
    }
  } else if !inner.try_inverse_mut() {
    inner.fill(f64::NAN);
  }
}

fn matrix_transform_point(
  matrix: &DOMMatrixInner,
  point: &DOMPointInner,
  out: &DOMPointInner,
) {
  let inner = matrix.inner.borrow();
  let point =
    Vector4::new(point.x.get(), point.y.get(), point.z.get(), point.w.get());
  let mut result = Vector4::zeros();
  inner.mul_to(&point, &mut result);
  out.x.set(result.x);
  out.y.set(result.y);
  out.z.set(result.z);
  out.w.set(result.w);
}
