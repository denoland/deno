// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#[op2(fast)]
fn op_string_owned(#[string] s: &str) -> u32 {}
