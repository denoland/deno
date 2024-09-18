// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  isArrayBuffer,
  isDataView,
  isTypedArray,
} = core;
import {
  op_ffi_buf_copy_into,
  op_ffi_call_nonblocking,
  op_ffi_call_ptr,
  op_ffi_call_ptr_nonblocking,
  op_ffi_cstr_read,
  op_ffi_get_buf,
  op_ffi_get_static,
  op_ffi_load,
  op_ffi_ptr_create,
  op_ffi_ptr_equals,
  op_ffi_ptr_of,
  op_ffi_ptr_of_exact,
  op_ffi_ptr_offset,
  op_ffi_ptr_value,
  op_ffi_read_bool,
  op_ffi_read_f32,
  op_ffi_read_f64,
  op_ffi_read_i16,
  op_ffi_read_i32,
  op_ffi_read_i64,
  op_ffi_read_i8,
  op_ffi_read_ptr,
  op_ffi_read_u16,
  op_ffi_read_u32,
  op_ffi_read_u64,
  op_ffi_read_u8,
  op_ffi_unsafe_callback_close,
  op_ffi_unsafe_callback_create,
  op_ffi_unsafe_callback_ref,
} from "ext:core/ops";
const {
  ArrayBufferIsView,
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeMap,
  ArrayPrototypeJoin,
  BigInt,
  DataViewPrototypeGetByteLength,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  TypedArrayPrototypeGetByteLength,
  TypeError,
  Uint8Array,
  Function,
  ReflectHas,
  PromisePrototypeThen,
  MathMax,
  MathCeil,
  SafeMap,
  SafeArrayIterator,
  SafeWeakMap,
} = primordials;

import { pathFromURL } from "ext:deno_web/00_infra.js";

/**
 * @param {BufferSource} source
 * @returns {number}
 */
function getBufferSourceByteLength(source) {
  if (isTypedArray(source)) {
    return TypedArrayPrototypeGetByteLength(source);
  } else if (isDataView(source)) {
    return DataViewPrototypeGetByteLength(source);
  }
  return ArrayBufferPrototypeGetByteLength(source);
}
class UnsafePointerView {
  pointer;

  constructor(pointer) {
    this.pointer = pointer;
  }

  getBool(offset = 0) {
    return op_ffi_read_bool(
      this.pointer,
      offset,
    );
  }

  getUint8(offset = 0) {
    return op_ffi_read_u8(
      this.pointer,
      offset,
    );
  }

  getInt8(offset = 0) {
    return op_ffi_read_i8(
      this.pointer,
      offset,
    );
  }

  getUint16(offset = 0) {
    return op_ffi_read_u16(
      this.pointer,
      offset,
    );
  }

  getInt16(offset = 0) {
    return op_ffi_read_i16(
      this.pointer,
      offset,
    );
  }

  getUint32(offset = 0) {
    return op_ffi_read_u32(
      this.pointer,
      offset,
    );
  }

  getInt32(offset = 0) {
    return op_ffi_read_i32(
      this.pointer,
      offset,
    );
  }

  getBigUint64(offset = 0) {
    return op_ffi_read_u64(
      this.pointer,
      // We return a BigInt, so the turbocall
      // is forced to use BigInts everywhere.
      BigInt(offset),
    );
  }

  getBigInt64(offset = 0) {
    return op_ffi_read_i64(
      this.pointer,
      // We return a BigInt, so the turbocall
      // is forced to use BigInts everywhere.
      BigInt(offset),
    );
  }

  getFloat32(offset = 0) {
    return op_ffi_read_f32(
      this.pointer,
      offset,
    );
  }

  getFloat64(offset = 0) {
    return op_ffi_read_f64(
      this.pointer,
      offset,
    );
  }

  getPointer(offset = 0) {
    return op_ffi_read_ptr(
      this.pointer,
      offset,
    );
  }

  getCString(offset = 0) {
    return op_ffi_cstr_read(
      this.pointer,
      offset,
    );
  }

  static getCString(pointer, offset = 0) {
    return op_ffi_cstr_read(
      pointer,
      offset,
    );
  }

  getArrayBuffer(byteLength, offset = 0) {
    return op_ffi_get_buf(
      this.pointer,
      offset,
      byteLength,
    );
  }

  static getArrayBuffer(pointer, byteLength, offset = 0) {
    return op_ffi_get_buf(
      pointer,
      offset,
      byteLength,
    );
  }

  copyInto(destination, offset = 0) {
    op_ffi_buf_copy_into(
      this.pointer,
      offset,
      destination,
      getBufferSourceByteLength(destination),
    );
  }

  static copyInto(pointer, destination, offset = 0) {
    op_ffi_buf_copy_into(
      pointer,
      offset,
      destination,
      getBufferSourceByteLength(destination),
    );
  }
}

const POINTER_TO_BUFFER_WEAK_MAP = new SafeWeakMap();
class UnsafePointer {
  static create(value) {
    return op_ffi_ptr_create(value);
  }

  static equals(a, b) {
    if (a === null || b === null) {
      return a === b;
    }
    return op_ffi_ptr_equals(a, b);
  }

  static of(value) {
    if (ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, value)) {
      return value.pointer;
    }
    let pointer;
    if (ArrayBufferIsView(value)) {
      if (value.length === 0) {
        pointer = op_ffi_ptr_of_exact(value);
      } else {
        pointer = op_ffi_ptr_of(value);
      }
    } else if (isArrayBuffer(value)) {
      if (value.length === 0) {
        pointer = op_ffi_ptr_of_exact(new Uint8Array(value));
      } else {
        pointer = op_ffi_ptr_of(new Uint8Array(value));
      }
    } else {
      throw new TypeError(
        `Cannot access pointer: expected 'ArrayBuffer', 'ArrayBufferView' or 'UnsafeCallbackPrototype', received ${typeof value}`,
      );
    }
    if (pointer) {
      POINTER_TO_BUFFER_WEAK_MAP.set(pointer, value);
    }
    return pointer;
  }

  static offset(value, offset) {
    return op_ffi_ptr_offset(value, offset);
  }

  static value(value) {
    if (ObjectPrototypeIsPrototypeOf(UnsafeCallbackPrototype, value)) {
      value = value.pointer;
    }
    return op_ffi_ptr_value(value);
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
        return op_ffi_call_ptr_nonblocking(
          this.pointer,
          this.definition,
          parameters,
        );
      } else {
        const buffer = new Uint8Array(this.#structSize);
        return PromisePrototypeThen(
          op_ffi_call_ptr_nonblocking(
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
        return op_ffi_call_ptr(
          this.pointer,
          this.definition,
          parameters,
        );
      } else {
        const buffer = new Uint8Array(this.#structSize);
        op_ffi_call_ptr(
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

function isStruct(type) {
  return typeof type === "object" && type !== null &&
    typeof type.struct === "object";
}

function getTypeSizeAndAlignment(type, cache = new SafeMap()) {
  if (isStruct(type)) {
    const cached = cache.get(type);
    if (cached !== undefined) {
      if (cached === null) {
        throw new TypeError(
          "Cannot get pointer size: found recursive struct",
        );
      }
      return cached;
    }
    cache.set(type, null);
    let size = 0;
    let alignment = 1;
    for (const field of new SafeArrayIterator(type.struct)) {
      const { 0: fieldSize, 1: fieldAlign } = getTypeSizeAndAlignment(
        field,
        cache,
      );
      alignment = MathMax(alignment, fieldAlign);
      size = MathCeil(size / fieldAlign) * fieldAlign;
      size += fieldSize;
    }
    size = MathCeil(size / alignment) * alignment;
    const result = [size, alignment];
    cache.set(type, result);
    return result;
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
      throw new TypeError(`Cannot get pointer size, unsupported type: ${type}`);
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
        "Cannot construct UnsafeCallback: cannot be nonblocking",
      );
    }
    const { 0: rid, 1: pointer } = op_ffi_unsafe_callback_create(
      definition,
      callback,
    );
    this.#refcount = 0;
    this.#rid = rid;
    this.pointer = pointer;
    this.definition = definition;
    this.callback = callback;
  }

  static threadSafe(definition, callback) {
    const unsafeCallback = new UnsafeCallback(definition, callback);
    unsafeCallback.ref();
    return unsafeCallback;
  }

  ref() {
    if (this.#refcount++ === 0) {
      if (this.#refpromise) {
        // Re-refing
        core.refOpPromise(this.#refpromise);
      } else {
        this.#refpromise = op_ffi_unsafe_callback_ref(
          this.#rid,
        );
      }
    }
    return this.#refcount;
  }

  unref() {
    // Only decrement refcount if it is positive, and only
    // unref the callback if refcount reaches zero.
    if (this.#refcount > 0 && --this.#refcount === 0) {
      core.unrefOpPromise(this.#refpromise);
    }
    return this.#refcount;
  }

  close() {
    this.#refcount = 0;
    op_ffi_unsafe_callback_close(this.#rid);
  }
}

const UnsafeCallbackPrototype = UnsafeCallback.prototype;

class DynamicLibrary {
  #rid;
  symbols = { __proto__: null };

  constructor(path, symbols) {
    ({ 0: this.#rid, 1: this.symbols } = op_ffi_load({ path, symbols }));
    for (const symbol in symbols) {
      if (!ObjectHasOwn(symbols, symbol)) {
        continue;
      }

      // Symbol was marked as optional, and not found.
      // In that case, we set its value to null in Rust-side.
      if (symbols[symbol] === null) {
        continue;
      }

      if (ReflectHas(symbols[symbol], "type")) {
        const type = symbols[symbol].type;
        if (type === "void") {
          throw new TypeError(
            "Foreign symbol of type 'void' is not supported",
          );
        }

        const name = symbols[symbol].name || symbol;
        const value = op_ffi_get_static(
          this.#rid,
          name,
          type,
          symbols[symbol].optional,
        );
        ObjectDefineProperty(
          this.symbols,
          symbol,
          {
            __proto__: null,
            configurable: false,
            enumerable: true,
            writable: false,
            value,
          },
        );
        continue;
      }
      const resultType = symbols[symbol].result;
      const isStructResult = isStruct(resultType);
      const structSize = isStructResult
        ? getTypeSizeAndAlignment(resultType)[0]
        : 0;

      const isNonBlocking = symbols[symbol].nonblocking;
      if (isNonBlocking) {
        ObjectDefineProperty(
          this.symbols,
          symbol,
          {
            __proto__: null,
            configurable: false,
            enumerable: true,
            writable: false,
            value: (...parameters) => {
              if (isStructResult) {
                const buffer = new Uint8Array(structSize);
                const ret = op_ffi_call_nonblocking(
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
                return op_ffi_call_nonblocking(
                  this.#rid,
                  symbol,
                  parameters,
                );
              }
            },
          },
        );
      }

      if (isStructResult && !isNonBlocking) {
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
  return new DynamicLibrary(pathFromURL(path), symbols);
}

export {
  dlopen,
  UnsafeCallback,
  UnsafeFnPointer,
  UnsafePointer,
  UnsafePointerView,
};
