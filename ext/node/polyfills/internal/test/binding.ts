// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.
(function () {
const { core } = globalThis.__bootstrap;
const lazyBindingMod = core.createLazyLoader(
  "ext:deno_node/internal_binding/mod.ts",
);

let warningEmitted = false;

function emitBindingWarning() {
  if (!warningEmitted) {
    warningEmitted = true;
    // deno-lint-ignore no-process-global
    if (typeof process !== "undefined" && process.emitWarning) {
      // deno-lint-ignore no-process-global
      process.emitWarning(
        "These APIs are for internal testing only. Do not use them.",
        "internal/test/binding",
      );
    }
  }
}

function internalBinding(name) {
  emitBindingWarning();
  return lazyBindingMod().getBinding(name);
}

// TODO(kt3k): export actual primordials
const primordials = {};

return {
  internalBinding,
  primordials,
  default: {
    internalBinding,
    primordials,
  },
};
})();
