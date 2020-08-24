// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  function isTypedArray(x) {
    return ArrayBuffer.isView(x) && !(x instanceof DataView);
  }

  function isInvalidDate(x) {
    return isNaN(x.getTime());
  }

  function requiredArguments(
    name,
    length,
    required,
  ) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  function immutableDefine(
    o,
    p,
    value,
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }

  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return Object.prototype.hasOwnProperty.call(obj, v);
  }

  /** Returns whether o is iterable. */
  function isIterable(
    o,
  ) {
    // checks for null and undefined
    if (o == null) {
      return false;
    }
    return (
      typeof (o)[Symbol.iterator] === "function"
    );
  }

  const objectCloneMemo = new WeakMap();

  function cloneArrayBuffer(
    srcBuffer,
    srcByteOffset,
    srcLength,
    _cloneConstructor,
  ) {
    // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
    return srcBuffer.slice(
      srcByteOffset,
      srcByteOffset + srcLength,
    );
  }

  /** Clone a value in a similar way to structured cloning.  It is similar to a
 * StructureDeserialize(StructuredSerialize(...)). */
  function cloneValue(value) {
    switch (typeof value) {
      case "number":
      case "string":
      case "boolean":
      case "undefined":
      case "bigint":
        return value;
      case "object": {
        if (objectCloneMemo.has(value)) {
          return objectCloneMemo.get(value);
        }
        if (value === null) {
          return value;
        }
        if (value instanceof Date) {
          return new Date(value.valueOf());
        }
        if (value instanceof RegExp) {
          return new RegExp(value);
        }
        if (value instanceof SharedArrayBuffer) {
          return value;
        }
        if (value instanceof ArrayBuffer) {
          const cloned = cloneArrayBuffer(
            value,
            0,
            value.byteLength,
            ArrayBuffer,
          );
          objectCloneMemo.set(value, cloned);
          return cloned;
        }
        if (ArrayBuffer.isView(value)) {
          const clonedBuffer = cloneValue(value.buffer);
          // Use DataViewConstructor type purely for type-checking, can be a
          // DataView or TypedArray.  They use the same constructor signature,
          // only DataView has a length in bytes and TypedArrays use a length in
          // terms of elements, so we adjust for that.
          let length;
          if (value instanceof DataView) {
            length = value.byteLength;
          } else {
            length = value.length;
          }
          return new (value.constructor)(
            clonedBuffer,
            value.byteOffset,
            length,
          );
        }
        if (value instanceof Map) {
          const clonedMap = new Map();
          objectCloneMemo.set(value, clonedMap);
          value.forEach((v, k) => clonedMap.set(k, cloneValue(v)));
          return clonedMap;
        }
        if (value instanceof Set) {
          const clonedSet = new Map();
          objectCloneMemo.set(value, clonedSet);
          value.forEach((v, k) => clonedSet.set(k, cloneValue(v)));
          return clonedSet;
        }

        const clonedObj = {};
        objectCloneMemo.set(value, clonedObj);
        const sourceKeys = Object.getOwnPropertyNames(value);
        for (const key of sourceKeys) {
          clonedObj[key] = cloneValue(value[key]);
        }
        return clonedObj;
      }
      case "symbol":
      case "function":
      default:
        throw new DOMException("Uncloneable value in stream", "DataCloneError");
    }
  }

  /** A helper function which ensures accessors are enumerable, as they normally
 * are not. */
  function defineEnumerableProps(
    Ctor,
    props,
  ) {
    for (const prop of props) {
      Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
    }
  }

  function getHeaderValueParams(value) {
    const params = new Map();
    // Forced to do so for some Map constructor param mismatch
    value
      .split(";")
      .slice(1)
      .map((s) => s.trim().split("="))
      .filter((arr) => arr.length > 1)
      .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
      .forEach(([k, v]) => params.set(k, v));
    return params;
  }

  function hasHeaderValueOf(s, value) {
    return new RegExp(`^${value}[\t\s]*;?`).test(s);
  }

  /** An internal function which provides a function name for some generated
 * functions, so stack traces are a bit more readable.
 */
  function setFunctionName(fn, value) {
    Object.defineProperty(fn, "name", { value, configurable: true });
  }

  window.__bootstrap.webUtil = {
    isTypedArray,
    isInvalidDate,
    requiredArguments,
    immutableDefine,
    hasOwnProperty,
    isIterable,
    cloneValue,
    defineEnumerableProps,
    getHeaderValueParams,
    hasHeaderValueOf,
    setFunctionName,
  };
})(this);
