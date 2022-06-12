// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const __bootstrap = window.__bootstrap;
  const {
    ArrayBufferPrototype,
    BigInt,
    Error,
    NumberIsFinite,
    NumberIsInteger,
    ObjectDefineProperty,
    ObjectPrototypeIsPrototypeOf,
    Symbol,
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
        this.pointer.value + BigInt(offset),
      );
    }

    getInt8(offset = 0) {
      return core.opSync(
        "op_ffi_read_i8",
        this.pointer.value + BigInt(offset),
      );
    }

    getUint16(offset = 0) {
      return core.opSync(
        "op_ffi_read_u16",
        this.pointer.value + BigInt(offset),
      );
    }

    getInt16(offset = 0) {
      return core.opSync(
        "op_ffi_read_i16",
        this.pointer.value + BigInt(offset),
      );
    }

    getUint32(offset = 0) {
      return core.opSync(
        "op_ffi_read_u32",
        this.pointer.value + BigInt(offset),
      );
    }

    getInt32(offset = 0) {
      return core.opSync(
        "op_ffi_read_i32",
        this.pointer.value + BigInt(offset),
      );
    }

    getBigUint64(offset = 0) {
      return core.opSync(
        "op_ffi_read_u64",
        this.pointer.value + BigInt(offset),
      );
    }

    getBigInt64(offset = 0) {
      return core.opSync(
        "op_ffi_read_u64",
        this.pointer.value + BigInt(offset),
      );
    }

    getFloat32(offset = 0) {
      return core.opSync(
        "op_ffi_read_f32",
        this.pointer.value + BigInt(offset),
      );
    }

    getFloat64(offset = 0) {
      return core.opSync(
        "op_ffi_read_f64",
        this.pointer.value + BigInt(offset),
      );
    }

    getCString(offset = 0) {
      return core.opSync(
        "op_ffi_cstr_read",
        this.pointer.value + BigInt(offset),
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
        this.pointer.value + BigInt(offset),
        destination,
        destination.byteLength,
      );
    }
  }

  class UnsafePointer {
    value;

    constructor(value) {
      if (typeof value === "number") {
        value = BigInt(value);
      }
      this.value = value;
    }

    static of(typedArray) {
      return new UnsafePointer(
        core.opSync("op_ffi_ptr_of", typedArray),
      );
    }

    valueOf() {
      return this.value;
    }
  }
  const UnsafePointerPrototype = UnsafePointer.prototype;

  function prepareArgs(types, args) {
    const parameters = [];

    if (types.length !== args.length) {
      throw new TypeError("Invalid FFI call, parameter vs args count mismatch");
    }

    for (let i = 0; i < types.length; i++) {
      const type = types[i];
      const arg = args[i];

      if (type === "u8" || type === "u16" || type === "u32") {
        if (!NumberIsInteger(arg) || arg < 0) {
          throw new TypeError(
            `Expected FFI argument to be an unsigned integer, but got '${arg}'`,
          );
        }
        parameters.push(arg);
      } else if (type === "i8" || type === "i16" || type === "i32") {
        if (!NumberIsInteger(arg)) {
          throw new TypeError(
            `Expected FFI argument to be a signed integer, but got '${arg}'`,
          );
        }
        parameters.push(arg);
      } else if (type === "u64" || type === "usize") {
        if (
          !(NumberIsInteger(arg) && arg >= 0 ||
            typeof arg === "bigint" && 0n <= arg && arg <= 0xffffffffffffffffn)
        ) {
          throw new TypeError(
            `Expected FFI argument to be an unsigned integer, but got '${arg}'`,
          );
        }
        parameters.push(arg);
      } else if (type == "i64" || type === "isize") {
        if (
          !(NumberIsInteger(arg) ||
            typeof arg === "bigint" && -1n * 2n ** 63n <= arg &&
              arg <= 2n ** 63n - 1n)
        ) {
          throw new TypeError(
            `Expected FFI argument to be a signed integer, but got '${arg}'`,
          );
        }
        parameters.push(arg);
      } else if (type === "f32" || type === "f64") {
        if (!NumberIsFinite(arg)) {
          throw new TypeError(
            `Expected FFI argument to be a number, but got '${arg}'`,
          );
        }
        parameters.push(arg);
      } else if (type === "pointer") {
        if (
          ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, arg?.buffer) &&
          arg.byteLength !== undefined
        ) {
          parameters.push(arg);
        } else if (ObjectPrototypeIsPrototypeOf(UnsafePointerPrototype, arg)) {
          parameters.push(arg.value);
        } else if (arg === null) {
          parameters.push(null);
        } else {
          throw new TypeError(
            "Expected FFI argument to be TypedArray, UnsafePointer or null",
          );
        }
      } else if (
        typeof type === "object" && type !== null && "function" in type
      ) {
        if (ObjectPrototypeIsPrototypeOf(RegisteredCallbackPrototype, arg)) {
          parameters.push(arg[_rid]);
        } else if (arg === null) {
          // nullptr
          parameters.push(null);
        } else if (
          ObjectPrototypeIsPrototypeOf(UnsafeFnPointerPrototype, arg) ||
          ObjectPrototypeIsPrototypeOf(UnsafePointerPrototype, arg)
        ) {
          // Foreign function given to us, we're passing it on
          parameters.push(arg.value);
        } else {
          throw new TypeError(
            "Expected FFI argument to be RegisteredCallback, UnsafeFn",
          );
        }
      } else {
        throw new TypeError(`Invalid FFI argument type '${type}'`);
      }
    }

    return parameters;
  }

  function unpackResult(type, result) {
    switch (type) {
      case "isize":
      case "i64":
        return unpackI64(result);
      case "usize":
      case "u64":
        return unpackU64(result);
      case "pointer":
        return new UnsafePointer(unpackU64(result));
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

    call(...args) {
      const parameters = prepareArgs(
        this.definition.parameters,
        args,
      );
      const resultType = this.definition.result;
      if (this.definition.nonblocking) {
        const promise = core.opAsync(
          "op_ffi_call_ptr_nonblocking",
          this.pointer.value,
          this.definition,
          parameters,
        );

        if (
          resultType === "pointer" || resultType === "u64" ||
          resultType === "i64" || resultType === "usize" ||
          resultType === "isize"
        ) {
          return promise.then((result) => unpackResult(resultType, result));
        }

        return promise;
      } else {
        const result = core.opSync(
          "op_ffi_call_ptr",
          this.pointer.value,
          this.definition,
          parameters,
        );

        if (resultType === "pointer") {
          return new UnsafePointer(result);
        }

        return result;
      }
    }
  }

  const UnsafeFnPointerPrototype = UnsafeFnPointer.prototype;

  const _rid = Symbol("[[rid]]");

  class RegisteredCallback {
    [_rid];
    #value;
    definition;
    callback;

    constructor(definition, callback) {
      if (definition.nonblocking) {
        throw new TypeError(
          "Invalid ffi RegisteredCallback, cannot be nonblocking",
        );
      }
      this[_rid] = core.opSync(
        "op_ffi_register_callback",
        definition,
        callback,
      );
      this.definition = definition;
      this.callback = callback;
    }

    call(...args) {
      const parameters = prepareArgs(
        this.definition.parameters,
        args,
      );
      if (this.definition.nonblocking) {
        throw new Error("Unreachable");
      } else {
        const result = core.opSync(
          "op_ffi_call_registered_callback",
          this[_rid],
          parameters,
        );

        if (this.definition.result === "pointer") {
          return new UnsafePointer(result);
        }

        return result;
      }
    }

    close() {
      core.close(this[_rid]);
    }

    get value() {
      if (!this.#value) {
        this.#value = core.opSync("op_ffi_ptr_of_cb", this[_rid]);
      }
      return this.#value;
    }
  }

  const RegisteredCallbackPrototype = RegisteredCallback.prototype;

  function registerCallback(definition, callback) {
    if (!definition || !callback) {
      throw new TypeError("Invalid arguments");
    }
    return new RegisteredCallback(definition, callback);
  }

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
          let value = core.opSync(
            "op_ffi_get_static",
            this.#rid,
            name,
            type,
          );
          if (type === "pointer") {
            value = new UnsafePointer(value);
          }
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
        const types = symbols[symbol].parameters;
        const resultType = symbols[symbol].result;

        let fn;
        if (isNonBlocking) {
          const needsUnpacking = resultType === "pointer" ||
            resultType === "u64" ||
            resultType === "i64" || resultType === "usize" ||
            resultType === "isize";
          fn = (...args) => {
            const parameters = prepareArgs(types, args);

            const promise = core.opAsync(
              "op_ffi_call_nonblocking",
              this.#rid,
              symbol,
              parameters,
            );

            if (needsUnpacking) {
              return promise.then((result) => unpackResult(resultType, result));
            }

            return promise;
          };
        } else {
          fn = (...args) => {
            const parameters = prepareArgs(types, args);

            const result = core.opSync(
              "op_ffi_call",
              this.#rid,
              symbol,
              parameters,
            );

            if (resultType === "pointer") {
              return new UnsafePointer(result);
            }

            return result;
          };
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
    registerCallback,
    UnsafePointer,
    UnsafePointerView,
    UnsafeFnPointer,
  };
})(this);
