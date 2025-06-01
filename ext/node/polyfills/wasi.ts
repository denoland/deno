// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

const { Error } = primordials;

class Context {
  constructor() {
    throw new Error("Context is currently not supported");
  }
}

export const WASI = Context;

export default { WASI };
