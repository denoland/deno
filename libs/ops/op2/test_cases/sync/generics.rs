// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

pub trait Trait {}

#[op2(fast)]
pub fn op_generics<T: Trait>() {}

#[op2(fast)]
pub fn op_generics_static<T: Trait + 'static>() {}

#[op2(fast)]
pub fn op_generics_static_where<T>()
where
  T: Trait + 'static,
{
}
