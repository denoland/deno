// Copyright 2018-2026 the Deno authors. MIT license.

// Two concurrent dynamic imports that both transitively depend on the same
// lazy-loaded ESM module ("custom:lazy_shared"). Each import() spawns its
// own RecursiveModuleLoad, and the per-load `visited` set does not dedupe
// across loads.
const [a, b] = await Promise.all([
  import("./lazy_loaded_concurrent_a.js"),
  import("./lazy_loaded_concurrent_b.js"),
]);
if (a.a !== "shared") {
  throw new Error("expected a.a to be 'shared', got: " + a.a);
}
if (b.b !== "shared") {
  throw new Error("expected b.b to be 'shared', got: " + b.b);
}
