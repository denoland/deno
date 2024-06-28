// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

class Context {
  constructor() {
    throw new Error("Context is currently not supported");
  }
}

export const WASI = Context;

export default { WASI };
