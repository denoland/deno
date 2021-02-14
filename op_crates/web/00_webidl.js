// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// Adapted from https://github.com/jsdom/webidl-conversions.
// Copyright Domenic Denicola. Licensed under BSD-2-Clause License.
// Original license at https://github.com/jsdom/webidl-conversions/blob/master/LICENSE.md.

"use strict";

((window) => {
  function makeException(ErrorType, message, opts = {}) {
    if (opts.globals) {
      ErrorType = opts.globals[ErrorType.name];
    }
    return new ErrorType(
      `${opts.prefix ? opts.prefix + ": " : ""}${
        opts.context ? opts.context : "Value"
      } ${message}.`,
    );
  }

  function toNumber(value, opts = {}) {
    if (!opts.globals) {
      return +value;
    }
    if (typeof value === "bigint") {
      throw opts.globals.TypeError("Cannot convert a BigInt value to a number");
    }
    return opts.globals.Number(value);
  }

  function type(V) {
    if (V === null) {
      return "Null";
    }
    switch (typeof V) {
      case "undefined":
        return "Undefined";
      case "boolean":
        return "Boolean";
      case "number":
        return "Number";
      case "string":
        return "String";
      case "symbol":
        return "Symbol";
      case "bigint":
        return "BigInt";
      case "object":
      // Falls through
      case "function":
      // Falls through
      default:
        // Per ES spec, typeof returns an implemention-defined value that is not any of the existing ones for
        // uncallable non-standard exotic objects. Yet Type() which the Web IDL spec depends on returns Object for
        // such cases. So treat the default case as an object.
        return "Object";
    }
  }

  // Round x to the nearest integer, choosing the even integer if it lies halfway between two.
  function evenRound(x) {
    // There are four cases for numbers with fractional part being .5:
    //
    // case |     x     | floor(x) | round(x) | expected | x <> 0 | x % 1 | x & 1 |   example
    //   1  |  2n + 0.5 |  2n      |  2n + 1  |  2n      |   >    |  0.5  |   0   |  0.5 ->  0
    //   2  |  2n + 1.5 |  2n + 1  |  2n + 2  |  2n + 2  |   >    |  0.5  |   1   |  1.5 ->  2
    //   3  | -2n - 0.5 | -2n - 1  | -2n      | -2n      |   <    | -0.5  |   0   | -0.5 ->  0
    //   4  | -2n - 1.5 | -2n - 2  | -2n - 1  | -2n - 2  |   <    | -0.5  |   1   | -1.5 -> -2
    // (where n is a non-negative integer)
    //
    // Branch here for cases 1 and 4
    if (
      (x > 0 && x % 1 === +0.5 && (x & 1) === 0) ||
      (x < 0 && x % 1 === -0.5 && (x & 1) === 1)
    ) {
      return censorNegativeZero(Math.floor(x));
    }

    return censorNegativeZero(Math.round(x));
  }

  function integerPart(n) {
    return censorNegativeZero(Math.trunc(n));
  }

  function sign(x) {
    return x < 0 ? -1 : 1;
  }

  function modulo(x, y) {
    // https://tc39.github.io/ecma262/#eqn-modulo
    // Note that http://stackoverflow.com/a/4467559/3191 does NOT work for large modulos
    const signMightNotMatch = x % y;
    if (sign(y) !== sign(signMightNotMatch)) {
      return signMightNotMatch + y;
    }
    return signMightNotMatch;
  }

  function censorNegativeZero(x) {
    return x === 0 ? 0 : x;
  }

  function createIntegerConversion(bitLength, typeOpts) {
    const isSigned = !typeOpts.unsigned;

    let lowerBound;
    let upperBound;
    if (bitLength === 64) {
      upperBound = Number.MAX_SAFE_INTEGER;
      lowerBound = !isSigned ? 0 : Number.MIN_SAFE_INTEGER;
    } else if (!isSigned) {
      lowerBound = 0;
      upperBound = Math.pow(2, bitLength) - 1;
    } else {
      lowerBound = -Math.pow(2, bitLength - 1);
      upperBound = Math.pow(2, bitLength - 1) - 1;
    }

    const twoToTheBitLength = Math.pow(2, bitLength);
    const twoToOneLessThanTheBitLength = Math.pow(2, bitLength - 1);

    return (V, opts = {}) => {
      let x = toNumber(V, opts);
      x = censorNegativeZero(x);

      if (opts.enforceRange) {
        if (!Number.isFinite(x)) {
          throw makeException(TypeError, "is not a finite number", opts);
        }

        x = integerPart(x);

        if (x < lowerBound || x > upperBound) {
          throw makeException(
            TypeError,
            `is outside the accepted range of ${lowerBound} to ${upperBound}, inclusive`,
            opts,
          );
        }

        return x;
      }

      if (!Number.isNaN(x) && opts.clamp) {
        x = Math.min(Math.max(x, lowerBound), upperBound);
        x = evenRound(x);
        return x;
      }

      if (!Number.isFinite(x) || x === 0) {
        return 0;
      }
      x = integerPart(x);

      // Math.pow(2, 64) is not accurately representable in JavaScript, so try to avoid these per-spec operations if
      // possible. Hopefully it's an optimization for the non-64-bitLength cases too.
      if (x >= lowerBound && x <= upperBound) {
        return x;
      }

      // These will not work great for bitLength of 64, but oh well. See the README for more details.
      x = modulo(x, twoToTheBitLength);
      if (isSigned && x >= twoToOneLessThanTheBitLength) {
        return x - twoToTheBitLength;
      }
      return x;
    };
  }

  function createLongLongConversion(bitLength, { unsigned }) {
    const upperBound = Number.MAX_SAFE_INTEGER;
    const lowerBound = unsigned ? 0 : Number.MIN_SAFE_INTEGER;
    const asBigIntN = unsigned ? BigInt.asUintN : BigInt.asIntN;

    return (V, opts = {}) => {
      let x = toNumber(V, opts);
      x = censorNegativeZero(x);

      if (opts.enforceRange) {
        if (!Number.isFinite(x)) {
          throw makeException(TypeError, "is not a finite number", opts);
        }

        x = integerPart(x);

        if (x < lowerBound || x > upperBound) {
          throw makeException(
            TypeError,
            `is outside the accepted range of ${lowerBound} to ${upperBound}, inclusive`,
            opts,
          );
        }

        return x;
      }

      if (!Number.isNaN(x) && opts.clamp) {
        x = Math.min(Math.max(x, lowerBound), upperBound);
        x = evenRound(x);
        return x;
      }

      if (!Number.isFinite(x) || x === 0) {
        return 0;
      }

      let xBigInt = BigInt(integerPart(x));
      xBigInt = asBigIntN(bitLength, xBigInt);
      return Number(xBigInt);
    };
  }

  const converters = [];

  converters.any = (V) => {
    return V;
  };

  converters.boolean = function (val) {
    return !!val;
  };

  converters.byte = createIntegerConversion(8, { unsigned: false });
  converters.octet = createIntegerConversion(8, { unsigned: true });

  converters.short = createIntegerConversion(16, { unsigned: false });
  converters["unsigned short"] = createIntegerConversion(16, {
    unsigned: true,
  });

  converters.long = createIntegerConversion(32, { unsigned: false });
  converters["unsigned long"] = createIntegerConversion(32, { unsigned: true });

  converters["long long"] = createLongLongConversion(64, { unsigned: false });
  converters["unsigned long long"] = createLongLongConversion(64, {
    unsigned: true,
  });

  converters.float = (V, opts) => {
    const x = toNumber(V, opts);

    if (!Number.isFinite(x)) {
      throw makeException(
        TypeError,
        "is not a finite floating-point value",
        opts,
      );
    }

    if (Object.is(x, -0)) {
      return x;
    }

    const y = Math.fround(x);

    if (!Number.isFinite(y)) {
      throw makeException(
        TypeError,
        "is outside the range of a single-precision floating-point value",
        opts,
      );
    }

    return y;
  };

  converters["unrestricted float"] = (V, opts) => {
    const x = toNumber(V, opts);

    if (isNaN(x)) {
      return x;
    }

    if (Object.is(x, -0)) {
      return x;
    }

    return Math.fround(x);
  };

  converters.double = (V, opts) => {
    const x = toNumber(V, opts);

    if (!Number.isFinite(x)) {
      throw makeException(
        TypeError,
        "is not a finite floating-point value",
        opts,
      );
    }

    return x;
  };

  converters["unrestricted double"] = (V, opts) => {
    const x = toNumber(V, opts);

    return x;
  };

  converters.DOMString = function (V, opts = {}) {
    if (opts.treatNullAsEmptyString && V === null) {
      return "";
    }

    if (typeof V === "symbol") {
      throw makeException(
        TypeError,
        "is a symbol, which cannot be converted to a string",
        opts,
      );
    }

    const StringCtor = opts.globals ? opts.globals.String : String;
    return StringCtor(V);
  };

  converters.ByteString = (V, opts) => {
    const x = converters.DOMString(V, opts);
    let c;
    for (let i = 0; (c = x.codePointAt(i)) !== undefined; ++i) {
      if (c > 255) {
        throw makeException(TypeError, "is not a valid ByteString", opts);
      }
    }

    return x;
  };

  converters.USVString = (V, opts) => {
    const S = converters.DOMString(V, opts);
    const n = S.length;
    const U = [];
    for (let i = 0; i < n; ++i) {
      const c = S.charCodeAt(i);
      if (c < 0xd800 || c > 0xdfff) {
        U.push(String.fromCodePoint(c));
      } else if (0xdc00 <= c && c <= 0xdfff) {
        U.push(String.fromCodePoint(0xfffd));
      } else if (i === n - 1) {
        U.push(String.fromCodePoint(0xfffd));
      } else {
        const d = S.charCodeAt(i + 1);
        if (0xdc00 <= d && d <= 0xdfff) {
          const a = c & 0x3ff;
          const b = d & 0x3ff;
          U.push(String.fromCodePoint((2 << 15) + (2 << 9) * a + b));
          ++i;
        } else {
          U.push(String.fromCodePoint(0xfffd));
        }
      }
    }

    return U.join("");
  };

  converters.object = (V, opts) => {
    if (type(V) !== "Object") {
      throw makeException(TypeError, "is not an object", opts);
    }

    return V;
  };

  // Not exported, but used in Function and VoidFunction.

  // Neither Function nor VoidFunction is defined with [TreatNonObjectAsNull], so
  // handling for that is omitted.
  function convertCallbackFunction(V, opts) {
    if (typeof V !== "function") {
      throw makeException(TypeError, "is not a function", opts);
    }
    return V;
  }

  const abByteLengthGetter = Object.getOwnPropertyDescriptor(
    ArrayBuffer.prototype,
    "byteLength",
  ).get;

  function isNonSharedArrayBuffer(V) {
    try {
      // This will throw on SharedArrayBuffers, but not detached ArrayBuffers.
      // (The spec says it should throw, but the spec conflicts with implementations: https://github.com/tc39/ecma262/issues/678)
      abByteLengthGetter.call(V);

      return true;
    } catch {
      return false;
    }
  }

  let sabByteLengthGetter;

  function isSharedArrayBuffer(V) {
    // TODO(lucacasonato): vulnerable to prototype pollution. Needs to happen
    // here because SharedArrayBuffer is not available during snapshotting.
    if (!sabByteLengthGetter) {
      sabByteLengthGetter = Object.getOwnPropertyDescriptor(
        SharedArrayBuffer.prototype,
        "byteLength",
      ).get;
    }
    try {
      sabByteLengthGetter.call(V);
      return true;
    } catch {
      return false;
    }
  }

  function isArrayBufferDetached(V) {
    try {
      // eslint-disable-next-line no-new
      new Uint8Array(V);
      return false;
    } catch {
      return true;
    }
  }

  converters.ArrayBuffer = (V, opts = {}) => {
    if (!isNonSharedArrayBuffer(V)) {
      if (opts.allowShared && !isSharedArrayBuffer(V)) {
        throw makeException(
          TypeError,
          "is not an ArrayBuffer or SharedArrayBuffer",
          opts,
        );
      }
      throw makeException(TypeError, "is not an ArrayBuffer", opts);
    }
    if (isArrayBufferDetached(V)) {
      throw makeException(TypeError, "is a detached ArrayBuffer", opts);
    }

    return V;
  };

  const dvByteLengthGetter = Object.getOwnPropertyDescriptor(
    DataView.prototype,
    "byteLength",
  ).get;
  converters.DataView = (V, opts = {}) => {
    try {
      dvByteLengthGetter.call(V);
    } catch (e) {
      throw makeException(TypeError, "is not a DataView", opts);
    }

    if (!opts.allowShared && isSharedArrayBuffer(V.buffer)) {
      throw makeException(
        TypeError,
        "is backed by a SharedArrayBuffer, which is not allowed",
        opts,
      );
    }
    if (isArrayBufferDetached(V.buffer)) {
      throw makeException(
        TypeError,
        "is backed by a detached ArrayBuffer",
        opts,
      );
    }

    return V;
  };

  // Returns the unforgeable `TypedArray` constructor name or `undefined`,
  // if the `this` value isn't a valid `TypedArray` object.
  //
  // https://tc39.es/ecma262/#sec-get-%typedarray%.prototype-@@tostringtag
  const typedArrayNameGetter = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(Uint8Array).prototype,
    Symbol.toStringTag,
  ).get;
  [
    Int8Array,
    Int16Array,
    Int32Array,
    Uint8Array,
    Uint16Array,
    Uint32Array,
    Uint8ClampedArray,
    Float32Array,
    Float64Array,
  ].forEach((func) => {
    const name = func.name;
    const article = /^[AEIOU]/.test(name) ? "an" : "a";
    converters[name] = (V, opts = {}) => {
      if (!ArrayBuffer.isView(V) || typedArrayNameGetter.call(V) !== name) {
        throw makeException(
          TypeError,
          `is not ${article} ${name} object`,
          opts,
        );
      }
      if (!opts.allowShared && isSharedArrayBuffer(V.buffer)) {
        throw makeException(
          TypeError,
          "is a view on a SharedArrayBuffer, which is not allowed",
          opts,
        );
      }
      if (isArrayBufferDetached(V.buffer)) {
        throw makeException(
          TypeError,
          "is a view on a detached ArrayBuffer",
          opts,
        );
      }

      return V;
    };
  });

  // Common definitions

  converters.ArrayBufferView = (V, opts = {}) => {
    if (!ArrayBuffer.isView(V)) {
      throw makeException(
        TypeError,
        "is not a view on an ArrayBuffer or SharedArrayBuffer",
        opts,
      );
    }

    if (!opts.allowShared && isSharedArrayBuffer(V.buffer)) {
      throw makeException(
        TypeError,
        "is a view on a SharedArrayBuffer, which is not allowed",
        opts,
      );
    }

    if (isArrayBufferDetached(V.buffer)) {
      throw makeException(
        TypeError,
        "is a view on a detached ArrayBuffer",
        opts,
      );
    }
    return V;
  };

  converters.BufferSource = (V, opts = {}) => {
    if (ArrayBuffer.isView(V)) {
      if (!opts.allowShared && isSharedArrayBuffer(V.buffer)) {
        throw makeException(
          TypeError,
          "is a view on a SharedArrayBuffer, which is not allowed",
          opts,
        );
      }

      if (isArrayBufferDetached(V.buffer)) {
        throw makeException(
          TypeError,
          "is a view on a detached ArrayBuffer",
          opts,
        );
      }
      return V;
    }

    if (!opts.allowShared && !isNonSharedArrayBuffer(V)) {
      throw makeException(
        TypeError,
        "is not an ArrayBuffer or a view on one",
        opts,
      );
    }
    if (
      opts.allowShared &&
      !isSharedArrayBuffer(V) &&
      !isNonSharedArrayBuffer(V)
    ) {
      throw makeException(
        TypeError,
        "is not an ArrayBuffer, SharedArrayBuffer, or a view on one",
        opts,
      );
    }
    if (isArrayBufferDetached(V)) {
      throw makeException(TypeError, "is a detached ArrayBuffer", opts);
    }

    return V;
  };

  converters.DOMTimeStamp = converters["unsigned long long"];

  converters.Function = convertCallbackFunction;

  converters.VoidFunction = convertCallbackFunction;

  function requiredArguments(length, required, opts = {}) {
    if (length < required) {
      const errMsg = `${
        opts.prefix ? opts.prefix + ": " : ""
      }${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present.`;
      throw new TypeError(errMsg);
    }
  }

  function createDictionaryConverter(name, ...dictionaries) {
    return function (V, opts = {}) {
      const typeV = type(V);
      switch (typeV) {
        case "Undefined":
        case "Null":
        case "Object":
          break;
        default:
          throw makeException(
            TypeError,
            "can not be converted to a dictionary",
            opts,
          );
      }
      const esDict = V;

      const idlDict = {};

      for (const members of dictionaries) {
        for (const member of members) {
          const key = member.key;

          let esMemberValue;
          if (typeV === "Undefined" || typeV === "Null") {
            esMemberValue = undefined;
          } else {
            esMemberValue = esDict[key];
          }

          if (esMemberValue !== undefined) {
            const converter = member.converter;
            const idlMemberValue = converter(esMemberValue, {
              ...opts,
              context: `${key} of '${name}'${
                opts.context ? `(${opts.context})` : ""
              }`,
            });
            idlDict[key] = idlMemberValue;
          } else if ("defaultValue" in member) {
            const defaultValue = member.defaultValue;
            const idlMemberValue = defaultValue;
            idlDict[key] = idlMemberValue;
          } else if (member.required) {
            throw new TypeError(
              `can not be converted to '${name}' because ${key} is required in '${name}'.`,
            );
          }
        }
      }

      return idlDict;
    };
  }

  function createEnumeration(name, ...values) {
    const E = new Set(values);

    return function (V, opts = {}) {
      const S = String(V);

      if (false === E.has(V)) {
        throw makeException(
          TypeError,
          `The provided value '${V}' is not a valid enum value of type ${name}.`,
          opts,
        );
      } else {
        return V;
      }
    };
  }

  window.__bootstrap ??= {};
  window.__bootstrap.webidl = {
    converters,
    requiredArguments,
    createDictionaryConverter,
    createEnumeration
  };
})(this);
