// Copyright 2018-2026 the Deno authors. MIT license.

use std::f64::consts::PI;
use std::f64::consts::TAU;

#[inline]
pub(super) fn normalize_angle(angle: f64) -> f64 {
  (angle + PI).rem_euclid(TAU) - PI
}

#[inline]
pub(super) fn positive_angle_delta(start_angle: f64, end_angle: f64) -> f64 {
  (end_angle - start_angle).rem_euclid(TAU)
}

#[inline]
pub(super) fn signed_angle_delta(start_angle: f64, end_angle: f64) -> f64 {
  let delta = normalize_angle(end_angle - start_angle);
  if delta == -PI { PI } else { delta }
}
