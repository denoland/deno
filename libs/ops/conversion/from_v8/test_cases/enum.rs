// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(FromV8)]
pub enum SimpleEnum {
  VariantA,
  #[from_v8(rename = "customName")]
  VariantB,
  VariantC(u32),
  #[from_v8(rename = "renamedNewtype")]
  VariantD(String),
}

#[derive(FromV8)]
pub enum SerdeEnum {
  WithoutSerde,
  #[from_v8(serde)]
  WithSerde(Vec<u32>),
}
