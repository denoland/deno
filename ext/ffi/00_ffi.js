// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const __bootstrap = window.__bootstrap;
  const {
    ArrayBuffer,
  } = window.__bootstrap.primordials;
  class DynamicLibrary {
    #rid;
    symbols = {};

    constructor(path, symbols) {
      this.#rid = core.opSync("op_ffi_load", { path, symbols });

      for (const symbol in symbols) {
        const isNonBlocking = symbols[symbol].nonblocking;

        this.symbols[symbol] = (...args) => {
          const parameters = [];
          const buffers = [];

          for (const arg of args) {
            if (
              arg?.buffer instanceof ArrayBuffer &&
              arg.byteLength !== undefined
            ) {
              parameters.push(buffers.length);
              buffers.push(arg);
            } else {
              parameters.push(arg);
            }
          }

          if (isNonBlocking) {
            return core.opAsync("op_ffi_call_nonblocking", {
              rid: this.#rid,
              symbol,
              parameters,
              buffers,
            });
          } else {
            return core.opSync("op_ffi_call", {
              rid: this.#rid,
              symbol,
              parameters,
              buffers,
            });
          }
        };
      }
    }

    close() {
      core.close(this.#rid);
    }
  }

  function dlopen(path, symbols) {
    // URL support is progressively enhanced by util in `runtime/js`.
    const pathFromURL = __bootstrap.util.pathFromURL ?? ((p) => p);
    return new DynamicLibrary(pathFromURL(path), symbols);
  }

  window.__bootstrap.ffi = { dlopen };
})(this);
