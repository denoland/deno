// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const __bootstrap = window.__bootstrap;
  const {
    ArrayPrototypeMap,
    ArrayPrototypeJoin,
    ObjectDefineProperty,
    ObjectPrototypeHasOwnProperty,
    ObjectPrototypeIsPrototypeOf,
    Number,
    NumberIsSafeInteger,
    TypeError,
    Uint8Array,
    Int32Array,
    Uint32Array,
    BigInt64Array,
    BigUint64Array,
    Function,
    ReflectHas,
    PromisePrototypeThen,
    MathMax,
    MathCeil,
    Map,
    SafeArrayIterator,
  } = window.__bootstrap.primordials;

  const U32_BUFFER = new Uint32Array(2);
  const U64_BUFFER = new BigUint64Array(U32_BUFFER.buffer);
  const I64_BUFFER = new BigInt64Array(U32_BUFFER.buffer);
  class UnsafePointerView {
    pointer;

    constructor(pointer) {
      this.pointer = pointer;
    }

    getBool(offset = 0) {
      return ops.op_ffi_read_bool(
        this.pointer,
        offset,
      );
    }

    getUint8(offset = 0) {
      return ops.op_ffi_read_u8(
        this.pointer,
        offset,
      );
    }

    getInt8(offset = 0) {
      return ops.op_ffi_read_i8(
        this.pointer,
        offset,
      );
    }

    getUint16(offset = 0) {
      return ops.op_ffi_read_u16(
        this.pointer,
        offset,
      );
    }

    getInt16(offset = 0) {
      return ops.op_ffi_read_i16(
        this.pointer,
        offset,
      );
    }

    getUint32(offset = 0) {
      return ops.op_ffi_read_u32(
        this.pointer,
        offset,
      );
    }

    getInt32(offset = 0) {
      return ops.op_ffi_read_i32(
        this.pointer,
        offset,
      );
    }

    getBigUint64(offset = 0) {
      ops.op_ffi_read_u64(
        this.pointer,
        offset,
        U32_BUFFER,
      );
      return U64_BUFFER[0];
    }

    getBigInt64(offset = 0) {
      ops.op_ffi_read_i64(
        this.pointer,
        offset,
        U32_BUFFER,
      );
      return I64_BUFFER[0];
    }

    getFloat32(offset = 0) {
      return ops.op_ffi_read_f32(
        this.pointer,
        offset,
      );
    }

    getFloat64(offset = 0) {
      return ops.op_ffi_read_f64(
        this.pointer,
        offset,
      );
    }

    getCString(offset = 0) {
      return ops.op_ffi_cstr_read(
        this.pointer,
        offset,
      );
    }

    static getCString(pointer, offset = 0) {
      return ops.op_ffi_cstr_read(
        pointer,
        offset,
      );
    }

    getArrayBuffer(byteLength, offset = 0) {
      return ops.op_ffi_get_buf(
        this.pointer,
        offset,
        byteLength,
      );
    }

    static getArrayBuffer(pointer, byteLength, offset = 0) {
      return ops.op_ffi_get_buf(
        pointer,
        offset,
        byteLength,
      );
    }

    copyInto(destination, offset = 0) {
      ops.op_ffi_buf_copy_into(
        this.pointer,
        offset,
        destination,
        destination.byteLength,
      );
    }

    static copyInto(pointer, destination, offset = 0) {
      ops.op_ffi_buf_copy_into(
        pointer,
        offset,
        destination,
        destination.byteLength,
      );
    }
  }

  const OUT_BUFFER = new Uint32Array(2);
  const OUT_BUFFER_64 = new BigInt64Array(OUT_BUFFER.buffer);
  class UnsafePointer {
    static of(value) {
      if (ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, value)) {
        return value.pointer;
      }
      ops.op_ffi_ptr_of(value, OUT_BUFFER);
      const result = OUT_BUFFER[0] + 2 ** 32 * OUT_BUFFER[1];
      if (NumberIsSafeInteger(result)) {
        return result;
      }
      return OUT_BUFFER_64[0];
    }
  }

  class UnsafeFnPointer {
    pointer;
    definition;
    #structSize;

    constructor(pointer, definition) {
      this.pointer = pointer;
      this.definition = definition;
      this.#structSize = isStruct(definition.result)
        ? getTypeSizeAndAlignment(definition.result)[0]
        : null;
    }

    call(...parameters) {
      if (this.definition.nonblocking) {
        if (this.#structSize === null) {
          return core.opAsync(
            "op_ffi_call_ptr_nonblocking",
            this.pointer,
            this.definition,
            parameters,
          );
        } else {
          const buffer = new Uint8Array(this.#structSize);
          return PromisePrototypeThen(
            core.opAsync(
              "op_ffi_call_ptr_nonblocking",
              this.pointer,
              this.definition,
              parameters,
              buffer,
            ),
            () => buffer,
          );
        }
      } else {
        if (this.#structSize === null) {
          return ops.op_ffi_call_ptr(
            this.pointer,
            this.definition,
            parameters,
          );
        } else {
          const buffer = new Uint8Array(this.#structSize);
          ops.op_ffi_call_ptr(
            this.pointer,
            this.definition,
            parameters,
            buffer,
          );
          return buffer;
        }
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

  function isStruct(type) {
    return typeof type === "object" && type !== null &&
      typeof type.struct === "object";
  }

  function getTypeSizeAndAlignment(type, cache = new Map()) {
    if (isStruct(type)) {
      const cached = cache.get(type);
      if (cached !== undefined) {
        if (cached === null) {
          throw new TypeError("Recursive struct definition");
        }
        return cached;
      }
      cache.set(type, null);
      let size = 0;
      let alignment = 1;
      for (const field of new SafeArrayIterator(type.struct)) {
        const [fieldSize, fieldAlign] = getTypeSizeAndAlignment(field, cache);
        alignment = MathMax(alignment, fieldAlign);
        size = MathCeil(size / fieldAlign) * fieldAlign;
        size += fieldSize;
      }
      size = MathCeil(size / alignment) * alignment;
      cache.set(type, size);
      return [size, alignment];
    }

    switch (type) {
      case "bool":
      case "u8":
      case "i8":
        return [1, 1];
      case "u16":
      case "i16":
        return [2, 2];
      case "u32":
      case "i32":
      case "f32":
        return [4, 4];
      case "u64":
      case "i64":
      case "f64":
      case "pointer":
      case "buffer":
      case "function":
      case "usize":
      case "isize":
        return [8, 8];
      default:
        throw new TypeError(`Unsupported type: ${type}`);
    }
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
        if (!ObjectPrototypeHasOwnProperty(symbols, symbol)) {
          continue;
        }

        if (ReflectHas(symbols[symbol], "type")) {
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
        const isStructResult = isStruct(resultType);
        const structSize = isStructResult
          ? getTypeSizeAndAlignment(resultType)[0]
          : 0;
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
                if (isStructResult) {
                  const buffer = new Uint8Array(structSize);
                  const ret = core.opAsync(
                    "op_ffi_call_nonblocking",
                    this.#rid,
                    symbol,
                    parameters,
                    buffer,
                  );
                  return PromisePrototypeThen(
                    ret,
                    () => buffer,
                  );
                } else {
                  return core.opAsync(
                    "op_ffi_call_nonblocking",
                    this.#rid,
                    symbol,
                    parameters,
                  );
                }
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
        } else if (isStructResult && !isNonBlocking) {
          const call = this.symbols[symbol];
          const parameters = symbols[symbol].parameters;
          const params = ArrayPrototypeJoin(
            ArrayPrototypeMap(parameters, (_, index) => `p${index}`),
            ", ",
          );
          this.symbols[symbol] = new Function(
            "call",
            `return function (${params}) {
            const buffer = new Uint8Array(${structSize});
            call(${params}${parameters.length > 0 ? ", " : ""}buffer);
            return buffer;
          }`,
          )(call);
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
