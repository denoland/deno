// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const __bootstrap = window.__bootstrap;
  const {
    ArrayBufferPrototype,
    ArrayPrototypePush,
    ArrayPrototypeSome,
    BigInt,
    NumberIsFinite,
    NumberIsInteger,
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
        ArrayPrototypePush(parameters, arg);
      } else if (type === "i8" || type === "i16" || type === "i32") {
        if (!NumberIsInteger(arg)) {
          throw new TypeError(
            `Expected FFI argument to be a signed integer, but got '${arg}'`,
          );
        }
        ArrayPrototypePush(parameters, arg);
      } else if (type === "u64" || type === "usize") {
        if (
          !(NumberIsInteger(arg) && arg >= 0 ||
            typeof arg === "bigint" && 0n <= arg && arg <= 0xffffffffffffffffn)
        ) {
          throw new TypeError(
            `Expected FFI argument to be an unsigned integer, but got '${arg}'`,
          );
        }
        ArrayPrototypePush(parameters, arg);
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
        ArrayPrototypePush(parameters, arg);
      } else if (type === "f32" || type === "f64") {
        if (!NumberIsFinite(arg)) {
          throw new TypeError(
            `Expected FFI argument to be a number, but got '${arg}'`,
          );
        }
        ArrayPrototypePush(parameters, arg);
      } else if (type === "pointer") {
        if (
          ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, arg?.buffer) &&
          arg.byteLength !== undefined
        ) {
          ArrayPrototypePush(parameters, arg);
        } else if (ObjectPrototypeIsPrototypeOf(UnsafePointerPrototype, arg)) {
          ArrayPrototypePush(parameters, arg.value);
        } else if (arg === null) {
          ArrayPrototypePush(parameters, null);
        } else {
          throw new TypeError(
            "Expected FFI argument to be TypedArray, UnsafePointer or null",
          );
        }
      } else if (
        typeof type === "object" && type !== null && "function" in type
      ) {
        if (ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, arg)) {
          // Own registered callback, pass the pointer value
          ArrayPrototypePush(parameters, arg.pointer.value);
        } else if (arg === null) {
          // nullptr
          ArrayPrototypePush(parameters, null);
        } else if (
          ObjectPrototypeIsPrototypeOf(UnsafeFnPointerPrototype, arg)
        ) {
          // Foreign function, pass the pointer value
          ArrayPrototypePush(parameters, arg.pointer.value);
        } else if (
          ObjectPrototypeIsPrototypeOf(UnsafePointerPrototype, arg)
        ) {
          // Foreign function, pass the pointer value
          ArrayPrototypePush(parameters, arg.value);
        } else {
          throw new TypeError(
            "Expected FFI argument to be UnsafeCallback, UnsafeFnPointer, UnsafePointer or null",
          );
        }
      } else {
        throw new TypeError(`Invalid FFI argument type '${type}'`);
      }
    }

    return parameters;
  }

  function unpackNonblockingReturnValue(type, result) {
    if (
      typeof type === "object" && type !== null && "function" in type ||
      type === "pointer"
    ) {
      return new UnsafePointer(unpackU64(result));
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
          isReturnedAsBigInt(resultType)
        ) {
          return PromisePrototypeThen(
            promise,
            (result) => unpackNonblockingReturnValue(resultType, result),
          );
        }

        return promise;
      } else {
        const result = core.opSync(
          "op_ffi_call_ptr",
          this.pointer.value,
          this.definition,
          parameters,
        );

        if (isPointerType(resultType)) {
          return new UnsafePointer(result);
        }

        return result;
      }
    }
  }

  const UnsafeFnPointerPrototype = UnsafeFnPointer.prototype;

  function isPointerType(type) {
    return type === "pointer" ||
      typeof type === "object" && type !== null && "function" in type;
  }

  function isReturnedAsBigInt(type) {
    return isPointerType(type) || type === "u64" || type === "i64" ||
      type === "usize" || type === "isize";
  }

  function prepareUnsafeCallbackParameters(types, args) {
    const parameters = [];
    if (types.length === 0) {
      return parameters;
    }

    for (let i = 0; i < types.length; i++) {
      const type = types[i];
      const arg = args[i];
      ArrayPrototypePush(
        parameters,
        isPointerType(type) ? new UnsafePointer(arg) : arg,
      );
    }

    return parameters;
  }

  function unwrapUnsafeCallbackReturnValue(result) {
    if (
      ObjectPrototypeIsPrototypeOf(UnsafePointerPrototype, result)
    ) {
      // Foreign function, return the pointer value
      ArrayPrototypePush(parameters, result.value);
    } else if (
      ObjectPrototypeIsPrototypeOf(UnsafeFnPointerPrototype, result)
    ) {
      // Foreign function, return the pointer value
      ArrayPrototypePush(parameters, result.pointer.value);
    } else if (
      ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, result)
    ) {
      // Own registered callback, return the pointer value.
      // Note that returning the ResourceId here would not work as
      // the Rust side code cannot access OpState to get the resource.
      ArrayPrototypePush(parameters, result.pointer.value);
    }
    return result;
  }

  function createInternalCallback(definition, callback) {
    const mustUnwrap = isPointerType(definition.result);
    return (...args) => {
      const convertedArgs = prepareUnsafeCallbackParameters(
        definition.parameters,
        args,
      );
      const result = callback(...convertedArgs);
      if (mustUnwrap) {
        return unwrapUnsafeCallbackReturnValue(result);
      }
      return result;
    };
  }

  class UnsafeCallback {
    #rid;
    #internal;
    definition;
    callback;
    pointer;

    constructor(definition, callback) {
      if (definition.nonblocking) {
        throw new TypeError(
          "Invalid UnsafeCallback, cannot be nonblocking",
        );
      }
      const needsWrapping = isPointerType(definition.result) ||
        ArrayPrototypeSome(definition.parameters, isPointerType);
      const internalCallback = needsWrapping
        ? createInternalCallback(definition, callback)
        : callback;

      const [rid, pointer] = core.opSync(
        "op_ffi_unsafe_callback_create",
        definition,
        internalCallback,
      );
      this.#rid = rid;
      this.pointer = new UnsafePointer(pointer);
      this.#internal = internalCallback;
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
          const needsUnpacking = isReturnedAsBigInt(resultType);
          fn = (...args) => {
            const parameters = prepareArgs(types, args);

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
          const mustWrap = isPointerType(resultType);
          fn = (...args) => {
            const parameters = prepareArgs(types, args);

            const result = core.opSync(
              "op_ffi_call",
              this.#rid,
              symbol,
              parameters,
            );

            if (mustWrap) {
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
    UnsafeCallback,
    UnsafePointer,
    UnsafePointerView,
    UnsafeFnPointer,
  };
})(this);
