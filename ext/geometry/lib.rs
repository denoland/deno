// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use nalgebra::Matrix4;
use nalgebra::MatrixView4;
use nalgebra::MatrixViewMut4;
use std::path::PathBuf;

type Matrix = Matrix4<f64>;
type MatrixView<'a> = MatrixView4<'a, f64>;
type MatrixViewMut<'a> = MatrixViewMut4<'a, f64>;

deno_core::extension!(
  deno_geometry,
  deps = [deno_webidl, deno_web, deno_console],
  ops = [
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
