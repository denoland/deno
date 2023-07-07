// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// https://github.com/denoland/deno/issues/12263
// Test for a panic that happens when a worker is closed in the reactions of a
// WASM async operation.

new Worker(
  import.meta.resolve("../workers/close_in_wasm_reactions.js"),
  { type: "module" },
);
