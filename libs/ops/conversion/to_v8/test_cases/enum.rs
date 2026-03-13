// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(ToV8)]
pub enum SimpleEnum {
  VariantA,
  #[to_v8(rename = "customName")]
  VariantB,
  VariantC {
    field: u32,
  },
  #[to_v8(rename = "renamedWithFields")]
  VariantD {
    value: String,
  },
}

#[derive(ToV8)]
#[to_v8(tag = "type")]
pub enum InternallyTaggedEnum {
  #[v8(rename = "custom_a")]
  A,
  B {
    data: u32,
  },
}

#[derive(ToV8)]
#[to_v8(tag = "kind", content = "data")]
pub enum AdjacentlyTaggedEnum {
  #[to_v8(rename = "FIRST")]
  First,
  Second(u32),
}

#[derive(ToV8)]
pub enum SerdeEnum {
  #[to_v8(serde)]
  WithSerde {
    data: Vec<u32>,
    #[to_v8(rename = "renamedField")]
    other: String,
  },
  WithoutSerde {
    value: u32,
  },
}
