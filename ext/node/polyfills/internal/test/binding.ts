// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.
import { getBinding } from "ext:deno_node/internal_binding/mod.ts";
import type { BindingName } from "ext:deno_node/internal_binding/mod.ts";

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

export function internalBinding(name: BindingName) {
  emitBindingWarning();
  return getBinding(name);
}

// TODO(kt3k): export actual primordials
export const primordials = {};

export default {
  internalBinding,
  primordials,
};
