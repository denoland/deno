// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::too_many_arguments)]

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::ptr;
use std::slice;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use lightningcss::properties::transform::Matrix as CSSMatrix;
use lightningcss::properties::transform::Matrix3d as CSSMatrix3d;
use lightningcss::properties::transform::Transform;
use lightningcss::properties::transform::TransformList;
use lightningcss::traits::Parse;
use lightningcss::values::length::LengthPercentage;
use lightningcss::values::number::CSSNumber;
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
  ops = [
    op_geometry_get_enable_window_features,
    op_geometry_matrix_set_matrix_value,
    op_geometry_matrix_to_buffer,
    op_geometry_matrix_to_string,
  ],
  objects = [
    DOMPointReadOnly,
    DOMPoint,
    DOMRectReadOnly,
    DOMRect,
    DOMQuad,
    DOMMatrixReadOnly,
    DOMMatrix,
  ],
  esm = ["00_init.js"],
  lazy_loaded_esm = ["01_geometry.js"],
  options = {
    enable_window_features: bool,
  },
  state = |state, options| {
    state.put(State::new(options.enable_window_features));
  },
);

struct State {
  enable_window_features: bool,
}

impl State {
  fn new(enable_window_features: bool) -> Self {
    Self {
      enable_window_features,
    }
  }
}

#[op2(fast)]
fn op_geometry_get_enable_window_features(state: &mut OpState) -> bool {
  let state = state.borrow_mut::<State>();
  state.enable_window_features
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
  #[class(type)]
  #[error("Mismatched types")]
  TypeMismatch,
  #[class("DOMExceptionInvalidStateError")]
  #[error("Cannot be serialized with NaN or Infinity values")]
  InvalidState,
  #[class(type)]
  #[error("Cannot parse a CSS <transform-list> value on Workers")]
  DisallowWindowFeatures,
  #[class("DOMExceptionSyntaxError")]
  #[error("Failed to parse the string as CSS <transform-list> value")]
  FailedToParse,
  #[class("DOMExceptionSyntaxError")]
  #[error("The CSS <transform-list> value contains relative values")]
  ContainsRelativeValue,
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
pub struct DOMPointReadOnly {
  inner: RefCell<Vector4<f64>>,
}

impl GarbageCollected for DOMPointReadOnly {
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
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] z: Option<webidl::UnrestrictedDouble>,
    #[webidl] w: Option<webidl::UnrestrictedDouble>,
  ) -> DOMPointReadOnly {
    DOMPointReadOnly {
      inner: RefCell::new(Vector4::new(
        *x.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *y.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *z.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *w.unwrap_or(webidl::UnrestrictedDouble(1.0)),
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
  fn to_json<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let mut obj = v8::Object::new(scope);
    set_f64(scope, &mut obj, "x", self.inner.borrow().x);
    set_f64(scope, &mut obj, "y", self.inner.borrow().y);
    set_f64(scope, &mut obj, "z", self.inner.borrow().z);
    set_f64(scope, &mut obj, "w", self.inner.borrow().w);
    obj
  }

  #[reentrant]
  #[required(0)]
  fn matrix_transform<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] value: DOMMatrixInit,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let matrix = DOMMatrixReadOnly::from_matrix_inner(&value)?;
    let ro = DOMPointReadOnly {
      inner: RefCell::new(Vector4::zeros()),
    };
    matrix_transform_point(&matrix, self, &ro);
    let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
    Ok(cppgc::wrap_object2(scope, obj, (ro, DOMPoint {})))
  }
}

pub struct DOMPoint {}

impl GarbageCollected for DOMPoint {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMPoint"
  }
}

#[op2(inherit = DOMPointReadOnly)]
impl DOMPoint {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] z: Option<webidl::UnrestrictedDouble>,
    #[webidl] w: Option<webidl::UnrestrictedDouble>,
  ) -> (DOMPointReadOnly, DOMPoint) {
    let ro = DOMPointReadOnly {
      inner: RefCell::new(Vector4::new(
        *x.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *y.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *z.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        *w.unwrap_or(webidl::UnrestrictedDouble(1.0)),
      )),
    };
    (ro, DOMPoint {})
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_point<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] init: DOMPointInit,
  ) -> v8::Local<'a, v8::Object> {
    let ro = DOMPointReadOnly::from_point_inner(init);
    let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
    cppgc::wrap_object2(scope, obj, (ro, DOMPoint {}))
  }

  #[fast]
  #[getter]
  fn x(&self, #[proto] ro: &DOMPointReadOnly) -> f64 {
    ro.inner.borrow().x
  }

  #[setter]
  fn x(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMPointReadOnly,
  ) {
    ro.inner.borrow_mut().x = *value
  }

  #[fast]
  #[getter]
  fn y(&self, #[proto] ro: &DOMPointReadOnly) -> f64 {
    ro.inner.borrow().y
  }

  #[setter]
  fn y(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMPointReadOnly,
  ) {
    ro.inner.borrow_mut().y = *value
  }

  #[fast]
  #[getter]
  fn z(&self, #[proto] ro: &DOMPointReadOnly) -> f64 {
    ro.inner.borrow().z
  }

  #[setter]
  fn z(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMPointReadOnly,
  ) {
    ro.inner.borrow_mut().z = *value
  }

  #[fast]
  #[getter]
  fn w(&self, #[proto] ro: &DOMPointReadOnly) -> f64 {
    ro.inner.borrow().w
  }

  #[setter]
  fn w(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMPointReadOnly,
  ) {
    ro.inner.borrow_mut().w = *value
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
pub struct DOMRectReadOnly {
  x: Cell<f64>,
  y: Cell<f64>,
  width: Cell<f64>,
  height: Cell<f64>,
}

impl GarbageCollected for DOMRectReadOnly {
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
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] width: Option<webidl::UnrestrictedDouble>,
    #[webidl] height: Option<webidl::UnrestrictedDouble>,
  ) -> DOMRectReadOnly {
    DOMRectReadOnly {
      x: Cell::new(*x.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      y: Cell::new(*y.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      width: Cell::new(*width.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      height: Cell::new(*height.unwrap_or(webidl::UnrestrictedDouble(0.0))),
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
  fn to_json<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let mut obj = v8::Object::new(scope);
    set_f64(scope, &mut obj, "x", self.x.get());
    set_f64(scope, &mut obj, "y", self.y.get());
    set_f64(scope, &mut obj, "width", self.width.get());
    set_f64(scope, &mut obj, "height", self.height.get());
    set_f64(scope, &mut obj, "top", self.get_top());
    set_f64(scope, &mut obj, "right", self.get_right());
    set_f64(scope, &mut obj, "bottom", self.get_bottom());
    set_f64(scope, &mut obj, "left", self.get_left());
    obj
  }
}

pub struct DOMRect {}

impl GarbageCollected for DOMRect {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMRect"
  }
}

#[op2(inherit = DOMRectReadOnly)]
impl DOMRect {
  #[constructor]
  #[required(0)]
  #[cppgc]
  fn constructor(
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] width: Option<webidl::UnrestrictedDouble>,
    #[webidl] height: Option<webidl::UnrestrictedDouble>,
  ) -> (DOMRectReadOnly, DOMRect) {
    let ro = DOMRectReadOnly {
      x: Cell::new(*x.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      y: Cell::new(*y.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      width: Cell::new(*width.unwrap_or(webidl::UnrestrictedDouble(0.0))),
      height: Cell::new(*height.unwrap_or(webidl::UnrestrictedDouble(0.0))),
    };
    (ro, DOMRect {})
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_rect<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] init: DOMRectInit,
  ) -> v8::Local<'a, v8::Object> {
    let ro = DOMRectReadOnly::from_rect_inner(init);
    let obj = cppgc::make_cppgc_empty_object::<DOMRect>(scope);
    cppgc::wrap_object2(scope, obj, (ro, DOMRect {}))
  }

  #[fast]
  #[getter]
  fn x(&self, #[proto] ro: &DOMRectReadOnly) -> f64 {
    ro.x.get()
  }

  #[setter]
  fn x(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMRectReadOnly,
  ) {
    ro.x.set(*value)
  }

  #[fast]
  #[getter]
  fn y(&self, #[proto] ro: &DOMRectReadOnly) -> f64 {
    ro.y.get()
  }

  #[setter]
  fn y(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMRectReadOnly,
  ) {
    ro.y.set(*value)
  }

  #[fast]
  #[getter]
  fn width(&self, #[proto] ro: &DOMRectReadOnly) -> f64 {
    ro.width.get()
  }

  #[setter]
  fn width(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMRectReadOnly,
  ) {
    ro.width.set(*value)
  }

  #[fast]
  #[getter]
  fn height(&self, #[proto] ro: &DOMRectReadOnly) -> f64 {
    ro.height.get()
  }

  #[setter]
  fn height(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMRectReadOnly,
  ) {
    ro.height.set(*value)
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
  p1: v8::Global<v8::Object>,
  p2: v8::Global<v8::Object>,
  p3: v8::Global<v8::Object>,
  p4: v8::Global<v8::Object>,
}

impl GarbageCollected for DOMQuad {
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
    scope: &mut v8::HandleScope,
    #[webidl] p1: DOMPointInit,
    #[webidl] p2: DOMPointInit,
    #[webidl] p3: DOMPointInit,
    #[webidl] p4: DOMPointInit,
  ) -> DOMQuad {
    #[inline]
    fn from_point(
      scope: &mut v8::HandleScope,
      point: DOMPointInit,
    ) -> v8::Global<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object2(scope, obj, (ro, DOMPoint {}));
      v8::Global::new(scope, obj)
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
    scope: &mut v8::HandleScope,
    #[webidl] rect: DOMRectInit,
  ) -> DOMQuad {
    #[inline]
    fn create_point(
      scope: &mut v8::HandleScope,
      x: f64,
      y: f64,
      z: f64,
      w: f64,
    ) -> v8::Global<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(x, y, z, w)),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object2(scope, obj, (ro, DOMPoint {}));
      v8::Global::new(scope, obj)
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
    scope: &mut v8::HandleScope,
    #[webidl] quad: DOMQuadInit,
  ) -> DOMQuad {
    #[inline]
    fn from_point(
      scope: &mut v8::HandleScope,
      point: DOMPointInit,
    ) -> v8::Global<v8::Object> {
      let ro = DOMPointReadOnly {
        inner: RefCell::new(Vector4::new(
          *point.x, *point.y, *point.z, *point.w,
        )),
      };
      let obj = cppgc::make_cppgc_empty_object::<DOMPoint>(scope);
      cppgc::wrap_object2(scope, obj, (ro, DOMPoint {}));
      v8::Global::new(scope, obj)
    }

    DOMQuad {
      p1: from_point(scope, quad.p1),
      p2: from_point(scope, quad.p2),
      p3: from_point(scope, quad.p3),
      p4: from_point(scope, quad.p4),
    }
  }

  #[getter]
  #[global]
  fn p1(&self) -> v8::Global<v8::Object> {
    self.p1.clone()
  }

  #[getter]
  #[global]
  fn p2(&self) -> v8::Global<v8::Object> {
    self.p2.clone()
  }

  #[getter]
  #[global]
  fn p3(&self) -> v8::Global<v8::Object> {
    self.p3.clone()
  }

  #[getter]
  #[global]
  fn p4(&self) -> v8::Global<v8::Object> {
    self.p4.clone()
  }

  fn get_bounds<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    #[inline]
    fn get_ptr(
      scope: &mut v8::HandleScope,
      value: &v8::Global<v8::Object>,
    ) -> cppgc::Ptr<DOMPointReadOnly> {
      let value = v8::Local::new(scope, value);
      cppgc::try_unwrap_cppgc_proto_object::<DOMPointReadOnly>(
        scope,
        value.cast(),
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
    cppgc::wrap_object2(scope, obj, (ro, DOMRect {}))
  }

  #[rename("toJSON")]
  fn to_json<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let mut obj = v8::Object::new(scope);
    set_object(scope, &mut obj, "p1", self.p1.clone());
    set_object(scope, &mut obj, "p2", self.p2.clone());
    set_object(scope, &mut obj, "p3", self.p3.clone());
    set_object(scope, &mut obj, "p4", self.p4.clone());
    obj
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
pub struct DOMMatrixReadOnly {
  inner: RefCell<Matrix4<f64>>,
  is_2d: Cell<bool>,
}

impl GarbageCollected for DOMMatrixReadOnly {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DOMMatrixReadOnly"
  }
}

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

impl DOMMatrixReadOnly {
  fn new<'a>(
    state: &mut OpState,
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'_>,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    // omitted (undefined)
    if value.is_undefined() {
      return Ok(DOMMatrixReadOnly::identity());
    }

    // sequence
    if !value.is_string()
      && let Ok(seq) = Vec::<webidl::UnrestrictedDouble>::convert(
        scope,
        value,
        prefix,
        context,
        &Default::default(),
      )
    {
      let seq = seq.into_iter().map(|f| *f).collect::<Vec<f64>>();
      return DOMMatrixReadOnly::from_sequence_inner(&seq);
    }

    // DOMString
    if let Some(value) = value.to_string(scope) {
      let state = state.borrow_mut::<State>();
      if !state.enable_window_features {
        return Err(GeometryError::DisallowWindowFeatures);
      }

      let matrix = DOMMatrixReadOnly::identity();
      let string = value.to_rust_string_lossy(scope);
      if !string.is_empty() {
        let Ok(transform_list) = TransformList::parse_string(&string) else {
          return Err(GeometryError::FailedToParse);
        };
        matrix.set_matrix_value_inner(&transform_list)?;
      }
      return Ok(matrix);
    }

    Err(GeometryError::FailedToParse)
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

  fn from_sequence_inner(
    seq: &[f64],
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    if let [a, b, c, d, e, f] = seq {
      Ok(DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
           *a,  *c, 0.0,  *e,
           *b,  *d, 0.0,  *f,
          0.0, 0.0, 1.0, 0.0,
          0.0, 0.0, 0.0, 1.0,
        )),
        is_2d: Cell::new(true),
      })
    } else if seq.len() == 16 {
      Ok(DOMMatrixReadOnly {
        inner: RefCell::new(Matrix4::from_column_slice(seq)),
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
    let shift = Vector3::new(tx, ty, tz);
    inner.prepend_translation_mut(&shift);
    self.is_2d.set(is_2d && tz == 0.0);
  }

  #[inline]
  fn scale_without_origin_self_inner(&self, sx: f64, sy: f64, sz: f64) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    let scaling = Vector3::new(sx, sy, sz);
    inner.prepend_nonuniform_scaling_mut(&scaling);
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
    let scaling = Vector3::new(sx, sy, sz);
    let mut shift = Vector3::new(origin_x, origin_y, origin_z);
    inner.prepend_translation_mut(&shift);
    inner.prepend_nonuniform_scaling_mut(&scaling);
    shift.neg_mut();
    inner.prepend_translation_mut(&shift);
    self.is_2d.set(is_2d && sz == 1.0 && origin_z == 0.0);
  }

  #[inline]
  fn rotate_self_inner(&self, roll: f64, pitch: f64, yaw: f64) {
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    let rotation =
      Rotation3::from_euler_angles(roll, pitch, yaw).to_homogeneous();
    let mut result = Matrix4x3::zeros();
    inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
    inner.set_column(0, &result.column(0));
    inner.set_column(1, &result.column(1));
    inner.set_column(2, &result.column(2));
    self.is_2d.set(is_2d && roll == 0.0 && pitch == 0.0);
  }

  #[inline]
  fn rotate_from_vector_self_inner(&self, x: f64, y: f64) {
    if x == 0.0 && y == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    let rotation = Rotation3::from_axis_angle(&Vector3::z_axis(), y.atan2(x))
      .to_homogeneous();
    let mut result = Matrix4x3::zeros();
    inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
    inner.set_column(0, &result.column(0));
    inner.set_column(1, &result.column(1));
    inner.set_column(2, &result.column(2));
  }

  #[inline]
  fn rotate_axis_angle_self_inner(&self, x: f64, y: f64, z: f64, angle: f64) {
    if x == 0.0 && y == 0.0 && z == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    let is_2d = self.is_2d.get();
    let rotation = Rotation3::from_axis_angle(
      &UnitVector3::new_normalize(Vector3::new(x, y, z)),
      angle,
    )
    .to_homogeneous();
    let mut result = Matrix4x3::zeros();
    inner.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
    inner.set_column(0, &result.column(0));
    inner.set_column(1, &result.column(1));
    inner.set_column(2, &result.column(2));
    self.is_2d.set(is_2d && x == 0.0 && y == 0.0);
  }

  #[inline]
  fn skew_self_inner(&self, x: f64, y: f64) {
    let mut inner = self.inner.borrow_mut();
    let skew = Matrix4x2::new(1.0, x.tan(), y.tan(), 1.0, 0.0, 0.0, 0.0, 0.0);
    let mut result = Matrix4x2::zeros();
    inner.mul_to(&skew, &mut result);
    inner.set_column(0, &result.column(0));
    inner.set_column(1, &result.column(1));
  }

  #[inline]
  fn perspective_self_inner(&self, d: f64) {
    if d == 0.0 {
      return;
    }
    let mut inner = self.inner.borrow_mut();
    let perspective =
      Matrix4x2::new(0.0, 0.0, 1.0, -1.0 / d, 0.0, 0.0, 0.0, 1.0);
    let mut result = Matrix4x2::zeros();
    inner.mul_to(&perspective, &mut result);
    inner.set_column(2, &result.column(0));
    inner.set_column(3, &result.column(1));
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
    lhs_inner.mul_to(&rhs_inner, &mut out_inner);
    self.is_2d.set(lhs_is_2d && rhs_is_2d);
  }

  #[inline]
  fn flip_x_inner(&self) {
    let mut inner = self.inner.borrow_mut();
    inner.column_mut(0).neg_mut();
  }

  #[inline]
  fn flip_y_inner(&self) {
    let mut inner = self.inner.borrow_mut();
    inner.column_mut(1).neg_mut();
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
        self.is_2d.set(false);
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

  #[inline]
  fn a_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_A) }
  }

  #[inline]
  fn b_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_B) }
  }

  #[inline]
  fn c_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_C) }
  }

  #[inline]
  fn d_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_D) }
  }

  #[inline]
  fn e_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_E) }
  }

  #[inline]
  fn f_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_F) }
  }

  #[inline]
  fn m11_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M11) }
  }

  #[inline]
  fn m12_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M12) }
  }

  #[inline]
  fn m13_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M13) }
  }

  #[inline]
  fn m14_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M14) }
  }

  #[inline]
  fn m21_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M21) }
  }

  #[inline]
  fn m22_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M22) }
  }

  #[inline]
  fn m23_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M23) }
  }

  #[inline]
  fn m24_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M24) }
  }

  #[inline]
  fn m31_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M31) }
  }

  #[inline]
  fn m32_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M32) }
  }

  #[inline]
  fn m33_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M33) }
  }

  #[inline]
  fn m34_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M34) }
  }

  #[inline]
  fn m41_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M41) }
  }

  #[inline]
  fn m42_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M42) }
  }

  #[inline]
  fn m43_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M43) }
  }

  #[inline]
  fn m44_inner(&self) -> f64 {
    // SAFETY: in-range access
    unsafe { *self.inner.borrow().get_unchecked(INDEX_M44) }
  }

  #[inline]
  fn is_identity_inner(&self) -> bool {
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

  #[inline]
  fn is_finite_inner(&self) -> bool {
    self
      .inner
      .borrow()
      .into_iter()
      .all(|&item| item.is_finite())
  }

  fn set_matrix_value_inner(
    &self,
    transform_list: &TransformList,
  ) -> Result<(), GeometryError> {
    for transform in transform_list.0.iter() {
      match transform {
        Transform::Translate(
          LengthPercentage::Dimension(x),
          LengthPercentage::Dimension(y),
        ) => {
          if let (Some(x), Some(y)) = (x.to_px(), y.to_px()) {
            self.translate_self_inner(x.into(), y.into(), 0.0);
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::TranslateX(LengthPercentage::Dimension(x)) => {
          if let Some(x) = x.to_px() {
            self.translate_self_inner(x.into(), 0.0, 0.0);
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::TranslateY(LengthPercentage::Dimension(y)) => {
          if let Some(y) = y.to_px() {
            self.translate_self_inner(0.0, y.into(), 0.0);
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::TranslateZ(z) => {
          if let Some(z) = z.to_px() {
            self.translate_self_inner(0.0, 0.0, z.into());
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::Translate3d(
          LengthPercentage::Dimension(x),
          LengthPercentage::Dimension(y),
          z,
        ) => {
          if let (Some(x), Some(y), Some(z)) = (x.to_px(), y.to_px(), z.to_px())
          {
            self.translate_self_inner(x.into(), y.into(), z.into());
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::Scale(x, y) => {
          let x: CSSNumber = x.into();
          let y: CSSNumber = y.into();
          self.scale_without_origin_self_inner(x.into(), y.into(), 1.0);
        }
        Transform::ScaleX(x) => {
          let x: CSSNumber = x.into();
          self.scale_without_origin_self_inner(x.into(), 1.0, 1.0);
        }
        Transform::ScaleY(y) => {
          let y: CSSNumber = y.into();
          self.scale_without_origin_self_inner(1.0, y.into(), 1.0);
        }
        Transform::ScaleZ(z) => {
          let z: CSSNumber = z.into();
          self.scale_without_origin_self_inner(1.0, 1.0, z.into());
        }
        Transform::Scale3d(x, y, z) => {
          let x: CSSNumber = x.into();
          let y: CSSNumber = y.into();
          let z: CSSNumber = z.into();
          self.scale_without_origin_self_inner(x.into(), y.into(), z.into());
        }
        Transform::Rotate(angle) | Transform::RotateZ(angle) => {
          self.rotate_axis_angle_self_inner(
            0.0,
            0.0,
            1.0,
            angle.to_radians().into(),
          );
        }
        Transform::RotateX(angle) => {
          self.rotate_axis_angle_self_inner(
            1.0,
            0.0,
            0.0,
            angle.to_radians().into(),
          );
        }
        Transform::RotateY(angle) => {
          self.rotate_axis_angle_self_inner(
            0.0,
            1.0,
            0.0,
            angle.to_radians().into(),
          );
        }
        Transform::Rotate3d(x, y, z, angle) => {
          self.rotate_axis_angle_self_inner(
            (*x).into(),
            (*y).into(),
            (*z).into(),
            angle.to_radians().into(),
          );
        }
        Transform::Skew(x, y) => {
          self.skew_self_inner(x.to_radians().into(), y.to_radians().into());
        }
        Transform::SkewX(angle) => {
          self.skew_self_inner(angle.to_radians().into(), 0.0);
        }
        Transform::SkewY(angle) => {
          self.skew_self_inner(0.0, angle.to_radians().into());
        }
        Transform::Perspective(length) => {
          if let Some(length) = length.to_px() {
            self.perspective_self_inner(length.into());
          } else {
            return Err(GeometryError::ContainsRelativeValue);
          }
        }
        Transform::Matrix(CSSMatrix { a, b, c, d, e, f }) => {
          let lhs = self.clone();
          let rhs = DOMMatrixReadOnly {
            #[rustfmt::skip]
            inner: RefCell::new(Matrix4::new(
              (*a).into(), (*c).into(), 0.0, (*e).into(),
              (*b).into(), (*d).into(), 0.0, (*f).into(),
                      0.0,         0.0, 1.0,         0.0,
                      0.0,         0.0, 0.0,         1.0,
            )),
            is_2d: Cell::new(true),
          };
          self.multiply_self_inner(&lhs, &rhs);
        }
        Transform::Matrix3d(CSSMatrix3d {
          m11,
          m12,
          m13,
          m14,
          m21,
          m22,
          m23,
          m24,
          m31,
          m32,
          m33,
          m34,
          m41,
          m42,
          m43,
          m44,
        }) => {
          let lhs = self.clone();
          let rhs = DOMMatrixReadOnly {
            #[rustfmt::skip]
            inner: RefCell::new(Matrix4::new(
              (*m11).into(), (*m21).into(), (*m31).into(), (*m41).into(),
              (*m12).into(), (*m22).into(), (*m32).into(), (*m42).into(),
              (*m13).into(), (*m23).into(), (*m33).into(), (*m43).into(),
              (*m14).into(), (*m24).into(), (*m34).into(), (*m44).into(),
            )),
            is_2d: Cell::new(false),
          };
          self.multiply_self_inner(&lhs, &rhs);
        }
        _ => {
          return Err(GeometryError::ContainsRelativeValue);
        }
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
    state: &mut OpState,
    scope: &mut v8::HandleScope<'a>,
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
  fn from_float32_array<'a>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    if !value.is_float32_array() {
      return Err(GeometryError::TypeMismatch);
    }
    let seq = Vec::<webidl::UnrestrictedDouble>::convert(
      scope,
      value,
      "Failed to execute 'DOMMatrixReadOnly.fromFloat32Array'".into(),
      (|| Cow::Borrowed("Argument 1")).into(),
      &Default::default(),
    )?;
    let seq = seq.into_iter().map(|f| *f).collect::<Vec<f64>>();
    DOMMatrixReadOnly::from_sequence_inner(&seq)
  }

  #[rename("fromFloat64Array")]
  #[required(1)]
  #[static_method]
  #[cppgc]
  fn from_float64_array<'a>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<DOMMatrixReadOnly, GeometryError> {
    if !value.is_float64_array() {
      return Err(GeometryError::TypeMismatch);
    }
    let seq = Vec::<webidl::UnrestrictedDouble>::convert(
      scope,
      value,
      "Failed to execute 'DOMMatrixReadOnly.fromFloat64Array'".into(),
      (|| Cow::Borrowed("Argument 1")).into(),
      &Default::default(),
    )?;
    let seq = seq.into_iter().map(|f| *f).collect::<Vec<f64>>();
    DOMMatrixReadOnly::from_sequence_inner(&seq)
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
    scope: &mut v8::HandleScope<'a>,
    #[webidl] tx: Option<webidl::UnrestrictedDouble>,
    #[webidl] ty: Option<webidl::UnrestrictedDouble>,
    #[webidl] tz: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let tx = *tx.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let ty = *ty.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let tz = *tz.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    out.translate_self_inner(tx, ty, tz);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn scale<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] sx: Option<webidl::UnrestrictedDouble>,
    #[webidl] sy: Option<webidl::UnrestrictedDouble>,
    #[webidl] sz: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_z: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let sx = *sx.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(webidl::UnrestrictedDouble(sx));
    let sz = *sz.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      out.scale_without_origin_self_inner(sx, sy, sz);
    } else {
      out
        .scale_with_origin_self_inner(sx, sy, sz, origin_x, origin_y, origin_z);
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn scale_non_uniform<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] sx: Option<webidl::UnrestrictedDouble>,
    #[webidl] sy: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let sx = *sx.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let out = self.clone();
    out.scale_without_origin_self_inner(sx, sy, 1.0);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[rename("scale3d")]
  #[required(0)]
  fn scale3d<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] scale: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_z: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let scale = *scale.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      out.scale_without_origin_self_inner(scale, scale, scale);
    } else {
      out.scale_with_origin_self_inner(
        scale, scale, scale, origin_x, origin_y, origin_z,
      );
    }
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn rotate<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] rotate_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] rotate_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] rotate_z: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let rotate_x = *rotate_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let (roll_deg, pitch_deg, yaw_deg) =
      if rotate_y.is_none() && rotate_z.is_none() {
        (0.0, 0.0, rotate_x)
      } else {
        (
          rotate_x,
          *rotate_y.unwrap_or(webidl::UnrestrictedDouble(0.0)),
          *rotate_z.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        )
      };
    let out = self.clone();
    out.rotate_self_inner(
      roll_deg.to_radians(),
      pitch_deg.to_radians(),
      yaw_deg.to_radians(),
    );
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn rotate_from_vector<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x = *x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    out.rotate_from_vector_self_inner(x, y);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn rotate_axis_angle<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] z: Option<webidl::UnrestrictedDouble>,
    #[webidl] angle_deg: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x = *x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let z = *z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let angle_deg = *angle_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    out.rotate_axis_angle_self_inner(x, y, z, angle_deg.to_radians());
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn skew_x<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] x_deg: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let x_deg = *x_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    out.skew_self_inner(x_deg.to_radians(), 0.0);
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn skew_y<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl] y_deg: Option<webidl::UnrestrictedDouble>,
  ) -> v8::Local<'a, v8::Object> {
    let y_deg = *y_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let out = self.clone();
    out.skew_self_inner(0.0, y_deg.to_radians());
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn multiply<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    other: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let out = self.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, other)
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
    Ok(cppgc::wrap_object2(scope, obj, (out, DOMMatrix {})))
  }

  #[required(0)]
  fn flip_x<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.flip_x_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[required(0)]
  fn flip_y<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.flip_y_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  fn inverse<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let out = self.clone();
    out.invert_self_inner();
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    cppgc::wrap_object2(scope, obj, (out, DOMMatrix {}))
  }

  #[reentrant]
  #[required(0)]
  fn transform_point<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    point: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let out = DOMPointReadOnly {
      inner: RefCell::new(Vector4::zeros()),
    };
    if let Some(point) =
      cppgc::try_unwrap_cppgc_proto_object::<DOMPointReadOnly>(scope, point)
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
    Ok(cppgc::wrap_object2(scope, obj, (out, DOMPoint {})))
  }

  #[rename("toJSON")]
  fn to_json<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::Object> {
    let mut obj = v8::Object::new(scope);
    set_f64(scope, &mut obj, "a", self.a_inner());
    set_f64(scope, &mut obj, "b", self.b_inner());
    set_f64(scope, &mut obj, "c", self.c_inner());
    set_f64(scope, &mut obj, "d", self.d_inner());
    set_f64(scope, &mut obj, "e", self.e_inner());
    set_f64(scope, &mut obj, "f", self.f_inner());
    set_f64(scope, &mut obj, "m11", self.m11_inner());
    set_f64(scope, &mut obj, "m12", self.m12_inner());
    set_f64(scope, &mut obj, "m13", self.m13_inner());
    set_f64(scope, &mut obj, "m14", self.m14_inner());
    set_f64(scope, &mut obj, "m21", self.m21_inner());
    set_f64(scope, &mut obj, "m22", self.m22_inner());
    set_f64(scope, &mut obj, "m23", self.m23_inner());
    set_f64(scope, &mut obj, "m24", self.m24_inner());
    set_f64(scope, &mut obj, "m31", self.m31_inner());
    set_f64(scope, &mut obj, "m32", self.m32_inner());
    set_f64(scope, &mut obj, "m33", self.m33_inner());
    set_f64(scope, &mut obj, "m34", self.m34_inner());
    set_f64(scope, &mut obj, "m41", self.m41_inner());
    set_f64(scope, &mut obj, "m42", self.m42_inner());
    set_f64(scope, &mut obj, "m43", self.m43_inner());
    set_f64(scope, &mut obj, "m44", self.m44_inner());
    set_boolean(scope, &mut obj, "is2D", self.is_2d.get());
    set_boolean(scope, &mut obj, "isIdentity", self.is_identity_inner());
    obj
  }
}

pub struct DOMMatrix {}

impl GarbageCollected for DOMMatrix {
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
    state: &mut OpState,
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    // TODO(petamoriken): Error when deleting next line. proc-macro bug?
    #[webidl] _: bool,
  ) -> Result<(DOMMatrixReadOnly, DOMMatrix), GeometryError> {
    let ro = DOMMatrixReadOnly::new(
      state,
      scope,
      value,
      "Failed to construct 'DOMMatrixReadOnly'".into(),
      ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
    )?;
    Ok((ro, DOMMatrix {}))
  }

  #[reentrant]
  #[required(0)]
  #[static_method]
  fn from_matrix<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[webidl] init: DOMMatrixInit,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    let ro = DOMMatrixReadOnly::from_matrix_inner(&init)?;
    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object2(scope, obj, (ro, DOMMatrix {})))
  }

  #[rename("fromFloat32Array")]
  #[required(1)]
  #[static_method]
  fn from_float32_array<'a>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    if !value.is_float32_array() {
      return Err(GeometryError::TypeMismatch);
    }
    let float64 = Vec::<webidl::UnrestrictedDouble>::convert(
      scope,
      value,
      "Failed to execute 'DOMMatrixReadOnly.fromFloat32Array'".into(),
      (|| Cow::Borrowed("Argument 1")).into(),
      &Default::default(),
    )?;
    let float64 = float64.into_iter().map(|f| *f).collect::<Vec<f64>>();

    let ro = if let [a, b, c, d, e, f] = float64.as_slice() {
      DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
           *a,  *c, 0.0,  *e,
           *b,  *d, 0.0,  *f,
          0.0, 0.0, 1.0, 0.0,
          0.0, 0.0, 0.0, 1.0,
        )),
        is_2d: Cell::new(true),
      }
    } else if float64.len() == 16 {
      DOMMatrixReadOnly {
        inner: RefCell::new(Matrix4::from_column_slice(float64.as_slice())),
        is_2d: Cell::new(false),
      }
    } else {
      return Err(GeometryError::InvalidSequenceSize);
    };

    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object2(scope, obj, (ro, DOMMatrix {})))
  }

  #[rename("fromFloat64Array")]
  #[required(1)]
  #[static_method]
  fn from_float64_array<'a>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Object>, GeometryError> {
    if !value.is_float64_array() {
      return Err(GeometryError::TypeMismatch);
    }
    let float64 = Vec::<webidl::UnrestrictedDouble>::convert(
      scope,
      value,
      "Failed to execute 'DOMMatrixReadOnly.fromFloat64Array'".into(),
      (|| Cow::Borrowed("Argument 1")).into(),
      &Default::default(),
    )?;
    let float64 = float64.into_iter().map(|f| *f).collect::<Vec<f64>>();

    let ro = if let [a, b, c, d, e, f] = float64.as_slice() {
      DOMMatrixReadOnly {
        #[rustfmt::skip]
        inner: RefCell::new(Matrix4::new(
           *a,  *c, 0.0,  *e,
           *b,  *d, 0.0,  *f,
          0.0, 0.0, 1.0, 0.0,
          0.0, 0.0, 0.0, 1.0,
        )),
        is_2d: Cell::new(true),
      }
    } else if float64.len() == 16 {
      DOMMatrixReadOnly {
        inner: RefCell::new(Matrix4::from_column_slice(float64.as_slice())),
        is_2d: Cell::new(false),
      }
    } else {
      return Err(GeometryError::InvalidSequenceSize);
    };

    let obj = cppgc::make_cppgc_empty_object::<DOMMatrix>(scope);
    Ok(cppgc::wrap_object2(scope, obj, (ro, DOMMatrix {})))
  }

  #[fast]
  #[getter]
  fn a(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.a_inner()
  }

  #[setter]
  fn a(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_A) = *value;
    }
  }

  #[fast]
  #[getter]
  fn b(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.b_inner()
  }

  #[setter]
  fn b(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_B) = *value;
    }
  }

  #[fast]
  #[getter]
  fn c(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.c_inner()
  }

  #[setter]
  fn c(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_C) = *value;
    }
  }

  #[fast]
  #[getter]
  fn d(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.d_inner()
  }

  #[setter]
  fn d(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_D) = *value;
    }
  }

  #[fast]
  #[getter]
  fn e(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.e_inner()
  }

  #[setter]
  fn e(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_E) = *value;
    }
  }

  #[fast]
  #[getter]
  fn f(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.f_inner()
  }

  #[setter]
  fn f(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_F) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m11(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m11_inner()
  }

  #[setter]
  fn m11(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M11) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m12(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m12_inner()
  }

  #[setter]
  fn m12(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M12) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m13(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m13_inner()
  }

  #[setter]
  fn m13(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M13) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m14(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m14_inner()
  }

  #[setter]
  fn m14(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M14) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m21(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m21_inner()
  }

  #[setter]
  fn m21(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M21) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m22(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m22_inner()
  }

  #[setter]
  fn m22(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M22) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m23(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m23_inner()
  }

  #[setter]
  fn m23(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M23) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m24(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m24_inner()
  }

  #[setter]
  fn m24(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M24) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m31(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m31_inner()
  }

  #[setter]
  fn m31(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M31) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m32(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m32_inner()
  }

  #[setter]
  fn m32(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M32) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m33(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m33_inner()
  }

  #[setter]
  fn m33(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M33) = *value;
    }
    if *value != 1.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m34(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m34_inner()
  }

  #[setter]
  fn m34(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M34) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m41(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m41_inner()
  }

  #[setter]
  fn m41(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M41) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m42(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m42_inner()
  }

  #[setter]
  fn m42(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M42) = *value;
    }
  }

  #[fast]
  #[getter]
  fn m43(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m43_inner()
  }

  #[setter]
  fn m43(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M43) = *value;
    }
    if *value != 0.0 {
      ro.is_2d.set(false);
    }
  }

  #[fast]
  #[getter]
  fn m44(&self, #[proto] ro: &DOMMatrixReadOnly) -> f64 {
    ro.m44_inner()
  }

  #[setter]
  fn m44(
    &self,
    #[webidl] value: webidl::UnrestrictedDouble,
    #[proto] ro: &DOMMatrixReadOnly,
  ) {
    // SAFETY: in-range access
    unsafe {
      *ro.inner.borrow_mut().get_unchecked_mut(INDEX_M44) = *value;
    }
    if *value != 1.0 {
      ro.is_2d.set(false);
    }
  }

  #[required(0)]
  #[global]
  fn translate_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] tx: Option<webidl::UnrestrictedDouble>,
    #[webidl] ty: Option<webidl::UnrestrictedDouble>,
    #[webidl] tz: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let tx = *tx.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let ty = *ty.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let tz = *tz.unwrap_or(webidl::UnrestrictedDouble(0.0));
    ro.translate_self_inner(tx, ty, tz);
    this
  }

  #[required(0)]
  #[global]
  fn scale_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] sx: Option<webidl::UnrestrictedDouble>,
    #[webidl] sy: Option<webidl::UnrestrictedDouble>,
    #[webidl] sz: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_z: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let sx = *sx.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let sy = *sy.unwrap_or(webidl::UnrestrictedDouble(sx));
    let sz = *sz.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      ro.scale_without_origin_self_inner(sx, sy, sz);
    } else {
      ro.scale_with_origin_self_inner(sx, sy, sz, origin_x, origin_y, origin_z);
    }
    this
  }

  #[rename("scale3dSelf")]
  #[required(0)]
  #[global]
  fn scale3d_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] scale: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] origin_z: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let scale = *scale.unwrap_or(webidl::UnrestrictedDouble(1.0));
    let origin_x = *origin_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_y = *origin_y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let origin_z = *origin_z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    if origin_x == 0.0 && origin_y == 0.0 && origin_z == 0.0 {
      ro.scale_without_origin_self_inner(scale, scale, scale);
    } else {
      ro.scale_with_origin_self_inner(
        scale, scale, scale, origin_x, origin_y, origin_z,
      );
    }
    this
  }

  #[required(0)]
  #[global]
  fn rotate_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] rotate_x: Option<webidl::UnrestrictedDouble>,
    #[webidl] rotate_y: Option<webidl::UnrestrictedDouble>,
    #[webidl] rotate_z: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let rotate_x = *rotate_x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let (roll_deg, pitch_deg, yaw_deg) =
      if rotate_y.is_none() && rotate_z.is_none() {
        (0.0, 0.0, rotate_x)
      } else {
        (
          rotate_x,
          *rotate_y.unwrap_or(webidl::UnrestrictedDouble(0.0)),
          *rotate_z.unwrap_or(webidl::UnrestrictedDouble(0.0)),
        )
      };
    ro.rotate_self_inner(
      roll_deg.to_radians(),
      pitch_deg.to_radians(),
      yaw_deg.to_radians(),
    );
    this
  }

  #[required(0)]
  #[global]
  fn rotate_from_vector_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let x = *x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    ro.rotate_from_vector_self_inner(x, y);
    this
  }

  #[required(0)]
  #[global]
  fn rotate_axis_angle_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x: Option<webidl::UnrestrictedDouble>,
    #[webidl] y: Option<webidl::UnrestrictedDouble>,
    #[webidl] z: Option<webidl::UnrestrictedDouble>,
    #[webidl] angle_deg: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let x = *x.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let y = *y.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let z = *z.unwrap_or(webidl::UnrestrictedDouble(0.0));
    let angle_deg = *angle_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    ro.rotate_axis_angle_self_inner(x, y, z, angle_deg.to_radians());
    this
  }

  #[required(0)]
  #[global]
  fn skew_x_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] x_deg: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let x_deg = *x_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    ro.skew_self_inner(x_deg.to_radians(), 0.0);
    this
  }

  #[required(0)]
  #[global]
  fn skew_y_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[webidl] y_deg: Option<webidl::UnrestrictedDouble>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    let y_deg = *y_deg.unwrap_or(webidl::UnrestrictedDouble(0.0));
    ro.skew_self_inner(0.0, y_deg.to_radians());
    this
  }

  #[required(0)]
  #[global]
  fn multiply_self<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'a>,
    other: v8::Local<'a, v8::Value>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> Result<v8::Global<v8::Object>, GeometryError> {
    let lhs = ro.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, other)
    {
      if ptr::eq(ro, &*other) {
        ro.multiply_self_inner(&lhs, &other.clone());
      } else {
        ro.multiply_self_inner(&lhs, &other);
      };
    } else {
      let other = DOMMatrixInit::convert(
        scope,
        other,
        "Failed to execute 'multiply' on 'DOMMatrixReadOnly'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let other = DOMMatrixReadOnly::from_matrix_inner(&other)?;
      ro.multiply_self_inner(&lhs, &other);
    }
    Ok(this)
  }

  #[required(0)]
  #[global]
  fn pre_multiply_self<'a>(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope<'a>,
    other: v8::Local<'a, v8::Value>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> Result<v8::Global<v8::Object>, GeometryError> {
    let rhs = ro.clone();
    if let Some(other) =
      cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, other)
    {
      if ptr::eq(ro, &*other) {
        ro.multiply_self_inner(&other.clone(), &rhs);
      } else {
        ro.multiply_self_inner(&other, &rhs);
      }
    } else {
      let other = DOMMatrixInit::convert(
        scope,
        other,
        "Failed to execute 'multiply' on 'DOMMatrixReadOnly'".into(),
        (|| Cow::Borrowed("Argument 1")).into(),
        &Default::default(),
      )?;
      let other = DOMMatrixReadOnly::from_matrix_inner(&other)?;
      ro.multiply_self_inner(&other, &rhs);
    }
    Ok(this)
  }

  #[global]
  fn invert_self(
    &self,
    #[this] this: v8::Global<v8::Object>,
    #[proto] ro: &DOMMatrixReadOnly,
  ) -> v8::Global<v8::Object> {
    ro.invert_self_inner();
    this
  }
}

#[inline]
fn set_f64(
  scope: &mut v8::HandleScope,
  object: &mut v8::Local<v8::Object>,
  key: &str,
  value: f64,
) {
  let key = v8::String::new(scope, key).unwrap();
  let value = v8::Number::new(scope, value);
  object.set(scope, key.into(), value.into()).unwrap();
}

#[inline]
fn set_boolean(
  scope: &mut v8::HandleScope,
  object: &mut v8::Local<v8::Object>,
  key: &str,
  value: bool,
) {
  let key = v8::String::new(scope, key).unwrap();
  let value = v8::Boolean::new(scope, value);
  object.set(scope, key.into(), value.into()).unwrap();
}

#[inline]
fn set_object(
  scope: &mut v8::HandleScope,
  object: &mut v8::Local<v8::Object>,
  key: &str,
  value: v8::Global<v8::Object>,
) {
  let key = v8::String::new(scope, key).unwrap();
  let value = v8::Local::new(scope, value);
  object.set(scope, key.into(), value.into()).unwrap();
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
fn matrix_transform_point(
  matrix: &DOMMatrixReadOnly,
  point: &DOMPointReadOnly,
  out: &DOMPointReadOnly,
) {
  let inner = matrix.inner.borrow();
  let point = point.inner.borrow();
  let mut result = out.inner.borrow_mut();
  inner.mul_to(&point, &mut result);
}

#[op2]
#[arraybuffer]
pub fn op_geometry_matrix_to_buffer<'a>(
  scope: &mut v8::HandleScope<'a>,
  matrix: v8::Local<'a, v8::Value>,
) -> Result<Vec<u8>, GeometryError> {
  let Some(matrix) =
    cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, matrix)
  else {
    return Err(GeometryError::IllegalInvocation);
  };
  let inner = matrix.inner.borrow();
  Ok(
    // SAFETY: in-range access
    unsafe {
      slice::from_raw_parts(
        inner.as_slice().as_ptr() as *mut u8,
        mem::size_of::<f64>() * 16,
      )
    }
    .to_vec(),
  )
}

#[op2]
#[string]
pub fn op_geometry_matrix_to_string<'a>(
  scope: &mut v8::HandleScope<'a>,
  matrix: v8::Local<'a, v8::Value>,
) -> Result<String, GeometryError> {
  #[inline]
  fn to_string(scope: &mut v8::HandleScope, value: f64) -> String {
    let number = v8::Number::new(scope, value);
    number.to_string(scope).unwrap().to_rust_string_lossy(scope)
  }

  let Some(matrix) =
    cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, matrix)
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

#[op2]
pub fn op_geometry_matrix_set_matrix_value<'a>(
  scope: &mut v8::HandleScope<'a>,
  input: v8::Local<'a, v8::Value>,
  #[string] transform_list: &str,
) -> Result<v8::Local<'a, v8::Value>, GeometryError> {
  if cppgc::try_unwrap_cppgc_proto_object::<DOMMatrix>(scope, input).is_none() {
    return Err(GeometryError::IllegalInvocation);
  }
  let matrix =
    cppgc::try_unwrap_cppgc_proto_object::<DOMMatrixReadOnly>(scope, input)
      .unwrap();
  let Ok(transform_list) = TransformList::parse_string(transform_list) else {
    return Err(GeometryError::FailedToParse);
  };
  matrix.set_matrix_value_inner(&transform_list)?;
  Ok(input)
}
