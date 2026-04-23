// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

/// This is a doc comment.
#[op2(fast)]
pub fn op_has_doc_comment() -> () {}
