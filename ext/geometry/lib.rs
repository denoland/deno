// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use nalgebra::Matrix4;
use nalgebra::MatrixView4;
use nalgebra::MatrixViewMut4;
use nalgebra::Vector3;
use std::path::PathBuf;

type Matrix = Matrix4<f64>;
type MatrixView<'a> = MatrixView4<'a, f64>;
type MatrixViewMut<'a> = MatrixViewMut4<'a, f64>;

deno_core::extension!(
  deno_geometry,
  deps = [deno_webidl, deno_web, deno_console],
  ops = [
    op_geometry_translate,
    op_geometry_translate_self,
    op_geometry_scale,
    op_geometry_scale_self,
    op_geometry_scale_with_origin,
    op_geometry_scale_with_origin_self,
    op_geometry_multiply,
    op_geometry_multiply_self,
    op_geometry_premultiply_self,
  ],
  esm = ["01_geometry.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_geometry.d.ts")
}

#[op2(fast)]
pub fn op_geometry_translate(
  #[buffer] input: &[f64],
  x: f64,
  y: f64,
  z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let shift = Vector3::new(x, y, z);
  let mut out = MatrixViewMut::from_slice(out);
  out.copy_from_slice(input);
  out.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_translate_self(
  x: f64,
  y: f64,
  z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let shift = Vector3::new(x, y, z);
  let mut out = MatrixViewMut::from_slice(out);
  out.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_scale(
  #[buffer] input: &[f64],
  x: f64,
  y: f64,
  z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let scaling = Vector3::new(x, y, z);
  let mut out = MatrixViewMut::from_slice(out);
  out.copy_from_slice(input);
  out.prepend_nonuniform_scaling_mut(&scaling);
}

#[op2(fast)]
pub fn op_geometry_scale_self(
  x: f64,
  y: f64,
  z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let scaling = Vector3::new(x, y, z);
  let mut out = MatrixViewMut::from_slice(out);
  out.prepend_nonuniform_scaling_mut(&scaling);
}

#[op2(fast)]
pub fn op_geometry_scale_with_origin(
  #[buffer] input: &[f64],
  x: f64,
  y: f64,
  z: f64,
  origin_x: f64,
  origin_y: f64,
  origin_z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let scaling = Vector3::new(x, y, z);
  let mut shift = Vector3::new(origin_x, origin_y, origin_z);
  let mut out = MatrixViewMut::from_slice(out);
  out.copy_from_slice(input);
  out.prepend_translation_mut(&shift);
  out.prepend_nonuniform_scaling_mut(&scaling);
  shift.neg_mut();
  out.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_scale_with_origin_self(
  x: f64,
  y: f64,
  z: f64,
  origin_x: f64,
  origin_y: f64,
  origin_z: f64,
  #[buffer] out: &mut [f64],
) -> () {
  let scaling = Vector3::new(x, y, z);
  let mut shift = Vector3::new(origin_x, origin_y, origin_z);
  let mut out = MatrixViewMut::from_slice(out);
  out.prepend_translation_mut(&shift);
  out.prepend_nonuniform_scaling_mut(&scaling);
  shift.neg_mut();
  out.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_multiply(
  #[buffer] lhs: &[f64],
  #[buffer] rhs: &[f64],
  #[buffer] out: &mut [f64],
) -> () {
  let lhs = MatrixView::from_slice(lhs);
  let rhs = MatrixView::from_slice(rhs);
  let mut out = MatrixViewMut::from_slice(out);
  lhs.mul_to(&rhs, &mut out);
}

#[op2(fast)]
pub fn op_geometry_multiply_self(
  #[buffer] rhs: &[f64],
  #[buffer] out: &mut [f64],
) -> () {
  let rhs = MatrixView::from_slice(rhs);
  let mut out = MatrixViewMut::from_slice(out);
  let mut result = Matrix::zeros();
  out.mul_to(&rhs, &mut result);
  out.copy_from(&result);
}

#[op2(fast)]
pub fn op_geometry_premultiply_self(
  #[buffer] lhs: &[f64],
  #[buffer] out: &mut [f64],
) -> () {
  let lhs = MatrixView::from_slice(lhs);
  let mut out = MatrixViewMut::from_slice(out);
  let mut result = Matrix::zeros();
  lhs.mul_to(&out, &mut result);
  out.copy_from(&result);
}
