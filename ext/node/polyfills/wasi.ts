// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

class Context {
  constructor() {
    throw new Error("Context is currently not supported");
  }
}

export const WASI = Context;

export default { WASI };
