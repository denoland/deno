// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::path::PathBuf;
use std::slice;

use deno_core::cppgc;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl;
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
  objects = [DOMPointInner, DOMRectInner, DOMQuadInner, DOMMatrixInner],
  esm = ["00_init.js"],
  lazy_loaded_esm = ["01_geometry.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_geometry.d.ts")
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum GeometryError {
  #[class(type)]
  #[error("Inconsistent 2d matrix value")]
  Inconsistent2DMatrix,
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMPointInit {
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  x: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  y: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  z: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(1.0))]
  w: webidl::UnrestrictedDouble,
}

#[derive(Debug)]
pub struct DOMPointInner {
  inner: RefCell<Vector4<f64>>,
}

impl GarbageCollected for DOMPointInner {}

#[op2]
impl DOMPointInner {
  #[constructor]
  #[cppgc]
  pub fn constructor(
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
    #[webidl] z: webidl::UnrestrictedDouble,
    #[webidl] w: webidl::UnrestrictedDouble,
  ) -> DOMPointInner {
    DOMPointInner {
      inner: RefCell::new(Vector4::new(*x, *y, *z, *w)),
    }
  }

  #[reentrant]
  #[static_method]
  #[cppgc]
  pub fn from_point(#[webidl] init: DOMPointInit) -> DOMPointInner {
    DOMPointInner {
      inner: RefCell::new(Vector4::new(*init.x, *init.y, *init.z, *init.w)),
    }
  }

  #[fast]
  #[getter]
  pub fn x(&self) -> f64 {
    self.inner.borrow().x
  }

  #[setter]
  pub fn x(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.inner.borrow_mut().x = *value
  }

  #[fast]
  #[getter]
  pub fn y(&self) -> f64 {
    self.inner.borrow().y
  }

  #[setter]
  pub fn y(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.inner.borrow_mut().y = *value
  }

  #[fast]
  #[getter]
  pub fn z(&self) -> f64 {
    self.inner.borrow().z
  }

  #[setter]
  pub fn z(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.inner.borrow_mut().z = *value
  }

  #[fast]
  #[getter]
  pub fn w(&self) -> f64 {
    self.inner.borrow().w
  }

  #[setter]
  pub fn w(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.inner.borrow_mut().w = *value
  }

  #[cppgc]
  pub fn matrix_transform(
    &self,
    #[cppgc] matrix: &DOMMatrixInner,
  ) -> DOMPointInner {
    let out = DOMPointInner {
      inner: RefCell::new(Vector4::zeros()),
    };
    matrix_transform_point(matrix, self, &out);
    out
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMRectInit {
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  x: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  y: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  width: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  height: webidl::UnrestrictedDouble,
}

#[derive(Debug)]
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
  pub fn constructor(
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
    #[webidl] width: webidl::UnrestrictedDouble,
    #[webidl] height: webidl::UnrestrictedDouble,
  ) -> DOMRectInner {
    DOMRectInner {
      x: Cell::new(*x),
      y: Cell::new(*y),
      width: Cell::new(*width),
      height: Cell::new(*height),
    }
  }

  #[reentrant]
  #[static_method]
  #[cppgc]
  pub fn from_rect(#[webidl] init: DOMRectInit) -> DOMRectInner {
    DOMRectInner {
      x: Cell::new(*init.x),
      y: Cell::new(*init.y),
      width: Cell::new(*init.width),
      height: Cell::new(*init.height),
    }
  }

  #[fast]
  #[getter]
  pub fn x(&self) -> f64 {
    self.x.get()
  }

  #[setter]
  pub fn x(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.x.set(*value)
  }

  #[fast]
  #[getter]
  pub fn y(&self) -> f64 {
    self.y.get()
  }

  #[setter]
  pub fn y(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.y.set(*value)
  }

  #[fast]
  #[getter]
  pub fn width(&self) -> f64 {
    self.width.get()
  }

  #[setter]
  pub fn width(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.width.set(*value)
  }

  #[fast]
  #[getter]
  pub fn height(&self) -> f64 {
    self.height.get()
  }

  #[setter]
  pub fn height(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    self.height.set(*value)
  }

  #[fast]
  #[getter]
  pub fn top(&self) -> f64 {
    let y = self.y.get();
    let height = self.height.get();
    minimum(y, y + height)
  }

  #[fast]
  #[getter]
  pub fn right(&self) -> f64 {
    let x = self.x.get();
    let width = self.width.get();
    maximum(x, x + width)
  }

  #[fast]
  #[getter]
  pub fn bottom(&self) -> f64 {
    let y = self.y.get();
    let height = self.height.get();
    maximum(y, y + height)
  }

  #[fast]
  #[getter]
  pub fn left(&self) -> f64 {
    let x = self.x.get();
    let width = self.width.get();
    minimum(x, x + width)
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

pub struct DOMQuadInner {
  p1: SameObject<DOMPointInner>,
  p2: SameObject<DOMPointInner>,
  p3: SameObject<DOMPointInner>,
  p4: SameObject<DOMPointInner>,
}

impl GarbageCollected for DOMQuadInner {}

#[op2]
impl DOMQuadInner {
  #[constructor]
  #[reentrant]
  #[cppgc]
  pub fn constructor(
    scope: &mut v8::HandleScope,
    #[webidl] p1: DOMPointInit,
    #[webidl] p2: DOMPointInit,
    #[webidl] p3: DOMPointInit,
    #[webidl] p4: DOMPointInit,
  ) -> DOMQuadInner {
    #[inline]
    fn from_point(
      scope: &mut v8::HandleScope,
      point: DOMPointInit,
    ) -> SameObject<DOMPointInner> {
      let obj = SameObject::new();
      obj.get(scope, |_| DOMPointInner {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      });
      obj
    }

    DOMQuadInner {
      p1: from_point(scope, p1),
      p2: from_point(scope, p2),
      p3: from_point(scope, p3),
      p4: from_point(scope, p4),
    }
  }

  #[reentrant]
  #[static_method]
  #[cppgc]
  pub fn from_rect(
    scope: &mut v8::HandleScope,
    #[webidl] rect: DOMRectInit,
  ) -> DOMQuadInner {
    #[inline]
    fn create_point(
      scope: &mut v8::HandleScope,
      x: f64,
      y: f64,
      z: f64,
      w: f64,
    ) -> SameObject<DOMPointInner> {
      let obj = SameObject::new();
      obj.get(scope, |_| DOMPointInner {
        inner: RefCell::new(Vector4::new(x, y, z, w)),
      });
      obj
    }

    let DOMRectInit {
      x,
      y,
      width,
      height,
    } = rect;
    DOMQuadInner {
      p1: create_point(scope, *x, *y, 0.0, 1.0),
      p2: create_point(scope, *x + *width, *y, 0.0, 1.0),
      p3: create_point(scope, *x + *width, *y + *height, 0.0, 1.0),
      p4: create_point(scope, *x, *y + *height, 0.0, 1.0),
    }
  }

  #[reentrant]
  #[static_method]
  #[cppgc]
  pub fn from_quad(
    scope: &mut v8::HandleScope,
    #[webidl] quad: DOMQuadInit,
  ) -> DOMQuadInner {
    #[inline]
    fn from_point(
      scope: &mut v8::HandleScope,
      point: DOMPointInit,
    ) -> SameObject<DOMPointInner> {
      let obj = SameObject::new();
      obj.get(scope, |_| DOMPointInner {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      });
      obj
    }

    DOMQuadInner {
      p1: from_point(scope, quad.p1),
      p2: from_point(scope, quad.p2),
      p3: from_point(scope, quad.p3),
      p4: from_point(scope, quad.p4),
    }
  }

  #[getter]
  #[global]
  pub fn p1(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.p1.get(scope, |_| unreachable!())
  }

  #[getter]
  #[global]
  pub fn p2(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.p2.get(scope, |_| unreachable!())
  }

  #[getter]
  #[global]
  pub fn p3(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.p3.get(scope, |_| unreachable!())
  }

  #[getter]
  #[global]
  pub fn p4(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.p4.get(scope, |_| unreachable!())
  }

  #[cppgc]
  pub fn get_bounds(&self, scope: &mut v8::HandleScope) -> DOMRectInner {
    #[inline]
    fn get_ptr(
      scope: &mut v8::HandleScope,
      value: &SameObject<DOMPointInner>,
    ) -> cppgc::Ptr<DOMPointInner> {
      let value = value.get(scope, |_| unreachable!());
      let value = v8::Local::new(scope, value);
      cppgc::try_unwrap_cppgc_object::<DOMPointInner>(scope, value.cast())
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
    DOMRectInner {
      x: Cell::new(left),
      y: Cell::new(top),
      width: Cell::new(right - left),
      height: Cell::new(bottom - top),
    }
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct DOMMatrixInit {
  #[webidl(default = None)]
  a: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  b: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  c: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  d: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  e: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  f: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  m11: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  m12: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m13: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m14: webidl::UnrestrictedDouble,
  #[webidl(default = None)]
  m21: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  m22: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m23: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m24: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m31: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m32: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(1.0))]
  m33: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m34: webidl::UnrestrictedDouble,
  #[webidl(default = None)]
  m41: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = None)]
  m42: Option<webidl::UnrestrictedDouble>,
  #[webidl(default = webidl::UnrestrictedDouble(0.0))]
  m43: webidl::UnrestrictedDouble,
  #[webidl(default = webidl::UnrestrictedDouble(1.0))]
  m44: webidl::UnrestrictedDouble,
  #[webidl(default = None)]
  is_2d: Option<bool>,
}

#[derive(Debug, Clone)]
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

  #[reentrant]
  #[static_method]
  #[cppgc]
  pub fn from_matrix(
    #[webidl] init: DOMMatrixInit,
  ) -> Result<DOMMatrixInner, GeometryError> {
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
          webidl::UnrestrictedDouble($default)
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
      Ok(DOMMatrixInner {
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
      Ok(DOMMatrixInner {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
          *m11, *m21, *m31, *m41,
          *m12, *m22, *m32, *m42,
          *m13, *m23, *m33, *m43,
          *m14, *m24, *m34, *m44,
        )),
        is_2d: Cell::new(false),
      })
    }
  }

  #[static_method]
  #[cppgc]
  pub fn identity() -> DOMMatrixInner {
    DOMMatrixInner {
      inner: RefCell::new(Matrix4::identity()),
      is_2d: Cell::new(true),
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

  #[setter]
  pub fn a(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_A) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn b(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_B) }
  }

  #[setter]
  pub fn b(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_B) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn c(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_C) }
  }

  #[setter]
  pub fn c(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_C) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn d(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_D) }
  }

  #[setter]
  pub fn d(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_D) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn e(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_E) }
  }

  #[setter]
  pub fn e(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_E) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn f(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_F) }
  }

  #[setter]
  pub fn f(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_F) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m11(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M11) }
  }

  #[setter]
  pub fn m11(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M11) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m12(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M12) }
  }

  #[setter]
  pub fn m12(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M12) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m13(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M13) }
  }

  #[setter]
  pub fn m13(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M13) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m14(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M14) }
  }

  #[setter]
  pub fn m14(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M14) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m21(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M21) }
  }

  #[setter]
  pub fn m21(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M21) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m22(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M22) }
  }

  #[setter]
  pub fn m22(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M22) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m23(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M23) }
  }

  #[setter]
  pub fn m23(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M23) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m24(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M24) }
  }

  #[setter]
  pub fn m24(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M24) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m31(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M31) }
  }

  #[setter]
  pub fn m31(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M31) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m32(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M32) }
  }

  #[setter]
  pub fn m32(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M32) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m33(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M33) }
  }

  #[setter]
  pub fn m33(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M33) = *value;
    }
    if *value != 1.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m34(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M34) }
  }

  #[setter]
  pub fn m34(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M34) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m41(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M41) }
  }

  #[setter]
  pub fn m41(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M41) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m42(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M42) }
  }

  #[setter]
  pub fn m42(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M42) = *value;
    }
  }

  #[fast]
  #[getter]
  pub fn m43(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M43) }
  }

  #[setter]
  pub fn m43(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M43) = *value;
    }
    if *value != 0.0 {
      self.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  pub fn m44(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M44) }
  }

  #[setter]
  pub fn m44(&self, #[webidl] value: webidl::UnrestrictedDouble) {
    // SAFETY: in-range access
    unsafe {
      *self.inner.borrow_mut().get_unchecked_mut(INDEX_M44) = *value;
    }
    if *value != 1.0 {
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
        mem::size_of::<f64>() * 16,
      )
    }
    .to_vec()
  }

  #[cppgc]
  pub fn translate(
    &self,
    #[webidl] tx: webidl::UnrestrictedDouble,
    #[webidl] ty: webidl::UnrestrictedDouble,
    #[webidl] tz: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_translate(&out, *tx, *ty, *tz);
    out
  }

  pub fn translate_self(
    &self,
    #[webidl] tx: webidl::UnrestrictedDouble,
    #[webidl] ty: webidl::UnrestrictedDouble,
    #[webidl] tz: webidl::UnrestrictedDouble,
  ) {
    matrix_translate(self, *tx, *ty, *tz);
  }

  #[cppgc]
  pub fn scale_without_origin(
    &self,
    #[webidl] sx: webidl::UnrestrictedDouble,
    #[webidl] sy: webidl::UnrestrictedDouble,
    #[webidl] sz: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_scale_without_origin(&out, *sx, *sy, *sz);
    out
  }

  pub fn scale_without_origin_self(
    &self,
    #[webidl] sx: webidl::UnrestrictedDouble,
    #[webidl] sy: webidl::UnrestrictedDouble,
    #[webidl] sz: webidl::UnrestrictedDouble,
  ) {
    matrix_scale_without_origin(self, *sx, *sy, *sz);
  }

  #[cppgc]
  pub fn scale_with_origin(
    &self,
    #[webidl] sx: webidl::UnrestrictedDouble,
    #[webidl] sy: webidl::UnrestrictedDouble,
    #[webidl] sz: webidl::UnrestrictedDouble,
    #[webidl] origin_x: webidl::UnrestrictedDouble,
    #[webidl] origin_y: webidl::UnrestrictedDouble,
    #[webidl] origin_z: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_scale_with_origin(
      &out, *sx, *sy, *sz, *origin_x, *origin_y, *origin_z,
    );
    out
  }

  pub fn scale_with_origin_self(
    &self,
    #[webidl] sx: webidl::UnrestrictedDouble,
    #[webidl] sy: webidl::UnrestrictedDouble,
    #[webidl] sz: webidl::UnrestrictedDouble,
    #[webidl] origin_x: webidl::UnrestrictedDouble,
    #[webidl] origin_y: webidl::UnrestrictedDouble,
    #[webidl] origin_z: webidl::UnrestrictedDouble,
  ) {
    matrix_scale_with_origin(
      self, *sx, *sy, *sz, *origin_x, *origin_y, *origin_z,
    );
  }

  #[cppgc]
  pub fn rotate(
    &self,
    #[webidl] roll_deg: webidl::UnrestrictedDouble,
    #[webidl] pitch_deg: webidl::UnrestrictedDouble,
    #[webidl] yaw_deg: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate(&out, *roll_deg, *pitch_deg, *yaw_deg);
    out
  }

  pub fn rotate_self(
    &self,
    #[webidl] roll_deg: webidl::UnrestrictedDouble,
    #[webidl] pitch_deg: webidl::UnrestrictedDouble,
    #[webidl] yaw_deg: webidl::UnrestrictedDouble,
  ) {
    matrix_rotate(self, *roll_deg, *pitch_deg, *yaw_deg);
  }

  #[cppgc]
  pub fn rotate_from_vector(
    &self,
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate_from_vector(&out, *x, *y);
    out
  }

  pub fn rotate_from_vector_self(
    &self,
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
  ) {
    matrix_rotate_from_vector(self, *x, *y);
  }

  #[cppgc]
  pub fn rotate_axis_angle(
    &self,
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
    #[webidl] z: webidl::UnrestrictedDouble,
    #[webidl] angle_deg: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_rotate_axis_angle(&out, *x, *y, *z, *angle_deg);
    out
  }

  pub fn rotate_axis_angle_self(
    &self,
    #[webidl] x: webidl::UnrestrictedDouble,
    #[webidl] y: webidl::UnrestrictedDouble,
    #[webidl] z: webidl::UnrestrictedDouble,
    #[webidl] angle_deg: webidl::UnrestrictedDouble,
  ) {
    matrix_rotate_axis_angle(self, *x, *y, *z, *angle_deg);
  }

  #[cppgc]
  pub fn skew_x(
    &self,
    #[webidl] x_deg: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_skew_x(&out, *x_deg);
    out
  }

  pub fn skew_x_self(&self, #[webidl] x_deg: webidl::UnrestrictedDouble) {
    matrix_skew_x(self, *x_deg);
  }

  #[cppgc]
  pub fn skew_y(
    &self,
    #[webidl] y_deg: webidl::UnrestrictedDouble,
  ) -> DOMMatrixInner {
    let out = self.clone();
    matrix_skew_y(&out, *y_deg);
    out
  }

  pub fn skew_y_self(&self, #[webidl] y_deg: webidl::UnrestrictedDouble) {
    matrix_skew_y(self, *y_deg);
  }

  #[cppgc]
  pub fn multiply(&self, #[cppgc] other: &DOMMatrixInner) -> DOMMatrixInner {
    let out = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&out, self, other);
    out
  }

  #[fast]
  pub fn multiply_self(&self, #[cppgc] other: &DOMMatrixInner) {
    let result = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&result, self, other);
    self.inner.borrow_mut().copy_from(&result.inner.borrow());
    self.is_2d.set(result.is_2d.get());
  }

  #[fast]
  pub fn pre_multiply_self(&self, #[cppgc] other: &DOMMatrixInner) {
    let result = DOMMatrixInner {
      inner: RefCell::new(Matrix4::zeros()),
      is_2d: Cell::new(true),
    };
    matrix_multiply(&result, other, self);
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
  pub fn transform_point(
    &self,
    #[cppgc] point: &DOMPointInner,
  ) -> DOMPointInner {
    let out = DOMPointInner {
      inner: RefCell::new(Vector4::zeros()),
    };
    matrix_transform_point(self, point, &out);
    out
  }
}

// TODO(petamoriken) Use f64::maximum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
fn maximum(a: f64, b: f64) -> f64 {
  if a > b {
    a
  } else if b > a {
    b
  } else if a == b {
    if a.is_sign_positive() && b.is_sign_negative() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
}

// TODO(petamoriken) Use f64::minimum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
fn minimum(a: f64, b: f64) -> f64 {
  if a < b {
    a
  } else if b < a {
    b
  } else if a == b {
    if a.is_sign_negative() && b.is_sign_positive() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
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
fn matrix_scale_without_origin(
  matrix: &DOMMatrixInner,
  sx: f64,
  sy: f64,
  sz: f64,
) {
  let mut inner = matrix.inner.borrow_mut();
  let is_2d = matrix.is_2d.get();
  let scaling = Vector3::new(sx, sy, sz);
  inner.prepend_nonuniform_scaling_mut(&scaling);
  matrix.is_2d.set(is_2d && sz == 1.0);
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
  matrix.is_2d.set(is_2d && sz == 1.0 && origin_z == 0.0);
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
    .set(is_2d && roll_deg == 0.0 && pitch_deg == 0.0);
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
  if x == 0.0 && y == 0.0 && z == 0.0 {
    return;
  }
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
  lhs_inner.mul_to(&rhs_inner, &mut out_inner);
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
        *inner.get_unchecked(INDEX_A),
        *inner.get_unchecked(INDEX_C),
        *inner.get_unchecked(INDEX_E),
        *inner.get_unchecked(INDEX_B),
        *inner.get_unchecked(INDEX_D),
        *inner.get_unchecked(INDEX_F),
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
      *inner.get_unchecked_mut(INDEX_A) = *matrix3.get_unchecked(0);
      *inner.get_unchecked_mut(INDEX_B) = *matrix3.get_unchecked(1);
      *inner.get_unchecked_mut(INDEX_C) = *matrix3.get_unchecked(3);
      *inner.get_unchecked_mut(INDEX_D) = *matrix3.get_unchecked(4);
      *inner.get_unchecked_mut(INDEX_E) = *matrix3.get_unchecked(6);
      *inner.get_unchecked_mut(INDEX_F) = *matrix3.get_unchecked(7);
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
  let point = point.inner.borrow();
  let mut result = out.inner.borrow_mut();
  inner.mul_to(&point, &mut result);
}
