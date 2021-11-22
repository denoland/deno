// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const __bootstrap = window.__bootstrap;
  const {
    ArrayBuffer,
    BigInt,
    Number,
    TypeError,
  } = window.__bootstrap.primordials;

  function unpackPointer([hi, lo]) {
    return BigInt(hi) << 32n | BigInt(lo);
  }

  function packPointer(value) {
    return [Number(value >> 32n), Number(value & 0xFFFFFFFFn)];
  }

  class UnsafePointer {
    value;

    constructor(value) {
      this.value = value;
    }

    static null() {
      return new UnsafePointer(0n);
    }

    static of(typedArray) {
      return new UnsafePointer(
        unpackPointer(core.opSync("op_ffi_ptr_of", typedArray)),
      );
    }

    read(into, offset = 0) {
      core.opSync("op_ffi_buf_read_into", [
        packPointer(this.value + BigInt(offset)),
        into,
        into.byteLength,
      ]);
    }

    readCString(offset = 0) {
      return core.opSync(
        "op_ffi_cstr_read",
        packPointer(this.value + BigInt(offset)),
      );
    }
  }

  class DynamicLibrary {
    #rid;
    symbols = {};

    constructor(path, symbols) {
      this.#rid = core.opSync("op_ffi_load", { path, symbols });

      for (const symbol in symbols) {
        const isNonBlocking = symbols[symbol].nonblocking;
        const types = symbols[symbol].parameters;

        this.symbols[symbol] = (...args) => {
          const parameters = [];
          const buffers = [];

          for (let i = 0; i < types.length; i++) {
            const type = types[i];
            const arg = args[i];

            if (type === "buffer") {
              if (
                arg?.buffer instanceof ArrayBuffer &&
                arg.byteLength !== undefined
              ) {
                parameters.push(buffers.length);
                buffers.push(arg);
              } else if (arg instanceof UnsafePointer) {
                parameters.push(packPointer(arg.value));
                buffers.push(undefined);
              } else {
                throw new TypeError(
                  "Invalid ffi arg value, expected TypedArray or UnsafePointer",
                );
              }
            } else {
              parameters.push(arg);
            }
          }

          if (isNonBlocking) {
            const promise = core.opAsync("op_ffi_call_nonblocking", {
              rid: this.#rid,
              symbol,
              parameters,
              buffers,
            });

            if (symbols[symbol].result === "buffer") {
              return promise.then((value) =>
                new UnsafePointer(unpackPointer(value))
              );
            }

            return promise;
          } else {
            const result = core.opSync("op_ffi_call", {
              rid: this.#rid,
              symbol,
              parameters,
              buffers,
            });

            if (symbols[symbol].result === "buffer") {
              return new UnsafePointer(unpackPointer(result));
            }

            return result;
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

  window.__bootstrap.ffi = { dlopen, UnsafePointer };
})(this);
