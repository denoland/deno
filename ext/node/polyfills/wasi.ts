// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { primordials } = globalThis.__bootstrap;

const { Error } = primordials;

class Context {
  constructor() {
    throw new Error("Context is currently not supported");
  }
}

const WASI = Context;

return {
  default: { WASI },
  WASI,
};
})();
