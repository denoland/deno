// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const __bootstrap = window.__bootstrap;
  const {
    BigInt,
    ObjectDefineProperty,
    ArrayPrototypeMap,
    Number,
    NumberIsSafeInteger,
    ArrayPrototypeJoin,
    ObjectPrototypeIsPrototypeOf,
    TypeError,
    Int32Array,
    Uint32Array,
    BigInt64Array,
    Function,
  } = window.__bootstrap.primordials;

  class UnsafePointerView {
    pointer;

    constructor(pointer) {
      this.pointer = pointer;
    }

    getBool(offset = 0) {
      return ops.op_ffi_read_bool(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getUint8(offset = 0) {
      return ops.op_ffi_read_u8(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getInt8(offset = 0) {
      return ops.op_ffi_read_i8(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getUint16(offset = 0) {
      return ops.op_ffi_read_u16(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getInt16(offset = 0) {
      return ops.op_ffi_read_i16(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getUint32(offset = 0) {
      return ops.op_ffi_read_u32(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getInt32(offset = 0) {
      return ops.op_ffi_read_i32(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getBigUint64(offset = 0) {
      return ops.op_ffi_read_u64(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getBigInt64(offset = 0) {
      return ops.op_ffi_read_i64(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getFloat32(offset = 0) {
      return ops.op_ffi_read_f32(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getFloat64(offset = 0) {
      return ops.op_ffi_read_f64(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    getCString(offset = 0) {
      return ops.op_ffi_cstr_read(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
      );
    }

    static getCString(pointer, offset = 0) {
      return ops.op_ffi_cstr_read(
        offset ? BigInt(pointer) + BigInt(offset) : pointer,
      );
    }

    getArrayBuffer(byteLength, offset = 0) {
      return ops.op_ffi_get_buf(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
        byteLength,
      );
    }

    static getArrayBuffer(pointer, byteLength, offset = 0) {
      return ops.op_ffi_get_buf(
        offset ? BigInt(pointer) + BigInt(offset) : pointer,
        byteLength,
      );
    }

    copyInto(destination, offset = 0) {
      ops.op_ffi_buf_copy_into(
        offset ? BigInt(this.pointer) + BigInt(offset) : this.pointer,
        destination,
        destination.byteLength,
      );
    }

    static copyInto(pointer, destination, offset = 0) {
      ops.op_ffi_buf_copy_into(
        offset ? BigInt(pointer) + BigInt(offset) : pointer,
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
      return ops.op_ffi_ptr_of(value);
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
      if (this.definition.nonblocking) {
        return core.opAsync(
          "op_ffi_call_ptr_nonblocking",
          this.pointer,
          this.definition,
          parameters,
        );
      } else {
        return ops.op_ffi_call_ptr(
          this.pointer,
          this.definition,
          parameters,
        );
      }
    }
  }

  function isReturnedAsBigInt(type) {
    return type === "buffer" || type === "pointer" || type === "function" ||
      type === "u64" || type === "i64" ||
      type === "usize" || type === "isize";
  }

  function isI64(type) {
    return type === "i64" || type === "isize";
  }

  class UnsafeCallback {
    #refcount;
    // Internal promise only meant to keep Deno from exiting
    #refpromise;
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
      const [rid, pointer] = ops.op_ffi_unsafe_callback_create(
        definition,
        callback,
      );
      this.#refcount = 0;
      this.#rid = rid;
      this.pointer = pointer;
      this.definition = definition;
      this.callback = callback;
    }

    ref() {
      if (this.#refcount++ === 0) {
        this.#refpromise = core.opAsync(
          "op_ffi_unsafe_callback_ref",
          this.#rid,
        );
      }
      return this.#refcount;
    }

    unref() {
      // Only decrement refcount if it is positive, and only
      // unref the callback if refcount reaches zero.
      if (this.#refcount > 0 && --this.#refcount === 0) {
        ops.op_ffi_unsafe_callback_unref(this.#rid);
      }
      return this.#refcount;
    }

    close() {
      this.#refcount = 0;
      core.close(this.#rid);
    }
  }

  const UnsafeCallbackPrototype = UnsafeCallback.prototype;

  class DynamicLibrary {
    #rid;
    symbols = {};

    constructor(path, symbols) {
      [this.#rid, this.symbols] = ops.op_ffi_load({ path, symbols });
      for (const symbol in symbols) {
        if ("type" in symbols[symbol]) {
          const type = symbols[symbol].type;
          if (type === "void") {
            throw new TypeError(
              "Foreign symbol of type 'void' is not supported.",
            );
          }

          const name = symbols[symbol].name || symbol;
          const value = ops.op_ffi_get_static(
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
        const resultType = symbols[symbol].result;
        const needsUnpacking = isReturnedAsBigInt(resultType);

        const isNonBlocking = symbols[symbol].nonblocking;
        if (isNonBlocking) {
          ObjectDefineProperty(
            this.symbols,
            symbol,
            {
              configurable: false,
              enumerable: true,
              value: (...parameters) => {
                return core.opAsync(
                  "op_ffi_call_nonblocking",
                  this.#rid,
                  symbol,
                  parameters,
                );
              },
              writable: false,
            },
          );
        }

        if (needsUnpacking && !isNonBlocking) {
          const call = this.symbols[symbol];
          const parameters = symbols[symbol].parameters;
          const vi = new Int32Array(2);
          const vui = new Uint32Array(vi.buffer);
          const b = new BigInt64Array(vi.buffer);

          const params = ArrayPrototypeJoin(
            ArrayPrototypeMap(parameters, (_, index) => `p${index}`),
            ", ",
          );
          // Make sure V8 has no excuse to not optimize this function.
          this.symbols[symbol] = new Function(
            "vi",
            "vui",
            "b",
            "call",
            "NumberIsSafeInteger",
            "Number",
            `return function (${params}) {
            call(${params}${parameters.length > 0 ? ", " : ""}vi);
            ${
              isI64(resultType)
                ? `const n1 = Number(b[0])`
                : `const n1 = vui[0] + 2 ** 32 * vui[1]` // Faster path for u64
            };
            if (NumberIsSafeInteger(n1)) return n1;
            return b[0];
          }`,
          )(vi, vui, b, call, NumberIsSafeInteger, Number);
        }
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
