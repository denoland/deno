// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// This is a doc comment.
#[op2]
pub fn op_print(#[string] msg: &str, is_err: bool) -> Result<(), Error> {}
