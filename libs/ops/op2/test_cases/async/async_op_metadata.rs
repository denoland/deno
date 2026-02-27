// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2]
#[meta(sanitizer_details = "read from a Blob or File")]
#[meta(sanitizer_fix = "awaiting the result of a Blob or File read")]
async fn op_blob_read_part() {
  return;
}

#[op2]
#[meta(
  sanitizer_details = "receive a message from a BroadcastChannel",
  sanitizer_fix = "closing the BroadcastChannel"
)]
async fn op_broadcast_recv() {
  return;
}
