// Copyright 2018-2025 the Deno authors. MIT license.
/**
 * This module provides the JavaScript interface atop calls to the Rust ops.
 */

// Minimal example, just passes arguments through to Rust:
export function callRust(stringValue) {
  const { op_call_rust } = Deno.core.ops;
  op_call_rust(stringValue);
}
