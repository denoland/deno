// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::convert::Infallible;

pub trait InfallibleResultExt<T> {
  fn unwrap_infallible(self) -> T;
}

impl<T> InfallibleResultExt<T> for Result<T, Infallible> {
  fn unwrap_infallible(self) -> T {
    match self {
      Ok(value) => value,
      Err(never) => match never {},
    }
  }
}
