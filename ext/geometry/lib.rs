// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use nalgebra::Matrix3;
use nalgebra::Matrix4;
use nalgebra::Matrix4x2;
use nalgebra::Matrix4x3;
use nalgebra::MatrixView4;
use nalgebra::MatrixViewMut4;
use nalgebra::Rotation3;
use nalgebra::UnitVector3;
use nalgebra::Vector3;
use nalgebra::Vector4;
use nalgebra::VectorViewMut4;
use std::path::PathBuf;

deno_core::extension!(
  deno_geometry,
  deps = [deno_webidl, deno_web, deno_console],
  ops = [
    op_geometry_translate_self,
    op_geometry_scale_self,
    op_geometry_scale_with_origin_self,
    op_geometry_rotate_self,
    op_geometry_rotate_from_vector_self,
    op_geometry_rotate_axis_angle_self,
    op_geometry_skew_self,
    op_geometry_multiply,
    op_geometry_multiply_self,
    op_geometry_premultiply_self,
    op_geometry_flip_x_self,
    op_geometry_flip_y_self,
    op_geometry_invert_self,
    op_geometry_invert_2d_self,
    op_geometry_premultiply_point_self,
  ],
  esm = ["00_init.js"],
  lazy_loaded_esm = ["01_geometry.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_geometry.d.ts")
}

#[op2(fast)]
pub fn op_geometry_translate_self(
  x: f64,
  y: f64,
  z: f64,
  #[buffer] inout: &mut [f64],
) {
  let shift = Vector3::new(x, y, z);
  let mut inout = MatrixViewMut4::from_slice(inout);
  inout.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_scale_self(
  x: f64,
  y: f64,
  z: f64,
  #[buffer] inout: &mut [f64],
) {
  let scaling = Vector3::new(x, y, z);
  let mut inout = MatrixViewMut4::from_slice(inout);
  inout.prepend_nonuniform_scaling_mut(&scaling);
}

#[op2(fast)]
pub fn op_geometry_scale_with_origin_self(
  x: f64,
  y: f64,
  z: f64,
  origin_x: f64,
  origin_y: f64,
  origin_z: f64,
  #[buffer] inout: &mut [f64],
) {
  let scaling = Vector3::new(x, y, z);
  let mut shift = Vector3::new(origin_x, origin_y, origin_z);
  let mut inout = MatrixViewMut4::from_slice(inout);
  inout.prepend_translation_mut(&shift);
  inout.prepend_nonuniform_scaling_mut(&scaling);
  shift.neg_mut();
  inout.prepend_translation_mut(&shift);
}

#[op2(fast)]
pub fn op_geometry_rotate_self(
  roll_degrees: f64,
  pitch_degrees: f64,
  yaw_degrees: f64,
  #[buffer] inout: &mut [f64],
) {
  let rotation = Rotation3::from_euler_angles(
    roll_degrees.to_radians(),
    pitch_degrees.to_radians(),
    yaw_degrees.to_radians(),
  )
  .to_homogeneous();
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4x3::zeros();
  inout.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inout.set_column(0, &result.column(0));
  inout.set_column(1, &result.column(1));
  inout.set_column(2, &result.column(2));
}

#[op2(fast)]
pub fn op_geometry_rotate_from_vector_self(
  x: f64,
  y: f64,
  #[buffer] inout: &mut [f64],
) {
  let rotation =
    Rotation3::from_axis_angle(&Vector3::z_axis(), y.atan2(x)).to_homogeneous();
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4x3::zeros();
  inout.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inout.set_column(0, &result.column(0));
  inout.set_column(1, &result.column(1));
  inout.set_column(2, &result.column(2));
}

#[op2(fast)]
pub fn op_geometry_rotate_axis_angle_self(
  x: f64,
  y: f64,
  z: f64,
  angle_degrees: f64,
  #[buffer] inout: &mut [f64],
) {
  let rotation = Rotation3::from_axis_angle(
    &UnitVector3::new_normalize(Vector3::new(x, y, z)),
    angle_degrees.to_radians(),
  )
  .to_homogeneous();
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4x3::zeros();
  inout.mul_to(&rotation.fixed_view::<4, 3>(0, 0), &mut result);
  inout.set_column(0, &result.column(0));
  inout.set_column(1, &result.column(1));
  inout.set_column(2, &result.column(2));
}

#[op2(fast)]
pub fn op_geometry_skew_self(
  x_degrees: f64,
  y_degrees: f64,
  #[buffer] inout: &mut [f64],
) {
  let skew = Matrix4x2::new(
    1.0,
    x_degrees.to_radians().tan(),
    y_degrees.to_radians().tan(),
    1.0,
    0.0,
    0.0,
    0.0,
    0.0,
  );
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4x2::zeros();
  inout.mul_to(&skew, &mut result);
  inout.set_column(0, &result.column(0));
  inout.set_column(1, &result.column(1));
}

#[op2(fast)]
pub fn op_geometry_multiply(
  #[buffer] lhs: &[f64],
  #[buffer] rhs: &[f64],
  #[buffer] out: &mut [f64],
) {
  let lhs = MatrixView4::from_slice(lhs);
  let rhs = MatrixView4::from_slice(rhs);
  let mut out = MatrixViewMut4::from_slice(out);
  lhs.mul_to(&rhs, &mut out);
}

#[op2(fast)]
pub fn op_geometry_multiply_self(
  #[buffer] rhs: &[f64],
  #[buffer] inout: &mut [f64],
) {
  let rhs = MatrixView4::from_slice(rhs);
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4::zeros();
  inout.mul_to(&rhs, &mut result);
  inout.copy_from(&result);
}

#[op2(fast)]
pub fn op_geometry_premultiply_self(
  #[buffer] lhs: &[f64],
  #[buffer] inout: &mut [f64],
) {
  let lhs = MatrixView4::from_slice(lhs);
  let mut inout = MatrixViewMut4::from_slice(inout);
  let mut result = Matrix4::zeros();
  lhs.mul_to(&inout, &mut result);
  inout.copy_from(&result);
}

#[op2(fast)]
pub fn op_geometry_flip_x_self(#[buffer] inout: &mut [f64]) {
  let mut inout = MatrixViewMut4::from_slice(inout);
  inout.column_mut(0).neg_mut();
}

#[op2(fast)]
pub fn op_geometry_flip_y_self(#[buffer] inout: &mut [f64]) {
  let mut inout = MatrixViewMut4::from_slice(inout);
  inout.column_mut(1).neg_mut();
}

#[op2(fast)]
pub fn op_geometry_invert_self(#[buffer] inout: &mut [f64]) -> bool {
  if inout.iter().any(|&x| x.is_infinite()) {
    inout.fill(f64::NAN);
    return false;
  }

  let mut inout = MatrixViewMut4::from_slice(inout);
  if !inout.try_inverse_mut() {
    inout.fill(f64::NAN);
    return false;
  }

  true
}

#[op2(fast)]
pub fn op_geometry_invert_2d_self(#[buffer] inout: &mut [f64]) -> bool {
  if inout.iter().any(|&x| x.is_infinite()) {
    inout.fill(f64::NAN);
    return false;
  }

  let mut matrix = Matrix3::new(
    inout[0], inout[4], inout[12], inout[1], inout[5], inout[13], 0.0, 0.0, 1.0,
  );
  if !matrix.try_inverse_mut() {
    inout.fill(f64::NAN);
    return false;
  }

  let matrix = matrix.as_slice();
  inout[0] = matrix[0];
  inout[1] = matrix[1];
  inout[4] = matrix[3];
  inout[5] = matrix[4];
  inout[12] = matrix[6];
  inout[13] = matrix[7];

  true
}

#[op2(fast)]
pub fn op_geometry_premultiply_point_self(
  #[buffer] lhs: &[f64],
  #[buffer] inout: &mut [f64],
) {
  let lhs = MatrixView4::from_slice(lhs);
  let mut inout = VectorViewMut4::from_slice(inout);
  let mut result = Vector4::zeros();
  lhs.mul_to(&inout, &mut result);
  inout.copy_from(&result);
}
