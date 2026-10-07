// Copyright 2018-2026 the Deno authors. MIT license.
// Mirrors the `__bootstrap` IIFE preamble that ~160 of deno's residual ext
// polyfills use: destructure the captured bootstrap view and pull ops off
// `core.ops` at load (module-body) time, long after the runtime bootstrap has
// deleted `globalThis.__bootstrap` and reduced the user-visible ops surface.
(function () {
const core = __bootstrap.core;
const { op_bootstrap_add } = core.ops;
return {
  addIsFunction: typeof op_bootstrap_add === "function",
  sum: op_bootstrap_add(40, 2),
  viaOpsObject: core.ops.op_bootstrap_add(1, 2),
  bootstrapDeleted: typeof globalThis.__bootstrap === "undefined",
};
})();
