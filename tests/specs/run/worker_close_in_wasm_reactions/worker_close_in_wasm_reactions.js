// Copyright 2018-2025 the Deno authors. MIT license.

// https://github.com/denoland/deno/issues/12263
// Test for a panic that happens when a worker is closed in the reactions of a
// Wasm async operation.

new Worker(
  import.meta.resolve("./close_in_wasm_reactions.js"),
  { type: "module" },
);
