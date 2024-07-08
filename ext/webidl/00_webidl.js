// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Adapted from https://github.com/jsdom/webidl-conversions.
// Copyright Domenic Denicola. Licensed under BSD-2-Clause License.
// Original license at https://github.com/jsdom/webidl-conversions/blob/master/LICENSE.md.

import { core, primordials } from "ext:core/mod.js";
const {
  isArrayBuffer,
  isDataView,
  isSharedArrayBuffer,
  isTypedArray,
} = core;
const {
  ArrayBufferIsView,
  ArrayPrototypeForEach,
  ArrayPrototypePush,
  ArrayPrototypeSort,
  ArrayIteratorPrototype,
  BigInt,
  BigIntAsIntN,
  BigIntAsUintN,
  DataViewPrototypeGetBuffer,
  Float32Array,
  Float64Array,
  FunctionPrototypeBind,
  Int16Array,
  Int32Array,
  Int8Array,
  MathFloor,
  MathFround,
  MathMax,
  MathMin,
  MathPow,
  MathRound,
  MathTrunc,
  Number,
  NumberIsFinite,
  NumberIsNaN,
  NumberMAX_SAFE_INTEGER,
  NumberMIN_SAFE_INTEGER,
  ObjectAssign,
  ObjectCreate,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetOwnPropertyDescriptors,
  ObjectGetPrototypeOf,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  ObjectIs,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  ReflectApply,
  ReflectDefineProperty,
  ReflectGetOwnPropertyDescriptor,
  ReflectHas,
  ReflectOwnKeys,
  RegExpPrototypeTest,
  SafeRegExp,
  SafeSet,
  SetPrototypeEntries,
  SetPrototypeForEach,
  SetPrototypeKeys,
  SetPrototypeValues,
  SetPrototypeHas,
  SetPrototypeClear,
  SetPrototypeDelete,
  SetPrototypeAdd,
  // TODO(lucacasonato): add SharedArrayBuffer to primordials
  // SharedArrayBufferPrototype,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeToWellFormed,
  Symbol,
  SymbolIterator,
  SymbolToStringTag,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetSymbolToStringTag,
  TypeError,
  Uint16Array,
  Uint32Array,
  Uint8Array,
  Uint8ClampedArray,
} = primordials;

function makeException(ErrorType, message, prefix, context) {
  return new ErrorType(
    `${prefix ? prefix + ": " : ""}${context ? context : "Value"} ${message}`,
  );
}

function toNumber(value) {
  if (typeof value === "bigint") {
    throw TypeError("Cannot convert a BigInt value to a number");
  }
  return Number(value);
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
      // Per ES spec, typeof returns an implementation-defined value that is not any of the existing ones for
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
    return censorNegativeZero(MathFloor(x));
  }

  return censorNegativeZero(MathRound(x));
}

function integerPart(n) {
  return censorNegativeZero(MathTrunc(n));
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
    upperBound = NumberMAX_SAFE_INTEGER;
    lowerBound = !isSigned ? 0 : NumberMIN_SAFE_INTEGER;
  } else if (!isSigned) {
    lowerBound = 0;
    upperBound = MathPow(2, bitLength) - 1;
  } else {
    lowerBound = -MathPow(2, bitLength - 1);
    upperBound = MathPow(2, bitLength - 1) - 1;
  }

  const twoToTheBitLength = MathPow(2, bitLength);
  const twoToOneLessThanTheBitLength = MathPow(2, bitLength - 1);

  return (
    V,
    prefix = undefined,
    context = undefined,
    opts = { __proto__: null },
  ) => {
    let x = toNumber(V);
    x = censorNegativeZero(x);

    if (opts.enforceRange) {
      if (!NumberIsFinite(x)) {
        throw makeException(
          TypeError,
          "is not a finite number",
          prefix,
          context,
        );
      }

      x = integerPart(x);

      if (x < lowerBound || x > upperBound) {
        throw makeException(
          TypeError,
          `is outside the accepted range of ${lowerBound} to ${upperBound}, inclusive`,
          prefix,
          context,
        );
      }

      return x;
    }

    if (!NumberIsNaN(x) && opts.clamp) {
      x = MathMin(MathMax(x, lowerBound), upperBound);
      x = evenRound(x);
      return x;
    }

    if (!NumberIsFinite(x) || x === 0) {
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
  const upperBound = NumberMAX_SAFE_INTEGER;
  const lowerBound = unsigned ? 0 : NumberMIN_SAFE_INTEGER;
  const asBigIntN = unsigned ? BigIntAsUintN : BigIntAsIntN;

  return (
    V,
    prefix = undefined,
    context = undefined,
    opts = { __proto__: null },
  ) => {
    let x = toNumber(V);
    x = censorNegativeZero(x);

    if (opts.enforceRange) {
      if (!NumberIsFinite(x)) {
        throw makeException(
          TypeError,
          "is not a finite number",
          prefix,
          context,
        );
      }

      x = integerPart(x);

      if (x < lowerBound || x > upperBound) {
        throw makeException(
          TypeError,
          `is outside the accepted range of ${lowerBound} to ${upperBound}, inclusive`,
          prefix,
          context,
        );
      }

      return x;
    }

    if (!NumberIsNaN(x) && opts.clamp) {
      x = MathMin(MathMax(x, lowerBound), upperBound);
      x = evenRound(x);
      return x;
    }

    if (!NumberIsFinite(x) || x === 0) {
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

converters.float = (V, prefix, context, _opts) => {
  const x = toNumber(V);

  if (!NumberIsFinite(x)) {
    throw makeException(
      TypeError,
      "is not a finite floating-point value",
      prefix,
      context,
    );
  }

  if (ObjectIs(x, -0)) {
    return x;
  }

  const y = MathFround(x);

  if (!NumberIsFinite(y)) {
    throw makeException(
      TypeError,
      "is outside the range of a single-precision floating-point value",
      prefix,
      context,
    );
  }

  return y;
};

converters["unrestricted float"] = (V, _prefix, _context, _opts) => {
  const x = toNumber(V);

  if (NumberIsNaN(x)) {
    return x;
  }

  if (ObjectIs(x, -0)) {
    return x;
  }

  return MathFround(x);
};

converters.double = (V, prefix, context, _opts) => {
  const x = toNumber(V);

  if (!NumberIsFinite(x)) {
    throw makeException(
      TypeError,
      "is not a finite floating-point value",
      prefix,
      context,
    );
  }

  return x;
};

converters["unrestricted double"] = (V, _prefix, _context, _opts) => {
  const x = toNumber(V);

  return x;
};

converters.DOMString = function (
  V,
  prefix,
  context,
  opts = { __proto__: null },
) {
  if (typeof V === "string") {
    return V;
  } else if (V === null && opts.treatNullAsEmptyString) {
    return "";
  } else if (typeof V === "symbol") {
    throw makeException(
      TypeError,
      "is a symbol, which cannot be converted to a string",
      prefix,
      context,
    );
  }

  return String(V);
};

function isByteString(input) {
  for (let i = 0; i < input.length; i++) {
    if (StringPrototypeCharCodeAt(input, i) > 255) {
      // If a character code is greater than 255, it means the string is not a byte string.
      return false;
    }
  }
  return true;
}

converters.ByteString = (V, prefix, context, opts) => {
  const x = converters.DOMString(V, prefix, context, opts);
  if (!isByteString(x)) {
    throw makeException(
      TypeError,
      "is not a valid ByteString",
      prefix,
      context,
    );
  }
  return x;
};

converters.USVString = (V, prefix, context, opts) => {
  const S = converters.DOMString(V, prefix, context, opts);
  return StringPrototypeToWellFormed(S);
};

converters.object = (V, prefix, context, _opts) => {
  if (type(V) !== "Object") {
    throw makeException(
      TypeError,
      "is not an object",
      prefix,
      context,
    );
  }

  return V;
};

// Not exported, but used in Function and VoidFunction.

// Neither Function nor VoidFunction is defined with [TreatNonObjectAsNull], so
// handling for that is omitted.
function convertCallbackFunction(V, prefix, context, _opts) {
  if (typeof V !== "function") {
    throw makeException(
      TypeError,
      "is not a function",
      prefix,
      context,
    );
  }
  return V;
}

converters.ArrayBuffer = (
  V,
  prefix = undefined,
  context = undefined,
  opts = { __proto__: null },
) => {
  if (!isArrayBuffer(V)) {
    if (opts.allowShared && !isSharedArrayBuffer(V)) {
      throw makeException(
        TypeError,
        "is not an ArrayBuffer or SharedArrayBuffer",
        prefix,
        context,
      );
    }
    throw makeException(
      TypeError,
      "is not an ArrayBuffer",
      prefix,
      context,
    );
  }

  return V;
};

converters.DataView = (
  V,
  prefix = undefined,
  context = undefined,
  opts = { __proto__: null },
) => {
  if (!isDataView(V)) {
    throw makeException(
      TypeError,
      "is not a DataView",
      prefix,
      context,
    );
  }

  if (
    !opts.allowShared &&
    isSharedArrayBuffer(DataViewPrototypeGetBuffer(V))
  ) {
    throw makeException(
      TypeError,
      "is backed by a SharedArrayBuffer, which is not allowed",
      prefix,
      context,
    );
  }

  return V;
};

ArrayPrototypeForEach(
  [
    Int8Array,
    Int16Array,
    Int32Array,
    Uint8Array,
    Uint16Array,
    Uint32Array,
    Uint8ClampedArray,
    // TODO(petamoriken): add Float16Array converter
    // Float16Array,
    Float32Array,
    Float64Array,
  ],
  (func) => {
    const name = func.name;
    const article = RegExpPrototypeTest(new SafeRegExp(/^[AEIOU]/), name)
      ? "an"
      : "a";
    converters[name] = (
      V,
      prefix = undefined,
      context = undefined,
      opts = { __proto__: null },
    ) => {
      if (TypedArrayPrototypeGetSymbolToStringTag(V) !== name) {
        throw makeException(
          TypeError,
          `is not ${article} ${name} object`,
          prefix,
          context,
        );
      }
      if (
        !opts.allowShared &&
        isSharedArrayBuffer(TypedArrayPrototypeGetBuffer(V))
      ) {
        throw makeException(
          TypeError,
          "is a view on a SharedArrayBuffer, which is not allowed",
          prefix,
          context,
        );
      }

      return V;
    };
  },
);

// Common definitions

converters.ArrayBufferView = (
  V,
  prefix = undefined,
  context = undefined,
  opts = { __proto__: null },
) => {
  if (!ArrayBufferIsView(V)) {
    throw makeException(
      TypeError,
      "is not a view on an ArrayBuffer or SharedArrayBuffer",
      prefix,
      context,
    );
  }
  let buffer;
  if (isTypedArray(V)) {
    buffer = TypedArrayPrototypeGetBuffer(V);
  } else {
    buffer = DataViewPrototypeGetBuffer(V);
  }
  if (!opts.allowShared && isSharedArrayBuffer(buffer)) {
    throw makeException(
      TypeError,
      "is a view on a SharedArrayBuffer, which is not allowed",
      prefix,
      context,
    );
  }

  return V;
};

converters.BufferSource = (
  V,
  prefix = undefined,
  context = undefined,
  opts = { __proto__: null },
) => {
  if (ArrayBufferIsView(V)) {
    let buffer;
    if (isTypedArray(V)) {
      buffer = TypedArrayPrototypeGetBuffer(V);
    } else {
      buffer = DataViewPrototypeGetBuffer(V);
    }
    if (!opts.allowShared && isSharedArrayBuffer(buffer)) {
      throw makeException(
        TypeError,
        "is a view on a SharedArrayBuffer, which is not allowed",
        prefix,
        context,
      );
    }

    return V;
  }

  if (!opts.allowShared && !isArrayBuffer(V)) {
    throw makeException(
      TypeError,
      "is not an ArrayBuffer or a view on one",
      prefix,
      context,
    );
  }
  if (
    opts.allowShared &&
    !isSharedArrayBuffer(V) &&
    !isArrayBuffer(V)
  ) {
    throw makeException(
      TypeError,
      "is not an ArrayBuffer, SharedArrayBuffer, or a view on one",
      prefix,
      context,
    );
  }

  return V;
};

converters.DOMTimeStamp = converters["unsigned long long"];
converters.DOMHighResTimeStamp = converters["double"];

converters.Function = convertCallbackFunction;

converters.VoidFunction = convertCallbackFunction;

converters["UVString?"] = createNullableConverter(
  converters.USVString,
);
converters["sequence<double>"] = createSequenceConverter(
  converters.double,
);
converters["sequence<object>"] = createSequenceConverter(
  converters.object,
);
converters["Promise<undefined>"] = createPromiseConverter(() => undefined);

converters["sequence<ByteString>"] = createSequenceConverter(
  converters.ByteString,
);
converters["sequence<sequence<ByteString>>"] = createSequenceConverter(
  converters["sequence<ByteString>"],
);
converters["record<ByteString, ByteString>"] = createRecordConverter(
  converters.ByteString,
  converters.ByteString,
);

converters["sequence<USVString>"] = createSequenceConverter(
  converters.USVString,
);
converters["sequence<sequence<USVString>>"] = createSequenceConverter(
  converters["sequence<USVString>"],
);
converters["record<USVString, USVString>"] = createRecordConverter(
  converters.USVString,
  converters.USVString,
);

converters["sequence<DOMString>"] = createSequenceConverter(
  converters.DOMString,
);

function requiredArguments(length, required, prefix) {
  if (length < required) {
    const errMsg = `${prefix ? prefix + ": " : ""}${required} argument${
      required === 1 ? "" : "s"
    } required, but only ${length} present.`;
    throw new TypeError(errMsg);
  }
}

function createDictionaryConverter(name, ...dictionaries) {
  let hasRequiredKey = false;
  const allMembers = [];
  for (let i = 0; i < dictionaries.length; ++i) {
    const members = dictionaries[i];
    for (let j = 0; j < members.length; ++j) {
      const member = members[j];
      if (member.required) {
        hasRequiredKey = true;
      }
      ArrayPrototypePush(allMembers, member);
    }
  }
  ArrayPrototypeSort(allMembers, (a, b) => {
    if (a.key == b.key) {
      return 0;
    }
    return a.key < b.key ? -1 : 1;
  });

  const defaultValues = { __proto__: null };
  for (let i = 0; i < allMembers.length; ++i) {
    const member = allMembers[i];
    if (ReflectHas(member, "defaultValue")) {
      const idlMemberValue = member.defaultValue;
      const imvType = typeof idlMemberValue;
      // Copy by value types can be directly assigned, copy by reference types
      // need to be re-created for each allocation.
      if (
        imvType === "number" || imvType === "boolean" ||
        imvType === "string" || imvType === "bigint" ||
        imvType === "undefined"
      ) {
        defaultValues[member.key] = member.converter(idlMemberValue, {});
      } else {
        ObjectDefineProperty(defaultValues, member.key, {
          get() {
            return member.converter(idlMemberValue, member.defaultValue);
          },
          enumerable: true,
        });
      }
    }
  }

  return function (
    V,
    prefix = undefined,
    context = undefined,
    opts = { __proto__: null },
  ) {
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
          prefix,
          context,
        );
    }
    const esDict = V;

    const idlDict = ObjectAssign({}, defaultValues);

    // NOTE: fast path Null and Undefined.
    if ((V === undefined || V === null) && !hasRequiredKey) {
      return idlDict;
    }

    for (let i = 0; i < allMembers.length; ++i) {
      const member = allMembers[i];
      const key = member.key;

      let esMemberValue;
      if (typeV === "Undefined" || typeV === "Null") {
        esMemberValue = undefined;
      } else {
        esMemberValue = esDict[key];
      }

      if (esMemberValue !== undefined) {
        const memberContext = `'${key}' of '${name}'${
          context ? ` (${context})` : ""
        }`;
        const converter = member.converter;
        const idlMemberValue = converter(
          esMemberValue,
          prefix,
          memberContext,
          opts,
        );
        idlDict[key] = idlMemberValue;
      } else if (member.required) {
        throw makeException(
          TypeError,
          `can not be converted to '${name}' because '${key}' is required in '${name}'.`,
          prefix,
          context,
        );
      }
    }

    return idlDict;
  };
}

// https://heycam.github.io/webidl/#es-enumeration
function createEnumConverter(name, values) {
  const E = new SafeSet(values);

  return function (
    V,
    prefix = undefined,
    _context = undefined,
    _opts = { __proto__: null },
  ) {
    const S = String(V);

    if (!E.has(S)) {
      throw new TypeError(
        `${
          prefix ? prefix + ": " : ""
        }The provided value '${S}' is not a valid enum value of type ${name}.`,
      );
    }

    return S;
  };
}

function createNullableConverter(converter) {
  return (
    V,
    prefix = undefined,
    context = undefined,
    opts = { __proto__: null },
  ) => {
    // FIXME: If Type(V) is not Object, and the conversion to an IDL value is
    // being performed due to V being assigned to an attribute whose type is a
    // nullable callback function that is annotated with
    // [LegacyTreatNonObjectAsNull], then return the IDL nullable type T?
    // value null.

    if (V === null || V === undefined) return null;
    return converter(V, prefix, context, opts);
  };
}

// https://heycam.github.io/webidl/#es-sequence
function createSequenceConverter(converter) {
  return function (
    V,
    prefix = undefined,
    context = undefined,
    opts = { __proto__: null },
  ) {
    if (type(V) !== "Object") {
      throw makeException(
        TypeError,
        "can not be converted to sequence.",
        prefix,
        context,
      );
    }
    const iter = V?.[SymbolIterator]?.();
    if (iter === undefined) {
      throw makeException(
        TypeError,
        "can not be converted to sequence.",
        prefix,
        context,
      );
    }
    const array = [];
    while (true) {
      const res = iter?.next?.();
      if (res === undefined) {
        throw makeException(
          TypeError,
          "can not be converted to sequence.",
          prefix,
          context,
        );
      }
      if (res.done === true) break;
      const val = converter(
        res.value,
        prefix,
        `${context}, index ${array.length}`,
        opts,
      );
      ArrayPrototypePush(array, val);
    }
    return array;
  };
}

function createRecordConverter(keyConverter, valueConverter) {
  return (V, prefix, context, opts) => {
    if (type(V) !== "Object") {
      throw makeException(
        TypeError,
        "can not be converted to dictionary.",
        prefix,
        context,
      );
    }
    const result = { __proto__: null };
    // Fast path for common case (not a Proxy)
    if (!core.isProxy(V)) {
      for (const key in V) {
        if (!ObjectHasOwn(V, key)) {
          continue;
        }
        const typedKey = keyConverter(key, prefix, context, opts);
        const value = V[key];
        const typedValue = valueConverter(value, prefix, context, opts);
        result[typedKey] = typedValue;
      }
      return result;
    }
    // Slow path if Proxy (e.g: in WPT tests)
    const keys = ReflectOwnKeys(V);
    for (let i = 0; i < keys.length; ++i) {
      const key = keys[i];
      const desc = ObjectGetOwnPropertyDescriptor(V, key);
      if (desc !== undefined && desc.enumerable === true) {
        const typedKey = keyConverter(key, prefix, context, opts);
        const value = V[key];
        const typedValue = valueConverter(value, prefix, context, opts);
        result[typedKey] = typedValue;
      }
    }
    return result;
  };
}

function createPromiseConverter(converter) {
  return (V, prefix, context, opts) =>
    // should be able to handle thenables
    // see: https://github.com/web-platform-tests/wpt/blob/a31d3ba53a79412793642366f3816c9a63f0cf57/streams/writable-streams/close.any.js#L207
    typeof V?.then === "function"
      ? PromisePrototypeThen(PromiseResolve(V), (V) =>
        converter(V, prefix, context, opts))
      : PromiseResolve(converter(V, prefix, context, opts));
}

function invokeCallbackFunction(
  callable,
  args,
  thisArg,
  returnValueConverter,
  prefix,
  returnsPromise,
) {
  try {
    const rv = ReflectApply(callable, thisArg, args);
    return returnValueConverter(rv, prefix, "return value");
  } catch (err) {
    if (returnsPromise === true) {
      return PromiseReject(err);
    }
    throw err;
  }
}

const brand = Symbol("[[webidl.brand]]");

function createInterfaceConverter(name, prototype) {
  return (V, prefix, context, _opts) => {
    if (!ObjectPrototypeIsPrototypeOf(prototype, V) || V[brand] !== brand) {
      throw makeException(
        TypeError,
        `is not of type ${name}.`,
        prefix,
        context,
      );
    }
    return V;
  };
}

// TODO(lucacasonato): have the user pass in the prototype, and not the type.
function createBranded(Type) {
  const t = ObjectCreate(Type.prototype);
  t[brand] = brand;
  return t;
}

function assertBranded(self, prototype) {
  if (
    !ObjectPrototypeIsPrototypeOf(prototype, self) || self[brand] !== brand
  ) {
    throw new TypeError("Illegal invocation");
  }
}

function illegalConstructor() {
  throw new TypeError("Illegal constructor");
}

function define(target, source) {
  const keys = ReflectOwnKeys(source);
  for (let i = 0; i < keys.length; ++i) {
    const key = keys[i];
    const descriptor = ReflectGetOwnPropertyDescriptor(source, key);
    if (descriptor && !ReflectDefineProperty(target, key, descriptor)) {
      throw new TypeError(`Cannot redefine property: ${String(key)}`);
    }
  }
}

const _iteratorInternal = Symbol("iterator internal");

const globalIteratorPrototype = ObjectGetPrototypeOf(ArrayIteratorPrototype);

function mixinPairIterable(name, prototype, dataSymbol, keyKey, valueKey) {
  const iteratorPrototype = ObjectCreate(globalIteratorPrototype, {
    [SymbolToStringTag]: { configurable: true, value: `${name} Iterator` },
  });
  define(iteratorPrototype, {
    next() {
      const internal = this && this[_iteratorInternal];
      if (!internal) {
        throw new TypeError(
          `next() called on a value that is not a ${name} iterator object`,
        );
      }
      const { target, kind, index } = internal;
      const values = target[dataSymbol];
      const len = values.length;
      if (index >= len) {
        return { value: undefined, done: true };
      }
      const pair = values[index];
      internal.index = index + 1;
      let result;
      switch (kind) {
        case "key":
          result = pair[keyKey];
          break;
        case "value":
          result = pair[valueKey];
          break;
        case "key+value":
          result = [pair[keyKey], pair[valueKey]];
          break;
      }
      return { value: result, done: false };
    },
  });
  function createDefaultIterator(target, kind) {
    const iterator = ObjectCreate(iteratorPrototype);
    ObjectDefineProperty(iterator, _iteratorInternal, {
      value: { target, kind, index: 0 },
      configurable: true,
    });
    return iterator;
  }

  function entries() {
    assertBranded(this, prototype.prototype);
    return createDefaultIterator(this, "key+value");
  }

  const properties = {
    entries: {
      value: entries,
      writable: true,
      enumerable: true,
      configurable: true,
    },
    [SymbolIterator]: {
      value: entries,
      writable: true,
      enumerable: false,
      configurable: true,
    },
    keys: {
      value: function keys() {
        assertBranded(this, prototype.prototype);
        return createDefaultIterator(this, "key");
      },
      writable: true,
      enumerable: true,
      configurable: true,
    },
    values: {
      value: function values() {
        assertBranded(this, prototype.prototype);
        return createDefaultIterator(this, "value");
      },
      writable: true,
      enumerable: true,
      configurable: true,
    },
    forEach: {
      value: function forEach(idlCallback, thisArg = undefined) {
        assertBranded(this, prototype.prototype);
        const prefix = `Failed to execute 'forEach' on '${name}'`;
        requiredArguments(arguments.length, 1, { prefix });
        idlCallback = converters["Function"](idlCallback, {
          prefix,
          context: "Argument 1",
        });
        idlCallback = FunctionPrototypeBind(
          idlCallback,
          thisArg ?? globalThis,
        );
        const pairs = this[dataSymbol];
        for (let i = 0; i < pairs.length; i++) {
          const entry = pairs[i];
          idlCallback(entry[valueKey], entry[keyKey], this);
        }
      },
      writable: true,
      enumerable: true,
      configurable: true,
    },
  };
  return ObjectDefineProperties(prototype.prototype, properties);
}

function configureInterface(interface_) {
  configureProperties(interface_);
  configureProperties(interface_.prototype);
  ObjectDefineProperty(interface_.prototype, SymbolToStringTag, {
    value: interface_.name,
    enumerable: false,
    configurable: true,
    writable: false,
  });
}

function configureProperties(obj) {
  const descriptors = ObjectGetOwnPropertyDescriptors(obj);
  for (const key in descriptors) {
    if (!ObjectHasOwn(descriptors, key)) {
      continue;
    }
    if (key === "constructor") continue;
    if (key === "prototype") continue;
    const descriptor = descriptors[key];
    if (
      ReflectHas(descriptor, "value") &&
      typeof descriptor.value === "function"
    ) {
      ObjectDefineProperty(obj, key, {
        enumerable: true,
        writable: true,
        configurable: true,
      });
    } else if (ReflectHas(descriptor, "get")) {
      ObjectDefineProperty(obj, key, {
        enumerable: true,
        configurable: true,
      });
    }
  }
}

const setlikeInner = Symbol("[[set]]");

// Ref: https://webidl.spec.whatwg.org/#es-setlike
function setlike(obj, objPrototype, readonly) {
  ObjectDefineProperties(obj, {
    size: {
      configurable: true,
      enumerable: true,
      get() {
        assertBranded(this, objPrototype);
        return obj[setlikeInner].size;
      },
    },
    [SymbolIterator]: {
      configurable: true,
      enumerable: false,
      writable: true,
      value() {
        assertBranded(this, objPrototype);
        return obj[setlikeInner][SymbolIterator]();
      },
    },
    entries: {
      configurable: true,
      enumerable: true,
      writable: true,
      value() {
        assertBranded(this, objPrototype);
        return SetPrototypeEntries(obj[setlikeInner]);
      },
    },
    keys: {
      configurable: true,
      enumerable: true,
      writable: true,
      value() {
        assertBranded(this, objPrototype);
        return SetPrototypeKeys(obj[setlikeInner]);
      },
    },
    values: {
      configurable: true,
      enumerable: true,
      writable: true,
      value() {
        assertBranded(this, objPrototype);
        return SetPrototypeValues(obj[setlikeInner]);
      },
    },
    forEach: {
      configurable: true,
      enumerable: true,
      writable: true,
      value(callbackfn, thisArg) {
        assertBranded(this, objPrototype);
        return SetPrototypeForEach(obj[setlikeInner], callbackfn, thisArg);
      },
    },
    has: {
      configurable: true,
      enumerable: true,
      writable: true,
      value(value) {
        assertBranded(this, objPrototype);
        return SetPrototypeHas(obj[setlikeInner], value);
      },
    },
  });

  if (!readonly) {
    ObjectDefineProperties(obj, {
      add: {
        configurable: true,
        enumerable: true,
        writable: true,
        value(value) {
          assertBranded(this, objPrototype);
          return SetPrototypeAdd(obj[setlikeInner], value);
        },
      },
      delete: {
        configurable: true,
        enumerable: true,
        writable: true,
        value(value) {
          assertBranded(this, objPrototype);
          return SetPrototypeDelete(obj[setlikeInner], value);
        },
      },
      clear: {
        configurable: true,
        enumerable: true,
        writable: true,
        value() {
          assertBranded(this, objPrototype);
          return SetPrototypeClear(obj[setlikeInner]);
        },
      },
    });
  }
}

export {
  assertBranded,
  brand,
  configureInterface,
  converters,
  createBranded,
  createDictionaryConverter,
  createEnumConverter,
  createInterfaceConverter,
  createNullableConverter,
  createPromiseConverter,
  createRecordConverter,
  createSequenceConverter,
  illegalConstructor,
  invokeCallbackFunction,
  makeException,
  mixinPairIterable,
  requiredArguments,
  setlike,
  setlikeInner,
  type,
};
