// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const __bootstrap = window.__bootstrap;
  const {
    BigInt,
    ObjectDefineProperty,
    ObjectPrototypeIsPrototypeOf,
    PromisePrototypeThen,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

  function unpackU64([hi, lo]) {
    return BigInt(hi) << 32n | BigInt(lo);
  }

  function unpackI64([hi, lo]) {
    const u64 = unpackU64([hi, lo]);
    return u64 >> 63n ? u64 - 0x10000000000000000n : u64;
  }

  class UnsafePointerView {
    pointer;

    constructor(pointer) {
      this.pointer = pointer;
    }

    getUint8(offset = 0) {
      return core.opSync(
        "op_ffi_read_u8",
        this.pointer + BigInt(offset),
      );
    }

    getInt8(offset = 0) {
      return core.opSync(
        "op_ffi_read_i8",
        this.pointer + BigInt(offset),
      );
    }

    getUint16(offset = 0) {
      return core.opSync(
        "op_ffi_read_u16",
        this.pointer + BigInt(offset),
      );
    }

    getInt16(offset = 0) {
      return core.opSync(
        "op_ffi_read_i16",
        this.pointer + BigInt(offset),
      );
    }

    getUint32(offset = 0) {
      return core.opSync(
        "op_ffi_read_u32",
        this.pointer + BigInt(offset),
      );
    }

    getInt32(offset = 0) {
      return core.opSync(
        "op_ffi_read_i32",
        this.pointer + BigInt(offset),
      );
    }

    getBigUint64(offset = 0) {
      return core.opSync(
        "op_ffi_read_u64",
        this.pointer + BigInt(offset),
      );
    }

    getBigInt64(offset = 0) {
      return core.opSync(
        "op_ffi_read_u64",
        this.pointer + BigInt(offset),
      );
    }

    getFloat32(offset = 0) {
      return core.opSync(
        "op_ffi_read_f32",
        this.pointer + BigInt(offset),
      );
    }

    getFloat64(offset = 0) {
      return core.opSync(
        "op_ffi_read_f64",
        this.pointer + BigInt(offset),
      );
    }

    getCString(offset = 0) {
      return core.opSync(
        "op_ffi_cstr_read",
        this.pointer + BigInt(offset),
      );
    }

    getArrayBuffer(byteLength, offset = 0) {
      const uint8array = new Uint8Array(byteLength);
      this.copyInto(uint8array, offset);
      return uint8array.buffer;
    }

    copyInto(destination, offset = 0) {
      core.opSync(
        "op_ffi_buf_copy_into",
        this.pointer + BigInt(offset),
        destination,
        destination.byteLength,
      );
    }
  }

  class UnsafePointer {
    static of(value) {
      if (ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, value)) {
        return value.pointer;
      }
      return core.opSync("op_ffi_ptr_of", value);
    }
  }

  function unpackNonblockingReturnValue(type, result) {
    if (
      typeof type === "object" && type !== null && "function" in type ||
      type === "pointer"
    ) {
      return unpackU64(result);
    }
    switch (type) {
      case "isize":
      case "i64":
        return unpackI64(result);
      case "usize":
      case "u64":
        return unpackU64(result);
      default:
        return result;
    }
  }

  class UnsafeFnPointer {
    pointer;
    definition;

    constructor(pointer, definition) {
      this.pointer = pointer;
      this.definition = definition;
    }

    call(...parameters) {
      const resultType = this.definition.result;
      if (this.definition.nonblocking) {
        const promise = core.opAsync(
          "op_ffi_call_ptr_nonblocking",
          this.pointer,
          this.definition,
          parameters,
        );

        if (
          isReturnedAsBigInt(resultType)
        ) {
          return PromisePrototypeThen(
            promise,
            (result) => unpackNonblockingReturnValue(resultType, result),
          );
        }

        return promise;
      } else {
        return core.opSync(
          "op_ffi_call_ptr",
          this.pointer,
          this.definition,
          parameters,
        );
      }
    }
  }

  function isPointerType(type) {
    return type === "pointer" ||
      typeof type === "object" && type !== null && "function" in type;
  }

  function isReturnedAsBigInt(type) {
    return isPointerType(type) || type === "u64" || type === "i64" ||
      type === "usize" || type === "isize";
  }

  class UnsafeCallback {
    #rid;
    definition;
    callback;
    pointer;

    constructor(definition, callback) {
      if (definition.nonblocking) {
        throw new TypeError(
          "Invalid UnsafeCallback, cannot be nonblocking",
        );
      }
      const [rid, pointer] = core.opSync(
        "op_ffi_unsafe_callback_create",
        definition,
        callback,
      );
      this.#rid = rid;
      this.pointer = pointer;
      this.definition = definition;
      this.callback = callback;
    }

    close() {
      core.close(this.#rid);
    }
  }

  const UnsafeCallbackPrototype = UnsafeCallback.prototype;

  class DynamicLibrary {
    #rid;
    symbols = {};

    constructor(path, symbols) {
      this.#rid = core.opSync("op_ffi_load", { path, symbols });

      for (const symbol in symbols) {
        if ("type" in symbols[symbol]) {
          const type = symbols[symbol].type;
          if (type === "void") {
            throw new TypeError(
              "Foreign symbol of type 'void' is not supported.",
            );
          }

          const name = symbols[symbol].name || symbol;
          const value = core.opSync(
            "op_ffi_get_static",
            this.#rid,
            name,
            type,
          );
          ObjectDefineProperty(
            this.symbols,
            symbol,
            {
              configurable: false,
              enumerable: true,
              value,
              writable: false,
            },
          );
          continue;
        }

        const isNonBlocking = symbols[symbol].nonblocking;
        const resultType = symbols[symbol].result;

        let fn;
        if (isNonBlocking) {
          const needsUnpacking = isReturnedAsBigInt(resultType);
          fn = (...parameters) => {
            const promise = core.opAsync(
              "op_ffi_call_nonblocking",
              this.#rid,
              symbol,
              parameters,
            );

            if (needsUnpacking) {
              return PromisePrototypeThen(
                promise,
                (result) => unpackNonblockingReturnValue(resultType, result),
              );
            }

            return promise;
          };
        } else {
          fn = (...parameters) =>
            core.opSync(
              "op_ffi_call",
              this.#rid,
              symbol,
              parameters,
            );
        }

        ObjectDefineProperty(
          this.symbols,
          symbol,
          {
            configurable: false,
            enumerable: true,
            value: fn,
            writable: false,
          },
        );
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

  window.__bootstrap.ffi = {
    dlopen,
    UnsafeCallback,
    UnsafePointer,
    UnsafePointerView,
    UnsafeFnPointer,
  };
})(this);
