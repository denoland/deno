// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/util.ts", [], function (exports_27, context_27) {
  "use strict";
  const __moduleName = context_27 && context_27.id;
  // @internal
  function isTypedArray(x) {
    return (
      x instanceof Int8Array ||
      x instanceof Uint8Array ||
      x instanceof Uint8ClampedArray ||
      x instanceof Int16Array ||
      x instanceof Uint16Array ||
      x instanceof Int32Array ||
      x instanceof Uint32Array ||
      x instanceof Float32Array ||
      x instanceof Float64Array
    );
  }
  exports_27("isTypedArray", isTypedArray);
  // @internal
  function requiredArguments(name, length, required) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }
  exports_27("requiredArguments", requiredArguments);
  // @internal
  function immutableDefine(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    o,
    p,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }
  exports_27("immutableDefine", immutableDefine);
  // @internal
  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return Object.prototype.hasOwnProperty.call(obj, v);
  }
  exports_27("hasOwnProperty", hasOwnProperty);
  /** Returns whether o is iterable.
   *
   * @internal */
  function isIterable(o) {
    // checks for null and undefined
    if (o == null) {
      return false;
    }
    return typeof o[Symbol.iterator] === "function";
  }
  exports_27("isIterable", isIterable);
  /** A helper function which ensures accessors are enumerable, as they normally
   * are not. */
  function defineEnumerableProps(Ctor, props) {
    for (const prop of props) {
      Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
    }
  }
  exports_27("defineEnumerableProps", defineEnumerableProps);
  return {
    setters: [],
    execute: function () {},
  };
});
