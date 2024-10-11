// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { core, internals, primordials } from "ext:core/mod.js";
const {
  isAnyArrayBuffer,
  isArgumentsObject,
  isArrayBuffer,
  isAsyncFunction,
  isBigIntObject,
  isBooleanObject,
  isBoxedPrimitive,
  isDataView,
  isDate,
  isGeneratorFunction,
  isMap,
  isMapIterator,
  isModuleNamespaceObject,
  isNativeError,
  isNumberObject,
  isPromise,
  isRegExp,
  isSet,
  isSetIterator,
  isStringObject,
  isTypedArray,
  isWeakMap,
  isWeakSet,
} = core;
import {
  op_get_constructor_name,
  op_get_non_index_property_names,
  op_preview_entries,
} from "ext:core/ops";
import * as ops from "ext:core/ops";
const {
  Array,
  ArrayBufferPrototypeGetByteLength,
  ArrayIsArray,
  ArrayPrototypeFill,
  ArrayPrototypeFilter,
  ArrayPrototypeFind,
  ArrayPrototypeForEach,
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePop,
  ArrayPrototypePush,
  ArrayPrototypePushApply,
  ArrayPrototypeReduce,
  ArrayPrototypeShift,
  ArrayPrototypeSlice,
  ArrayPrototypeSort,
  ArrayPrototypeSplice,
  ArrayPrototypeUnshift,
  BigIntPrototypeValueOf,
  Boolean,
  BooleanPrototypeValueOf,
  DateNow,
  DatePrototypeGetTime,
  DatePrototypeToISOString,
  Error,
  ErrorCaptureStackTrace,
  ErrorPrototype,
  ErrorPrototypeToString,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  FunctionPrototypeToString,
  MapPrototypeDelete,
  MapPrototypeEntries,
  MapPrototypeForEach,
  MapPrototypeGet,
  MapPrototypeGetSize,
  MapPrototypeHas,
  MapPrototypeSet,
  MathAbs,
  MathFloor,
  MathMax,
  MathMin,
  MathRound,
  MathSqrt,
  Number,
  NumberIsInteger,
  NumberIsNaN,
  NumberParseInt,
  NumberPrototypeToFixed,
  NumberPrototypeToString,
  NumberPrototypeValueOf,
  ObjectAssign,
  ObjectCreate,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectFromEntries,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetOwnPropertyNames,
  ObjectGetOwnPropertySymbols,
  ObjectGetPrototypeOf,
  ObjectHasOwn,
  ObjectIs,
  ObjectKeys,
  ObjectPrototype,
  ObjectPrototypeIsPrototypeOf,
  ObjectPrototypePropertyIsEnumerable,
  ObjectSetPrototypeOf,
  ObjectValues,
  Proxy,
  ReflectGet,
  ReflectGetOwnPropertyDescriptor,
  ReflectGetPrototypeOf,
  ReflectHas,
  ReflectOwnKeys,
  RegExpPrototypeExec,
  RegExpPrototypeSymbolReplace,
  RegExpPrototypeTest,
  RegExpPrototypeToString,
  SafeArrayIterator,
  SafeMap,
  SafeMapIterator,
  SafeRegExp,
  SafeSet,
  SafeSetIterator,
  SafeStringIterator,
  SetPrototypeAdd,
  SetPrototypeGetSize,
  SetPrototypeHas,
  SetPrototypeValues,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeCodePointAt,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeLastIndexOf,
  StringPrototypeMatch,
  StringPrototypeNormalize,
  StringPrototypePadEnd,
  StringPrototypePadStart,
  StringPrototypeRepeat,
  StringPrototypeReplace,
  StringPrototypeReplaceAll,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  StringPrototypeTrim,
  StringPrototypeValueOf,
  Symbol,
  SymbolFor,
  SymbolHasInstance,
  SymbolIterator,
  SymbolPrototypeGetDescription,
  SymbolPrototypeToString,
  SymbolPrototypeValueOf,
  SymbolToStringTag,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetLength,
  Uint8Array,
  Uint32Array,
} = primordials;

let currentTime = DateNow;
if (ops.op_now) {
  const hrU8 = new Uint8Array(8);
  const hr = new Uint32Array(TypedArrayPrototypeGetBuffer(hrU8));
  currentTime = function opNow() {
    ops.op_now(hrU8);
    return (hr[0] * 1000 + hr[1] / 1e6);
  };
}

let noColorStdout = () => false;
let noColorStderr = () => false;

function setNoColorFns(stdoutFn, stderrFn) {
  noColorStdout = stdoutFn;
  noColorStderr = stderrFn;
}

function getStdoutNoColor() {
  return noColorStdout();
}

function getStderrNoColor() {
  return noColorStderr();
}

class AssertionError extends Error {
  name = "AssertionError";
  constructor(message) {
    super(message);
  }
}

function assert(cond, msg = "Assertion failed") {
  if (!cond) {
    throw new AssertionError(msg);
  }
}

// Don't use 'blue' not visible on cmd.exe
const styles = {
  special: "cyan",
  number: "yellow",
  bigint: "yellow",
  boolean: "yellow",
  undefined: "grey",
  null: "bold",
  string: "green",
  symbol: "green",
  date: "magenta",
  // "name": intentionally not styling
  // TODO(BridgeAR): Highlight regular expressions properly.
  regexp: "red",
  module: "underline",
  internalError: "red",
  temporal: "magenta",
};

const defaultFG = 39;
const defaultBG = 49;

// Set Graphics Rendition https://en.wikipedia.org/wiki/ANSI_escape_code#graphics
// Each color consists of an array with the color code as first entry and the
// reset code as second entry.
const colors = {
  reset: [0, 0],
  bold: [1, 22],
  dim: [2, 22], // Alias: faint
  italic: [3, 23],
  underline: [4, 24],
  blink: [5, 25],
  // Swap foreground and background colors
  inverse: [7, 27], // Alias: swapcolors, swapColors
  hidden: [8, 28], // Alias: conceal
  strikethrough: [9, 29], // Alias: strikeThrough, crossedout, crossedOut
  doubleunderline: [21, 24], // Alias: doubleUnderline
  black: [30, defaultFG],
  red: [31, defaultFG],
  green: [32, defaultFG],
  yellow: [33, defaultFG],
  blue: [34, defaultFG],
  magenta: [35, defaultFG],
  cyan: [36, defaultFG],
  white: [37, defaultFG],
  bgBlack: [40, defaultBG],
  bgRed: [41, defaultBG],
  bgGreen: [42, defaultBG],
  bgYellow: [43, defaultBG],
  bgBlue: [44, defaultBG],
  bgMagenta: [45, defaultBG],
  bgCyan: [46, defaultBG],
  bgWhite: [47, defaultBG],
  framed: [51, 54],
  overlined: [53, 55],
  gray: [90, defaultFG], // Alias: grey, blackBright
  redBright: [91, defaultFG],
  greenBright: [92, defaultFG],
  yellowBright: [93, defaultFG],
  blueBright: [94, defaultFG],
  magentaBright: [95, defaultFG],
  cyanBright: [96, defaultFG],
  whiteBright: [97, defaultFG],
  bgGray: [100, defaultBG], // Alias: bgGrey, bgBlackBright
  bgRedBright: [101, defaultBG],
  bgGreenBright: [102, defaultBG],
  bgYellowBright: [103, defaultBG],
  bgBlueBright: [104, defaultBG],
  bgMagentaBright: [105, defaultBG],
  bgCyanBright: [106, defaultBG],
  bgWhiteBright: [107, defaultBG],
};

function defineColorAlias(target, alias) {
  ObjectDefineProperty(colors, alias, {
    __proto__: null,
    get() {
      return this[target];
    },
    set(value) {
      this[target] = value;
    },
    configurable: true,
    enumerable: false,
  });
}

defineColorAlias("gray", "grey");
defineColorAlias("gray", "blackBright");
defineColorAlias("bgGray", "bgGrey");
defineColorAlias("bgGray", "bgBlackBright");
defineColorAlias("dim", "faint");
defineColorAlias("strikethrough", "crossedout");
defineColorAlias("strikethrough", "strikeThrough");
defineColorAlias("strikethrough", "crossedOut");
defineColorAlias("hidden", "conceal");
defineColorAlias("inverse", "swapColors");
defineColorAlias("inverse", "swapcolors");
defineColorAlias("doubleunderline", "doubleUnderline");

// https://tc39.es/ecma262/#sec-get-sharedarraybuffer.prototype.bytelength
let _getSharedArrayBufferByteLength;

function getSharedArrayBufferByteLength(value) {
  // TODO(kt3k): add SharedArrayBuffer to primordials
  _getSharedArrayBufferByteLength ??= ObjectGetOwnPropertyDescriptor(
    // deno-lint-ignore prefer-primordials
    SharedArrayBuffer.prototype,
    "byteLength",
  ).get;

  return FunctionPrototypeCall(_getSharedArrayBufferByteLength, value);
}

// The name property is used to allow cross realms to make a determination
// This is the same as WHATWG's structuredClone algorithm
// https://github.com/whatwg/html/pull/5150
function isAggregateError(value) {
  return (
    isNativeError(value) &&
    value.name === "AggregateError" &&
    ArrayIsArray(value.errors)
  );
}

const kObjectType = 0;
const kArrayType = 1;
const kArrayExtrasType = 2;

const kMinLineLength = 16;

// Constants to map the iterator state.
const kWeak = 0;
const kIterator = 1;
const kMapEntries = 2;

// Escaped control characters (plus the single quote and the backslash). Use
// empty strings to fill up unused entries.
// deno-fmt-ignore
const meta = [
  '\\x00', '\\x01', '\\x02', '\\x03', '\\x04', '\\x05', '\\x06', '\\x07', // x07
  '\\b', '\\t', '\\n', '\\x0B', '\\f', '\\r', '\\x0E', '\\x0F',           // x0F
  '\\x10', '\\x11', '\\x12', '\\x13', '\\x14', '\\x15', '\\x16', '\\x17', // x17
  '\\x18', '\\x19', '\\x1A', '\\x1B', '\\x1C', '\\x1D', '\\x1E', '\\x1F', // x1F
  '', '', '', '', '', '', '', "\\'", '', '', '', '', '', '', '', '',      // x2F
  '', '', '', '', '', '', '', '', '', '', '', '', '', '', '', '',         // x3F
  '', '', '', '', '', '', '', '', '', '', '', '', '', '', '', '',         // x4F
  '', '', '', '', '', '', '', '', '', '', '', '', '\\\\', '', '', '',     // x5F
  '', '', '', '', '', '', '', '', '', '', '', '', '', '', '', '',         // x6F
  '', '', '', '', '', '', '', '', '', '', '', '', '', '', '', '\\x7F',    // x7F
  '\\x80', '\\x81', '\\x82', '\\x83', '\\x84', '\\x85', '\\x86', '\\x87', // x87
  '\\x88', '\\x89', '\\x8A', '\\x8B', '\\x8C', '\\x8D', '\\x8E', '\\x8F', // x8F
  '\\x90', '\\x91', '\\x92', '\\x93', '\\x94', '\\x95', '\\x96', '\\x97', // x97
  '\\x98', '\\x99', '\\x9A', '\\x9B', '\\x9C', '\\x9D', '\\x9E', '\\x9F', // x9F
];

// https://tc39.es/ecma262/#sec-IsHTMLDDA-internal-slot
const isUndetectableObject = (v) => typeof v === "undefined" && v !== undefined;

const strEscapeSequencesReplacer = new SafeRegExp(
  "[\x00-\x1f\x27\x5c\x7f-\x9f]",
  "g",
);

const keyStrRegExp = new SafeRegExp("^[a-zA-Z_][a-zA-Z_0-9]*$");
const numberRegExp = new SafeRegExp("^(0|[1-9][0-9]*)$");

// TODO(wafuwafu13): Figure out
const escapeFn = (str) => meta[StringPrototypeCharCodeAt(str, 0)];

function stylizeNoColor(str) {
  return str;
}

// node custom inspect symbol
const nodeCustomInspectSymbol = SymbolFor("nodejs.util.inspect.custom");

// This non-unique symbol is used to support op_crates, ie.
// in extensions/web we don't want to depend on public
// Symbol.for("Deno.customInspect") symbol defined in the public API.
// Internal only, shouldn't be used by users.
const privateCustomInspect = SymbolFor("Deno.privateCustomInspect");

function getUserOptions(ctx, isCrossContext) {
  const ret = {
    stylize: ctx.stylize,
    showHidden: ctx.showHidden,
    depth: ctx.depth,
    colors: ctx.colors,
    customInspect: ctx.customInspect,
    showProxy: ctx.showProxy,
    maxArrayLength: ctx.maxArrayLength,
    maxStringLength: ctx.maxStringLength,
    breakLength: ctx.breakLength,
    compact: ctx.compact,
    sorted: ctx.sorted,
    getters: ctx.getters,
    numericSeparator: ctx.numericSeparator,
    ...ctx.userOptions,
  };

  // Typically, the target value will be an instance of `Object`. If that is
  // *not* the case, the object may come from another vm.Context, and we want
  // to avoid passing it objects from this Context in that case, so we remove
  // the prototype from the returned object itself + the `stylize()` function,
  // and remove all other non-primitives, including non-primitive user options.
  if (isCrossContext) {
    ObjectSetPrototypeOf(ret, null);
    for (const key of new SafeArrayIterator(ObjectKeys(ret))) {
      if (
        (typeof ret[key] === "object" || typeof ret[key] === "function") &&
        ret[key] !== null
      ) {
        delete ret[key];
      }
    }
    ret.stylize = ObjectSetPrototypeOf((value, flavour) => {
      let stylized;
      try {
        stylized = `${ctx.stylize(value, flavour)}`;
      } catch {
        // Continue regardless of error.
      }

      if (typeof stylized !== "string") return value;
      // `stylized` is a string as it should be, which is safe to pass along.
      return stylized;
    }, null);
  }

  return ret;
}

// Note: using `formatValue` directly requires the indentation level to be
// corrected by setting `ctx.indentationLvL += diff` and then to decrease the
// value afterwards again.
function formatValue(
  ctx,
  value,
  recurseTimes,
  typedArray,
) {
  // Primitive types cannot have properties.
  if (
    typeof value !== "object" &&
    typeof value !== "function" &&
    !isUndetectableObject(value)
  ) {
    return formatPrimitive(ctx.stylize, value, ctx);
  }
  if (value === null) {
    return ctx.stylize("null", "null");
  }

  // Memorize the context for custom inspection on proxies.
  const context = value;
  // Always check for proxies to prevent side effects and to prevent triggering
  // any proxy handlers.
  // TODO(wafuwafu13): Set Proxy
  const proxyDetails = core.getProxyDetails(value);
  // const proxy = getProxyDetails(value, !!ctx.showProxy);
  // if (proxy !== undefined) {
  //   if (ctx.showProxy) {
  //     return formatProxy(ctx, proxy, recurseTimes);
  //   }
  //   value = proxy;
  // }

  // Provide a hook for user-specified inspect functions.
  // Check that value is an object with an inspect function on it.
  if (ctx.customInspect) {
    if (
      ReflectHas(value, customInspect) &&
      typeof value[customInspect] === "function"
    ) {
      return String(value[customInspect](inspect, ctx));
    } else if (
      ReflectHas(value, privateCustomInspect) &&
      typeof value[privateCustomInspect] === "function"
    ) {
      // TODO(nayeemrmn): `inspect` is passed as an argument because custom
      // inspect implementations in `extensions` need it, but may not have access
      // to the `Deno` namespace in web workers. Remove when the `Deno`
      // namespace is always enabled.
      return String(value[privateCustomInspect](inspect, ctx));
    } else if (ReflectHas(value, nodeCustomInspectSymbol)) {
      const maybeCustom = value[nodeCustomInspectSymbol];
      if (
        typeof maybeCustom === "function" &&
        // Filter out the util module, its inspect function is special.
        maybeCustom !== ctx.inspect &&
        // Also filter out any prototype objects using the circular check.
        !(value.constructor && value.constructor.prototype === value)
      ) {
        // This makes sure the recurseTimes are reported as before while using
        // a counter internally.
        const depth = ctx.depth === null ? null : ctx.depth - recurseTimes;
        // TODO(@crowlKats): proxy handling
        const isCrossContext = !ObjectPrototypeIsPrototypeOf(
          ObjectPrototype,
          context,
        );
        const ret = FunctionPrototypeCall(
          maybeCustom,
          context,
          depth,
          getUserOptions(ctx, isCrossContext),
          ctx.inspect,
        );
        // If the custom inspection method returned `this`, don't go into
        // infinite recursion.
        if (ret !== context) {
          if (typeof ret !== "string") {
            return formatValue(ctx, ret, recurseTimes);
          }
          return StringPrototypeReplaceAll(
            ret,
            "\n",
            `\n${StringPrototypeRepeat(" ", ctx.indentationLvl)}`,
          );
        }
      }
    }
  }

  // Using an array here is actually better for the average case than using
  // a Set. `seen` will only check for the depth and will never grow too large.
  if (ArrayPrototypeIncludes(ctx.seen, value)) {
    let index = 1;
    if (ctx.circular === undefined) {
      ctx.circular = new SafeMap();
      MapPrototypeSet(ctx.circular, value, index);
    } else {
      index = ctx.circular.get(value);
      if (index === undefined) {
        index = ctx.circular.size + 1;
        MapPrototypeSet(ctx.circular, value, index);
      }
    }
    return ctx.stylize(`[Circular *${index}]`, "special");
  }

  return formatRaw(ctx, value, recurseTimes, typedArray, proxyDetails);
}

function getClassBase(value, constructor, tag) {
  const hasName = ObjectHasOwn(value, "name");
  const name = (hasName && value.name) || "(anonymous)";
  let base = `class ${name}`;
  if (constructor !== "Function" && constructor !== null) {
    base += ` [${constructor}]`;
  }
  if (tag !== "" && constructor !== tag) {
    base += ` [${tag}]`;
  }
  if (constructor !== null) {
    const superName = ObjectGetPrototypeOf(value).name;
    if (superName) {
      base += ` extends ${superName}`;
    }
  } else {
    base += " extends [null prototype]";
  }
  return `[${base}]`;
}

const stripCommentsRegExp = new SafeRegExp(
  "(\\/\\/.*?\\n)|(\\/\\*(.|\\n)*?\\*\\/)",
  "g",
);
const classRegExp = new SafeRegExp("^(\\s+[^(]*?)\\s*{");

function getFunctionBase(value, constructor, tag) {
  const stringified = FunctionPrototypeToString(value);
  if (
    StringPrototypeStartsWith(stringified, "class") &&
    StringPrototypeEndsWith(stringified, "}")
  ) {
    const slice = StringPrototypeSlice(stringified, 5, -1);
    const bracketIndex = StringPrototypeIndexOf(slice, "{");
    if (
      bracketIndex !== -1 &&
      (!StringPrototypeIncludes(
        StringPrototypeSlice(slice, 0, bracketIndex),
        "(",
      ) ||
        // Slow path to guarantee that it's indeed a class.
        RegExpPrototypeExec(
            classRegExp,
            RegExpPrototypeSymbolReplace(stripCommentsRegExp, slice),
          ) !== null)
    ) {
      return getClassBase(value, constructor, tag);
    }
  }
  let type = "Function";
  if (isGeneratorFunction(value)) {
    type = `Generator${type}`;
  }
  if (isAsyncFunction(value)) {
    type = `Async${type}`;
  }
  let base = `[${type}`;
  if (constructor === null) {
    base += " (null prototype)";
  }
  if (value.name === "") {
    base += " (anonymous)";
  } else {
    base += `: ${value.name}`;
  }
  base += "]";
  if (constructor !== type && constructor !== null) {
    base += ` ${constructor}`;
  }
  if (tag !== "" && constructor !== tag) {
    base += ` [${tag}]`;
  }
  return base;
}

function formatRaw(ctx, value, recurseTimes, typedArray, proxyDetails) {
  let keys;
  let protoProps;
  if (ctx.showHidden && (recurseTimes <= ctx.depth || ctx.depth === null)) {
    protoProps = [];
  }

  const constructor = getConstructorName(value, ctx, recurseTimes, protoProps);
  // Reset the variable to check for this later on.
  if (protoProps !== undefined && protoProps.length === 0) {
    protoProps = undefined;
  }

  let tag = value[SymbolToStringTag];
  // Only list the tag in case it's non-enumerable / not an own property.
  // Otherwise we'd print this twice.
  if (
    typeof tag !== "string"
    // TODO(wafuwafu13): Implement
    // (tag !== "" &&
    //   (ctx.showHidden
    //     ? Object.prototype.hasOwnProperty
    //     : Object.prototype.propertyIsEnumerable)(
    //       value,
    //       Symbol.toStringTag,
    //     ))
  ) {
    tag = "";
  }
  let base = "";
  let formatter = () => [];
  let braces;
  let noIterator = true;
  let i = 0;
  const filter = ctx.showHidden ? 0 : 2;

  let extrasType = kObjectType;

  if (proxyDetails !== null && ctx.showProxy) {
    return `Proxy ` + formatValue(ctx, proxyDetails, recurseTimes);
  } else {
    // Iterators and the rest are split to reduce checks.
    // We have to check all values in case the constructor is set to null.
    // Otherwise it would not possible to identify all types properly.
    if (ReflectHas(value, SymbolIterator) || constructor === null) {
      noIterator = false;
      if (ArrayIsArray(value)) {
        // Only set the constructor for non ordinary ("Array [...]") arrays.
        const prefix = (constructor !== "Array" || tag !== "")
          ? getPrefix(constructor, tag, "Array", `(${value.length})`)
          : "";
        keys = op_get_non_index_property_names(value, filter);
        braces = [`${prefix}[`, "]"];
        if (
          value.length === 0 && keys.length === 0 && protoProps === undefined
        ) {
          return `${braces[0]}]`;
        }
        extrasType = kArrayExtrasType;
        formatter = formatArray;
      } else if (
        (proxyDetails === null && isSet(value)) ||
        (proxyDetails !== null && isSet(proxyDetails[0]))
      ) {
        const set = proxyDetails?.[0] ?? value;
        const size = SetPrototypeGetSize(set);
        const prefix = getPrefix(constructor, tag, "Set", `(${size})`);
        keys = getKeys(set, ctx.showHidden);
        formatter = constructor !== null
          ? FunctionPrototypeBind(formatSet, null, set)
          : FunctionPrototypeBind(formatSet, null, SetPrototypeValues(set));
        if (size === 0 && keys.length === 0 && protoProps === undefined) {
          return `${prefix}{}`;
        }
        braces = [`${prefix}{`, "}"];
      } else if (
        (proxyDetails === null && isMap(value)) ||
        (proxyDetails !== null && isMap(proxyDetails[0]))
      ) {
        const map = proxyDetails?.[0] ?? value;
        const size = MapPrototypeGetSize(map);
        const prefix = getPrefix(constructor, tag, "Map", `(${size})`);
        keys = getKeys(map, ctx.showHidden);
        formatter = constructor !== null
          ? FunctionPrototypeBind(formatMap, null, map)
          : FunctionPrototypeBind(formatMap, null, MapPrototypeEntries(map));
        if (size === 0 && keys.length === 0 && protoProps === undefined) {
          return `${prefix}{}`;
        }
        braces = [`${prefix}{`, "}"];
      } else if (
        (proxyDetails === null && isTypedArray(value)) ||
        (proxyDetails !== null && isTypedArray(proxyDetails[0]))
      ) {
        const typedArray = proxyDetails?.[0] ?? value;
        keys = op_get_non_index_property_names(typedArray, filter);
        const bound = typedArray;
        const fallback = "";
        if (constructor === null) {
          // TODO(wafuwafu13): Implement
          // fallback = TypedArrayPrototypeGetSymbolToStringTag(value);
          // // Reconstruct the array information.
          // bound = new primordials[fallback](value);
        }
        const size = TypedArrayPrototypeGetLength(typedArray);
        const prefix = getPrefix(constructor, tag, fallback, `(${size})`);
        braces = [`${prefix}[`, "]"];
        if (typedArray.length === 0 && keys.length === 0 && !ctx.showHidden) {
          return `${braces[0]}]`;
        }
        // Special handle the value. The original value is required below. The
        // bound function is required to reconstruct missing information.
        formatter = FunctionPrototypeBind(formatTypedArray, null, bound, size);
        extrasType = kArrayExtrasType;
      } else if (
        (proxyDetails === null && isMapIterator(value)) ||
        (proxyDetails !== null && isMapIterator(proxyDetails[0]))
      ) {
        const mapIterator = proxyDetails?.[0] ?? value;
        keys = getKeys(mapIterator, ctx.showHidden);
        braces = getIteratorBraces("Map", tag);
        // Add braces to the formatter parameters.
        formatter = FunctionPrototypeBind(formatIterator, null, braces);
      } else if (
        (proxyDetails === null && isSetIterator(value)) ||
        (proxyDetails !== null && isSetIterator(proxyDetails[0]))
      ) {
        const setIterator = proxyDetails?.[0] ?? value;
        keys = getKeys(setIterator, ctx.showHidden);
        braces = getIteratorBraces("Set", tag);
        // Add braces to the formatter parameters.
        formatter = FunctionPrototypeBind(formatIterator, null, braces);
      } else {
        noIterator = true;
      }
    }
    if (noIterator) {
      keys = getKeys(value, ctx.showHidden);
      braces = ["{", "}"];
      if (constructor === "Object") {
        if (isArgumentsObject(value)) {
          braces[0] = "[Arguments] {";
        } else if (tag !== "") {
          braces[0] = `${getPrefix(constructor, tag, "Object")}{`;
        }
        if (keys.length === 0 && protoProps === undefined) {
          return `${braces[0]}}`;
        }
      } else if (typeof value === "function") {
        base = getFunctionBase(value, constructor, tag);
        if (keys.length === 0 && protoProps === undefined) {
          return ctx.stylize(base, "special");
        }
      } else if (
        (proxyDetails === null && isRegExp(value)) ||
        (proxyDetails !== null && isRegExp(proxyDetails[0]))
      ) {
        const regExp = proxyDetails?.[0] ?? value;
        // Make RegExps say that they are RegExps
        base = RegExpPrototypeToString(
          constructor !== null ? regExp : new SafeRegExp(regExp),
        );
        const prefix = getPrefix(constructor, tag, "RegExp");
        if (prefix !== "RegExp ") {
          base = `${prefix}${base}`;
        }
        if (
          (keys.length === 0 && protoProps === undefined) ||
          (recurseTimes > ctx.depth && ctx.depth !== null)
        ) {
          return ctx.stylize(base, "regexp");
        }
      } else if (
        (proxyDetails === null && isDate(value)) ||
        (proxyDetails !== null && isDate(proxyDetails[0]))
      ) {
        const date = proxyDetails?.[0] ?? value;
        if (NumberIsNaN(DatePrototypeGetTime(date))) {
          return ctx.stylize("Invalid Date", "date");
        } else {
          base = DatePrototypeToISOString(date);
          if (keys.length === 0 && protoProps === undefined) {
            return ctx.stylize(base, "date");
          }
        }
      } else if (
        proxyDetails === null &&
        ObjectPrototypeIsPrototypeOf(globalThis.Intl.Locale.prototype, value)
      ) {
        braces[0] = `${getPrefix(constructor, tag, "Intl.Locale")}{`;
        ArrayPrototypeUnshift(
          keys,
          "baseName",
          "calendar",
          "caseFirst",
          "collation",
          "hourCycle",
          "language",
          "numberingSystem",
          "numeric",
          "region",
          "script",
        );
      } else if (
        proxyDetails === null &&
        typeof globalThis.Temporal !== "undefined" &&
        (
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.Instant.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.ZonedDateTime.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.PlainDate.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.PlainTime.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.PlainDateTime.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.PlainYearMonth.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.PlainMonthDay.prototype,
            value,
          ) ||
          ObjectPrototypeIsPrototypeOf(
            globalThis.Temporal.Duration.prototype,
            value,
          )
        )
      ) {
        // Temporal is not available in primordials yet
        // deno-lint-ignore prefer-primordials
        return ctx.stylize(value.toString(), "temporal");
      } else if (
        (proxyDetails === null &&
          (isNativeError(value) ||
            ObjectPrototypeIsPrototypeOf(ErrorPrototype, value))) ||
        (proxyDetails !== null &&
          (isNativeError(proxyDetails[0]) ||
            ObjectPrototypeIsPrototypeOf(ErrorPrototype, proxyDetails[0])))
      ) {
        const error = proxyDetails?.[0] ?? value;
        base = inspectError(error, ctx);
        if (keys.length === 0 && protoProps === undefined) {
          return base;
        }
      } else if (isAnyArrayBuffer(value)) {
        // Fast path for ArrayBuffer and SharedArrayBuffer.
        // Can't do the same for DataView because it has a non-primitive
        // .buffer property that we need to recurse for.
        const arrayType = isArrayBuffer(value)
          ? "ArrayBuffer"
          : "SharedArrayBuffer";

        const prefix = getPrefix(constructor, tag, arrayType);
        if (typedArray === undefined) {
          formatter = formatArrayBuffer;
        } else if (keys.length === 0 && protoProps === undefined) {
          return prefix +
            `{ byteLength: ${
              formatNumber(ctx.stylize, TypedArrayPrototypeGetByteLength(value))
            } }`;
        }
        braces[0] = `${prefix}{`;
        ArrayPrototypeUnshift(keys, "byteLength");
      } else if (isDataView(value)) {
        braces[0] = `${getPrefix(constructor, tag, "DataView")}{`;
        // .buffer goes last, it's not a primitive like the others.
        ArrayPrototypeUnshift(keys, "byteLength", "byteOffset", "buffer");
      } else if (isPromise(value)) {
        braces[0] = `${getPrefix(constructor, tag, "Promise")}{`;
        formatter = formatPromise;
      } else if (isWeakSet(value)) {
        braces[0] = `${getPrefix(constructor, tag, "WeakSet")}{`;
        formatter = ctx.showHidden ? formatWeakSet : formatWeakCollection;
      } else if (isWeakMap(value)) {
        braces[0] = `${getPrefix(constructor, tag, "WeakMap")}{`;
        formatter = ctx.showHidden ? formatWeakMap : formatWeakCollection;
      } else if (isModuleNamespaceObject(value)) {
        braces[0] = `${getPrefix(constructor, tag, "Module")}{`;
        // Special handle keys for namespace objects.
        formatter = FunctionPrototypeBind(formatNamespaceObject, null, keys);
      } else if (isBoxedPrimitive(value)) {
        base = getBoxedBase(value, ctx, keys, constructor, tag);
        if (keys.length === 0 && protoProps === undefined) {
          return base;
        }
      } else {
        if (keys.length === 0 && protoProps === undefined) {
          // TODO(wafuwafu13): Implement
          // if (isExternal(value)) {
          //   const address = getExternalValue(value).toString(16);
          //   return ctx.stylize(`[External: ${address}]`, 'special');
          // }
          return `${getCtxStyle(value, constructor, tag)}{}`;
        }
        braces[0] = `${getCtxStyle(value, constructor, tag)}{`;
      }
    }
  }

  if (recurseTimes > ctx.depth && ctx.depth !== null) {
    let constructorName = StringPrototypeSlice(
      getCtxStyle(value, constructor, tag),
      0,
      -1,
    );
    if (constructor !== null) {
      constructorName = `[${constructorName}]`;
    }
    return ctx.stylize(constructorName, "special");
  }
  recurseTimes += 1;

  ArrayPrototypePush(ctx.seen, value);
  ctx.currentDepth = recurseTimes;
  let output;
  try {
    output = formatter(ctx, value, recurseTimes);
    for (i = 0; i < keys.length; i++) {
      ArrayPrototypePush(
        output,
        formatProperty(ctx, value, recurseTimes, keys[i], extrasType),
      );
    }
    if (protoProps !== undefined) {
      ArrayPrototypePushApply(output, protoProps);
    }
  } catch (error) {
    // TODO(wafuwafu13): Implement stack overflow check
    return ctx.stylize(
      `[Internal Formatting Error] ${error.stack}`,
      "internalError",
    );
  }

  if (ctx.circular !== undefined) {
    const index = ctx.circular.get(value);
    if (index !== undefined) {
      const reference = ctx.stylize(`<ref *${index}>`, "special");
      // Add reference always to the very beginning of the output.
      if (ctx.compact !== true) {
        base = base === "" ? reference : `${reference} ${base}`;
      } else {
        braces[0] = `${reference} ${braces[0]}`;
      }
    }
  }
  ArrayPrototypePop(ctx.seen);

  if (ctx.sorted) {
    const comparator = ctx.sorted === true ? undefined : ctx.sorted;
    if (extrasType === kObjectType) {
      output = ArrayPrototypeSort(output, comparator);
    } else if (keys.length > 1) {
      const sorted = ArrayPrototypeSort(
        ArrayPrototypeSlice(output, output.length - keys.length),
        comparator,
      );
      ArrayPrototypeSplice(
        output,
        output.length - keys.length,
        keys.length,
        ...new SafeArrayIterator(sorted),
      );
    }
  }

  const res = reduceToSingleString(
    ctx,
    output,
    base,
    braces,
    extrasType,
    recurseTimes,
    value,
  );
  const budget = ctx.budget[ctx.indentationLvl] || 0;
  const newLength = budget + res.length;
  ctx.budget[ctx.indentationLvl] = newLength;
  // If any indentationLvl exceeds this limit, limit further inspecting to the
  // minimum. Otherwise the recursive algorithm might continue inspecting the
  // object even though the maximum string size (~2 ** 28 on 32 bit systems and
  // ~2 ** 30 on 64 bit systems) exceeded. The actual output is not limited at
  // exactly 2 ** 27 but a bit higher. This depends on the object shape.
  // This limit also makes sure that huge objects don't block the event loop
  // significantly.
  if (newLength > 2 ** 27) {
    ctx.depth = -1;
  }
  return res;
}

const builtInObjectsRegExp = new SafeRegExp("^[A-Z][a-zA-Z0-9]+$");
const builtInObjects = new SafeSet(
  ArrayPrototypeFilter(
    ObjectGetOwnPropertyNames(globalThis),
    (e) => RegExpPrototypeTest(builtInObjectsRegExp, e),
  ),
);

function addPrototypeProperties(
  ctx,
  main,
  obj,
  recurseTimes,
  output,
) {
  let depth = 0;
  let keys;
  let keySet;
  do {
    if (depth !== 0 || main === obj) {
      obj = ObjectGetPrototypeOf(obj);
      // Stop as soon as a null prototype is encountered.
      if (obj === null) {
        return;
      }
      // Stop as soon as a built-in object type is detected.
      const descriptor = ObjectGetOwnPropertyDescriptor(obj, "constructor");
      if (
        descriptor !== undefined &&
        typeof descriptor.value === "function" &&
        SetPrototypeHas(builtInObjects, descriptor.value.name)
      ) {
        return;
      }
    }

    if (depth === 0) {
      keySet = new SafeSet();
    } else {
      ArrayPrototypeForEach(keys, (key) => SetPrototypeAdd(keySet, key));
    }
    // Get all own property names and symbols.
    keys = ReflectOwnKeys(obj);
    ArrayPrototypePush(ctx.seen, main);
    for (const key of new SafeArrayIterator(keys)) {
      // Ignore the `constructor` property and keys that exist on layers above.
      if (
        key === "constructor" ||
        ObjectHasOwn(main, key) ||
        (depth !== 0 && SetPrototypeHas(keySet, key))
      ) {
        continue;
      }
      const desc = ObjectGetOwnPropertyDescriptor(obj, key);
      if (typeof desc.value === "function") {
        continue;
      }
      const value = formatProperty(
        ctx,
        obj,
        recurseTimes,
        key,
        kObjectType,
        desc,
        main,
      );
      if (ctx.colors) {
        // Faint!
        ArrayPrototypePush(output, `\u001b[2m${value}\u001b[22m`);
      } else {
        ArrayPrototypePush(output, value);
      }
    }
    ArrayPrototypePop(ctx.seen);
    // Limit the inspection to up to three prototype layers. Using `recurseTimes`
    // is not a good choice here, because it's as if the properties are declared
    // on the current object from the users perspective.
  } while (++depth !== 3);
}

function isInstanceof(proto, object) {
  try {
    return ObjectPrototypeIsPrototypeOf(proto, object);
  } catch {
    return false;
  }
}

function getConstructorName(obj, ctx, recurseTimes, protoProps) {
  let firstProto;
  const tmp = obj;
  while (obj || isUndetectableObject(obj)) {
    let descriptor;
    try {
      descriptor = ObjectGetOwnPropertyDescriptor(obj, "constructor");
    } catch {
      /* this could fail */
    }
    if (
      descriptor !== undefined &&
      typeof descriptor.value === "function" &&
      descriptor.value.name !== "" &&
      isInstanceof(descriptor.value.prototype, tmp)
    ) {
      if (
        protoProps !== undefined &&
        (firstProto !== obj ||
          !SetPrototypeHas(builtInObjects, descriptor.value.name))
      ) {
        addPrototypeProperties(
          ctx,
          tmp,
          firstProto || tmp,
          recurseTimes,
          protoProps,
        );
      }
      return String(descriptor.value.name);
    }

    obj = ObjectGetPrototypeOf(obj);
    if (firstProto === undefined) {
      firstProto = obj;
    }
  }

  if (firstProto === null) {
    return null;
  }

  const res = op_get_constructor_name(tmp);

  if (recurseTimes > ctx.depth && ctx.depth !== null) {
    return `${res} <Complex prototype>`;
  }

  const protoConstr = getConstructorName(
    firstProto,
    ctx,
    recurseTimes + 1,
    protoProps,
  );

  if (protoConstr === null) {
    return `${res} <${
      inspect(firstProto, {
        ...ctx,
        customInspect: false,
        depth: -1,
      })
    }>`;
  }

  return `${res} <${protoConstr}>`;
}

const formatPrimitiveRegExp = new SafeRegExp("(?<=\n)");
function formatPrimitive(fn, value, ctx) {
  if (typeof value === "string") {
    let trailer = "";
    if (value.length > ctx.maxStringLength) {
      const remaining = value.length - ctx.maxStringLength;
      value = StringPrototypeSlice(value, 0, ctx.maxStringLength);
      trailer = `... ${remaining} more character${remaining > 1 ? "s" : ""}`;
    }
    if (
      ctx.compact !== true &&
      // TODO(BridgeAR): Add unicode support. Use the readline getStringWidth
      // function.
      value.length > kMinLineLength &&
      value.length > ctx.breakLength - ctx.indentationLvl - 4
    ) {
      return ArrayPrototypeJoin(
        ArrayPrototypeMap(
          StringPrototypeSplit(value, formatPrimitiveRegExp),
          (line) => fn(quoteString(line, ctx), "string"),
        ),
        ` +\n${StringPrototypeRepeat(" ", ctx.indentationLvl + 2)}`,
      ) + trailer;
    }
    return fn(quoteString(value, ctx), "string") + trailer;
  }
  if (typeof value === "number") {
    return formatNumber(fn, value);
  }
  if (typeof value === "bigint") {
    return formatBigInt(fn, value);
  }
  if (typeof value === "boolean") {
    return fn(`${value}`, "boolean");
  }
  if (typeof value === "undefined") {
    return fn("undefined", "undefined");
  }
  // es6 symbol primitive
  return fn(maybeQuoteSymbol(value, ctx), "symbol");
}

function getPrefix(constructor, tag, fallback, size = "") {
  if (constructor === null) {
    if (tag !== "" && fallback !== tag) {
      return `[${fallback}${size}: null prototype] [${tag}] `;
    }
    return `[${fallback}${size}: null prototype] `;
  }

  if (tag !== "" && constructor !== tag) {
    return `${constructor}${size} [${tag}] `;
  }
  return `${constructor}${size} `;
}

function formatArray(ctx, value, recurseTimes) {
  const valLen = value.length;
  const len = MathMin(MathMax(0, ctx.maxArrayLength), valLen);

  const remaining = valLen - len;
  const output = [];
  for (let i = 0; i < len; i++) {
    // Special handle sparse arrays.
    if (!ObjectHasOwn(value, i)) {
      return formatSpecialArray(ctx, value, recurseTimes, len, output, i);
    }
    ArrayPrototypePush(
      output,
      formatProperty(ctx, value, recurseTimes, i, kArrayType),
    );
  }
  if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }
  return output;
}

function getCtxStyle(value, constructor, tag) {
  let fallback = "";
  if (constructor === null) {
    fallback = op_get_constructor_name(value);
    if (fallback === tag) {
      fallback = "Object";
    }
  }
  return getPrefix(constructor, tag, fallback);
}

// Look up the keys of the object.
function getKeys(value, showHidden) {
  let keys;
  const symbols = ObjectGetOwnPropertySymbols(value);
  if (showHidden) {
    keys = ObjectGetOwnPropertyNames(value);
    if (symbols.length !== 0) {
      ArrayPrototypePushApply(keys, symbols);
    }
  } else {
    // This might throw if `value` is a Module Namespace Object from an
    // unevaluated module, but we don't want to perform the actual type
    // check because it's expensive.
    // TODO(devsnek): track https://github.com/tc39/ecma262/issues/1209
    // and modify this logic as needed.
    try {
      keys = ObjectKeys(value);
    } catch (err) {
      assert(
        isNativeError(err) && err.name === "ReferenceError" &&
          isModuleNamespaceObject(value),
      );
      keys = ObjectGetOwnPropertyNames(value);
    }
    if (symbols.length !== 0) {
      const filter = (key) => ObjectPrototypePropertyIsEnumerable(value, key);
      ArrayPrototypePushApply(keys, ArrayPrototypeFilter(symbols, filter));
    }
  }
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, value)) {
    keys = ArrayPrototypeFilter(keys, (key) => key !== "cause");
  }
  return keys;
}

function formatSet(value, ctx, _ignored, recurseTimes) {
  ctx.indentationLvl += 2;

  const values = [...new SafeSetIterator(value)];
  const valLen = SetPrototypeGetSize(value);
  const len = MathMin(MathMax(0, ctx.iterableLimit), valLen);

  const remaining = valLen - len;
  const output = [];
  for (let i = 0; i < len; i++) {
    ArrayPrototypePush(output, formatValue(ctx, values[i], recurseTimes));
  }
  if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }

  ctx.indentationLvl -= 2;
  return output;
}

function formatMap(value, ctx, _ignored, recurseTimes) {
  ctx.indentationLvl += 2;

  const values = [...new SafeMapIterator(value)];
  const valLen = MapPrototypeGetSize(value);
  const len = MathMin(MathMax(0, ctx.iterableLimit), valLen);

  const remaining = valLen - len;
  const output = [];
  for (let i = 0; i < len; i++) {
    ArrayPrototypePush(
      output,
      `${formatValue(ctx, values[i][0], recurseTimes)} => ${
        formatValue(ctx, values[i][1], recurseTimes)
      }`,
    );
  }
  if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }

  ctx.indentationLvl -= 2;
  return output;
}

function formatTypedArray(
  value,
  length,
  ctx,
  _ignored,
  recurseTimes,
) {
  const maxLength = MathMin(MathMax(0, ctx.maxArrayLength), length);
  const remaining = value.length - maxLength;
  const output = [];
  const elementFormatter = value.length > 0 && typeof value[0] === "number"
    ? formatNumber
    : formatBigInt;
  for (let i = 0; i < maxLength; ++i) {
    output[i] = elementFormatter(ctx.stylize, value[i]);
  }
  if (remaining > 0) {
    output[maxLength] = `... ${remaining} more item${remaining > 1 ? "s" : ""}`;
  }
  if (ctx.showHidden) {
    // .buffer goes last, it's not a primitive like the others.
    // All besides `BYTES_PER_ELEMENT` are actually getters.
    ctx.indentationLvl += 2;
    for (
      const key of new SafeArrayIterator([
        "BYTES_PER_ELEMENT",
        "length",
        "byteLength",
        "byteOffset",
        "buffer",
      ])
    ) {
      const str = formatValue(ctx, value[key], recurseTimes, true);
      ArrayPrototypePush(output, `[${key}]: ${str}`);
    }
    ctx.indentationLvl -= 2;
  }
  return output;
}

function getIteratorBraces(type, tag) {
  if (tag !== `${type} Iterator`) {
    if (tag !== "") {
      tag += "] [";
    }
    tag += `${type} Iterator`;
  }
  return [`[${tag}] {`, "}"];
}

const iteratorRegExp = new SafeRegExp(" Iterator] {$");
function formatIterator(braces, ctx, value, recurseTimes) {
  const { 0: entries, 1: isKeyValue } = op_preview_entries(value, true);
  if (isKeyValue) {
    // Mark entry iterators as such.
    braces[0] = StringPrototypeReplace(
      braces[0],
      iteratorRegExp,
      " Entries] {",
    );
    return formatMapIterInner(ctx, recurseTimes, entries, kMapEntries);
  }

  return formatSetIterInner(ctx, recurseTimes, entries, kIterator);
}

function handleCircular(value, ctx) {
  let index = 1;
  if (ctx.circular === undefined) {
    ctx.circular = new SafeMap();
    MapPrototypeSet(ctx.circular, value, index);
  } else {
    index = MapPrototypeGet(ctx.circular, value);
    if (index === undefined) {
      index = MapPrototypeGetSize(ctx.circular) + 1;
      MapPrototypeSet(ctx.circular, value, index);
    }
  }
  // Circular string is cyan
  return ctx.stylize(`[Circular *${index}]`, "special");
}

const AGGREGATE_ERROR_HAS_AT_PATTERN = new SafeRegExp(/\s+at/);
const AGGREGATE_ERROR_NOT_EMPTY_LINE_PATTERN = new SafeRegExp(/^(?!\s*$)/gm);

function inspectError(value, ctx) {
  const causes = [value];

  let err = value;
  while (err.cause) {
    if (ArrayPrototypeIncludes(causes, err.cause)) {
      ArrayPrototypePush(causes, handleCircular(err.cause, ctx));
      break;
    } else {
      ArrayPrototypePush(causes, err.cause);
      err = err.cause;
    }
  }

  const refMap = new SafeMap();
  for (let i = 0; i < causes.length; ++i) {
    const cause = causes[i];
    if (ctx.circular !== undefined) {
      const index = MapPrototypeGet(ctx.circular, cause);
      if (index !== undefined) {
        MapPrototypeSet(
          refMap,
          cause,
          ctx.stylize(`<ref *${index}> `, "special"),
        );
      }
    }
  }
  ArrayPrototypeShift(causes);

  let finalMessage = MapPrototypeGet(refMap, value) ?? "";

  if (isAggregateError(value)) {
    const stackLines = StringPrototypeSplit(value.stack, "\n");
    while (true) {
      const line = ArrayPrototypeShift(stackLines);
      if (RegExpPrototypeTest(AGGREGATE_ERROR_HAS_AT_PATTERN, line)) {
        ArrayPrototypeUnshift(stackLines, line);
        break;
      } else if (typeof line === "undefined") {
        break;
      }

      finalMessage += line;
      finalMessage += "\n";
    }
    const aggregateMessage = ArrayPrototypeJoin(
      ArrayPrototypeMap(
        value.errors,
        (error) =>
          StringPrototypeReplace(
            inspectArgs([error]),
            AGGREGATE_ERROR_NOT_EMPTY_LINE_PATTERN,
            StringPrototypeRepeat(" ", 4),
          ),
      ),
      "\n",
    );
    finalMessage += aggregateMessage;
    finalMessage += "\n";
    finalMessage += ArrayPrototypeJoin(stackLines, "\n");
  } else {
    const stack = value.stack;
    if (stack?.includes("\n    at")) {
      finalMessage += stack;
    } else {
      finalMessage += `[${stack || ErrorPrototypeToString(value)}]`;
    }
  }
  const doubleQuoteRegExp = new SafeRegExp('"', "g");
  finalMessage += ArrayPrototypeJoin(
    ArrayPrototypeMap(
      causes,
      (cause) =>
        "\nCaused by " + (MapPrototypeGet(refMap, cause) ?? "") +
        (cause?.stack ??
          StringPrototypeReplace(
            inspect(cause),
            doubleQuoteRegExp,
            "",
          )),
    ),
    "",
  );

  return finalMessage;
}

const hexSliceLookupTable = function () {
  const alphabet = "0123456789abcdef";
  const table = [];
  for (let i = 0; i < 16; ++i) {
    const i16 = i * 16;
    for (let j = 0; j < 16; ++j) {
      table[i16 + j] = alphabet[i] + alphabet[j];
    }
  }
  return table;
}();

function hexSlice(buf, start, end) {
  const len = TypedArrayPrototypeGetLength(buf);
  if (!start || start < 0) {
    start = 0;
  }
  if (!end || end < 0 || end > len) {
    end = len;
  }
  let out = "";
  for (let i = start; i < end; ++i) {
    out += hexSliceLookupTable[buf[i]];
  }
  return out;
}

const arrayBufferRegExp = new SafeRegExp("(.{2})", "g");
function formatArrayBuffer(ctx, value) {
  let valLen;
  try {
    valLen = ArrayBufferPrototypeGetByteLength(value);
  } catch {
    valLen = getSharedArrayBufferByteLength(value);
  }
  const len = MathMin(MathMax(0, ctx.maxArrayLength), valLen);
  let buffer;
  try {
    buffer = new Uint8Array(value, 0, len);
  } catch {
    return [ctx.stylize("(detached)", "special")];
  }
  let str = StringPrototypeTrim(
    StringPrototypeReplace(hexSlice(buffer), arrayBufferRegExp, "$1 "),
  );

  const remaining = valLen - len;
  if (remaining > 0) {
    str += ` ... ${remaining} more byte${remaining > 1 ? "s" : ""}`;
  }
  return [`${ctx.stylize("[Uint8Contents]", "special")}: <${str}>`];
}

function formatNumber(fn, value) {
  // Format -0 as '-0'. Checking `value === -0` won't distinguish 0 from -0.
  return fn(ObjectIs(value, -0) ? "-0" : `${value}`, "number");
}

const PromiseState = {
  Pending: 0,
  Fulfilled: 1,
  Rejected: 2,
};

function formatPromise(ctx, value, recurseTimes) {
  let output;
  const { 0: state, 1: result } = core.getPromiseDetails(value);
  if (state === PromiseState.Pending) {
    output = [ctx.stylize("<pending>", "special")];
  } else {
    ctx.indentationLvl += 2;
    const str = formatValue(ctx, result, recurseTimes);
    ctx.indentationLvl -= 2;
    output = [
      state === PromiseState.Rejected
        ? `${ctx.stylize("<rejected>", "special")} ${str}`
        : str,
    ];
  }
  return output;
}

function formatWeakCollection(ctx) {
  return [ctx.stylize("<items unknown>", "special")];
}

function formatWeakSet(ctx, value, recurseTimes) {
  const entries = op_preview_entries(value, false);
  return formatSetIterInner(ctx, recurseTimes, entries, kWeak);
}

function formatWeakMap(ctx, value, recurseTimes) {
  const entries = op_preview_entries(value, false);
  return formatMapIterInner(ctx, recurseTimes, entries, kWeak);
}

function formatProperty(
  ctx,
  value,
  recurseTimes,
  key,
  type,
  desc,
  original = value,
) {
  let name, str;
  let extra = " ";
  desc = desc || ObjectGetOwnPropertyDescriptor(value, key) ||
    { value: value[key], enumerable: true };
  if (desc.value !== undefined) {
    const diff = (ctx.compact !== true || type !== kObjectType) ? 2 : 3;
    ctx.indentationLvl += diff;
    str = formatValue(ctx, desc.value, recurseTimes);
    if (diff === 3 && ctx.breakLength < getStringWidth(str, ctx.colors)) {
      extra = `\n${StringPrototypeRepeat(" ", ctx.indentationLvl)}`;
    }
    ctx.indentationLvl -= diff;
  } else if (desc.get !== undefined) {
    const label = desc.set !== undefined ? "Getter/Setter" : "Getter";
    const s = ctx.stylize;
    const sp = "special";
    if (
      ctx.getters && (ctx.getters === true ||
        (ctx.getters === "get" && desc.set === undefined) ||
        (ctx.getters === "set" && desc.set !== undefined))
    ) {
      try {
        const tmp = FunctionPrototypeCall(desc.get, original);
        ctx.indentationLvl += 2;
        if (tmp === null) {
          str = `${s(`[${label}:`, sp)} ${s("null", "null")}${s("]", sp)}`;
        } else if (typeof tmp === "object") {
          str = `${s(`[${label}]`, sp)} ${formatValue(ctx, tmp, recurseTimes)}`;
        } else {
          const primitive = formatPrimitive(s, tmp, ctx);
          str = `${s(`[${label}:`, sp)} ${primitive}${s("]", sp)}`;
        }
        ctx.indentationLvl -= 2;
      } catch (err) {
        const message = `<Inspection threw (${err.message})>`;
        str = `${s(`[${label}:`, sp)} ${message}${s("]", sp)}`;
      }
    } else {
      str = ctx.stylize(`[${label}]`, sp);
    }
  } else if (desc.set !== undefined) {
    str = ctx.stylize("[Setter]", "special");
  } else {
    str = ctx.stylize("undefined", "undefined");
  }
  if (type === kArrayType) {
    return str;
  }
  if (typeof key === "symbol") {
    name = `[${ctx.stylize(maybeQuoteSymbol(key, ctx), "symbol")}]`;
  } else if (key === "__proto__") {
    name = "['__proto__']";
  } else if (desc.enumerable === false) {
    const tmp = StringPrototypeReplace(
      key,
      strEscapeSequencesReplacer,
      escapeFn,
    );

    name = `[${tmp}]`;
  } else if (keyStrRegExp.test(key)) {
    name = ctx.stylize(key, "name");
  } else {
    name = ctx.stylize(quoteString(key, ctx), "string");
  }
  return `${name}:${extra}${str}`;
}

const colorRegExp = new SafeRegExp("\u001b\\[\\d\\d?m", "g");
function removeColors(str) {
  return StringPrototypeReplace(str, colorRegExp, "");
}

function isBelowBreakLength(ctx, output, start, base) {
  // Each entry is separated by at least a comma. Thus, we start with a total
  // length of at least `output.length`. In addition, some cases have a
  // whitespace in-between each other that is added to the total as well.
  // TODO(BridgeAR): Add unicode support. Use the readline getStringWidth
  // function. Check the performance overhead and make it an opt-in in case it's
  // significant.
  let totalLength = output.length + start;
  if (totalLength + output.length > ctx.breakLength) {
    return false;
  }
  for (let i = 0; i < output.length; i++) {
    if (ctx.colors) {
      totalLength += removeColors(output[i]).length;
    } else {
      totalLength += output[i].length;
    }
    if (totalLength > ctx.breakLength) {
      return false;
    }
  }
  // Do not line up properties on the same line if `base` contains line breaks.
  return base === "" || !StringPrototypeIncludes(base, "\n");
}

function formatBigInt(fn, value) {
  return fn(`${value}n`, "bigint");
}

function formatNamespaceObject(
  keys,
  ctx,
  value,
  recurseTimes,
) {
  const output = [];
  for (let i = 0; i < keys.length; i++) {
    try {
      output[i] = formatProperty(
        ctx,
        value,
        recurseTimes,
        keys[i],
        kObjectType,
      );
    } catch (_err) {
      // TODO(wafuwfu13): Implement
      // assert(isNativeError(err) && err.name === 'ReferenceError');
      // Use the existing functionality. This makes sure the indentation and
      // line breaks are always correct. Otherwise it is very difficult to keep
      // this aligned, even though this is a hacky way of dealing with this.
      const tmp = { [keys[i]]: "" };
      output[i] = formatProperty(ctx, tmp, recurseTimes, keys[i], kObjectType);
      const pos = StringPrototypeLastIndexOf(output[i], " ");
      // We have to find the last whitespace and have to replace that value as
      // it will be visualized as a regular string.
      output[i] = StringPrototypeSlice(output[i], 0, pos + 1) +
        ctx.stylize("<uninitialized>", "special");
    }
  }
  // Reset the keys to an empty array. This prevents duplicated inspection.
  keys.length = 0;
  return output;
}

// The array is sparse and/or has extra keys
function formatSpecialArray(
  ctx,
  value,
  recurseTimes,
  maxLength,
  output,
  i,
) {
  const keys = ObjectKeys(value);
  let index = i;
  for (; i < keys.length && output.length < maxLength; i++) {
    const key = keys[i];
    const tmp = +key;
    // Arrays can only have up to 2^32 - 1 entries
    if (tmp > 2 ** 32 - 2) {
      break;
    }
    if (`${index}` !== key) {
      if (!numberRegExp.test(key)) {
        break;
      }
      const emptyItems = tmp - index;
      const ending = emptyItems > 1 ? "s" : "";
      const message = `<${emptyItems} empty item${ending}>`;
      ArrayPrototypePush(output, ctx.stylize(message, "undefined"));
      index = tmp;
      if (output.length === maxLength) {
        break;
      }
    }
    ArrayPrototypePush(
      output,
      formatProperty(ctx, value, recurseTimes, key, kArrayType),
    );
    index++;
  }
  const remaining = value.length - index;
  if (output.length !== maxLength) {
    if (remaining > 0) {
      const ending = remaining > 1 ? "s" : "";
      const message = `<${remaining} empty item${ending}>`;
      ArrayPrototypePush(output, ctx.stylize(message, "undefined"));
    }
  } else if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }
  return output;
}

function getBoxedBase(
  value,
  ctx,
  keys,
  constructor,
  tag,
) {
  let type, primitive;
  if (isNumberObject(value)) {
    type = "Number";
    primitive = NumberPrototypeValueOf(value);
  } else if (isStringObject(value)) {
    type = "String";
    primitive = StringPrototypeValueOf(value);
    // For boxed Strings, we have to remove the 0-n indexed entries,
    // since they just noisy up the output and are redundant
    // Make boxed primitive Strings look like such
    ArrayPrototypeSplice(keys, 0, value.length);
  } else if (isBooleanObject(value)) {
    type = "Boolean";
    primitive = BooleanPrototypeValueOf(value);
  } else if (isBigIntObject(value)) {
    type = "BigInt";
    primitive = BigIntPrototypeValueOf(value);
  } else {
    type = "Symbol";
    primitive = SymbolPrototypeValueOf(value);
  }

  let base = `[${type}`;
  if (type !== constructor) {
    if (constructor === null) {
      base += " (null prototype)";
    } else {
      base += ` (${constructor})`;
    }
  }
  base += `: ${formatPrimitive(stylizeNoColor, primitive, ctx)}]`;
  if (tag !== "" && tag !== constructor) {
    base += ` [${tag}]`;
  }
  if (keys.length !== 0 || ctx.stylize === stylizeNoColor) {
    return base;
  }
  return ctx.stylize(base, StringPrototypeToLowerCase(type));
}

function reduceToSingleString(
  ctx,
  output,
  base,
  braces,
  extrasType,
  recurseTimes,
  value,
) {
  if (ctx.compact !== true) {
    if (typeof ctx.compact === "number" && ctx.compact >= 1) {
      // Memorize the original output length. In case the output is grouped,
      // prevent lining up the entries on a single line.
      const entries = output.length;
      // Group array elements together if the array contains at least six
      // separate entries.
      if (extrasType === kArrayExtrasType && entries > 6) {
        output = groupArrayElements(ctx, output, value);
      }
      // `ctx.currentDepth` is set to the most inner depth of the currently
      // inspected object part while `recurseTimes` is the actual current depth
      // that is inspected.
      //
      // Example:
      //
      // const a = { first: [ 1, 2, 3 ], second: { inner: [ 1, 2, 3 ] } }
      //
      // The deepest depth of `a` is 2 (a.second.inner) and `a.first` has a max
      // depth of 1.
      //
      // Consolidate all entries of the local most inner depth up to
      // `ctx.compact`, as long as the properties are smaller than
      // `ctx.breakLength`.
      if (
        ctx.currentDepth - recurseTimes < ctx.compact &&
        entries === output.length
      ) {
        // Line up all entries on a single line in case the entries do not
        // exceed `breakLength`. Add 10 as constant to start next to all other
        // factors that may reduce `breakLength`.
        const start = output.length + ctx.indentationLvl +
          braces[0].length + base.length + 10;
        if (isBelowBreakLength(ctx, output, start, base)) {
          const joinedOutput = ArrayPrototypeJoin(output, ", ");
          if (!StringPrototypeIncludes(joinedOutput, "\n")) {
            return `${base ? `${base} ` : ""}${braces[0]} ${joinedOutput}` +
              ` ${braces[1]}`;
          }
        }
      }
    }
    // Line up each entry on an individual line.
    const indentation = `\n${StringPrototypeRepeat(" ", ctx.indentationLvl)}`;
    return `${base ? `${base} ` : ""}${braces[0]}${indentation}  ` +
      `${ArrayPrototypeJoin(output, `,${indentation}  `)}${
        ctx.trailingComma ? "," : ""
      }${indentation}${braces[1]}`;
  }
  // Line up all entries on a single line in case the entries do not exceed
  // `breakLength`.
  if (isBelowBreakLength(ctx, output, 0, base)) {
    return `${braces[0]}${base ? ` ${base}` : ""} ${
      ArrayPrototypeJoin(output, ", ")
    } ` +
      braces[1];
  }
  const indentation = StringPrototypeRepeat(" ", ctx.indentationLvl);
  // If the opening "brace" is too large, like in the case of "Set {",
  // we need to force the first item to be on the next line or the
  // items will not line up correctly.
  const ln = base === "" && braces[0].length === 1
    ? " "
    : `${base ? ` ${base}` : ""}\n${indentation}  `;
  // Line up each entry on an individual line.
  return `${braces[0]}${ln}${
    ArrayPrototypeJoin(output, `,\n${indentation}  `)
  } ${braces[1]}`;
}

function groupArrayElements(ctx, output, value) {
  let totalLength = 0;
  let maxLength = 0;
  let i = 0;
  let outputLength = output.length;
  if (ctx.maxArrayLength < output.length) {
    // This makes sure the "... n more items" part is not taken into account.
    outputLength--;
  }
  const separatorSpace = 2; // Add 1 for the space and 1 for the separator.
  const dataLen = [];
  // Calculate the total length of all output entries and the individual max
  // entries length of all output entries. We have to remove colors first,
  // otherwise the length would not be calculated properly.
  for (; i < outputLength; i++) {
    const len = getStringWidth(output[i], ctx.colors);
    dataLen[i] = len;
    totalLength += len + separatorSpace;
    if (maxLength < len) {
      maxLength = len;
    }
  }
  // Add two to `maxLength` as we add a single whitespace character plus a comma
  // in-between two entries.
  const actualMax = maxLength + separatorSpace;
  // Check if at least three entries fit next to each other and prevent grouping
  // of arrays that contains entries of very different length (i.e., if a single
  // entry is longer than 1/5 of all other entries combined). Otherwise the
  // space in-between small entries would be enormous.
  if (
    actualMax * 3 + ctx.indentationLvl < ctx.breakLength &&
    (totalLength / actualMax > 5 || maxLength <= 6)
  ) {
    const approxCharHeights = 2.5;
    const averageBias = MathSqrt(actualMax - totalLength / output.length);
    const biasedMax = MathMax(actualMax - 3 - averageBias, 1);
    // Dynamically check how many columns seem possible.
    const columns = MathMin(
      // Ideally a square should be drawn. We expect a character to be about 2.5
      // times as high as wide. This is the area formula to calculate a square
      // which contains n rectangles of size `actualMax * approxCharHeights`.
      // Divide that by `actualMax` to receive the correct number of columns.
      // The added bias increases the columns for short entries.
      MathRound(
        MathSqrt(
          approxCharHeights * biasedMax * outputLength,
        ) / biasedMax,
      ),
      // Do not exceed the breakLength.
      MathFloor((ctx.breakLength - ctx.indentationLvl) / actualMax),
      // Limit array grouping for small `compact` modes as the user requested
      // minimal grouping.
      ctx.compact * 4,
      // Limit the columns to a maximum of fifteen.
      15,
    );
    // Return with the original output if no grouping should happen.
    if (columns <= 1) {
      return output;
    }
    const tmp = [];
    const maxLineLength = [];
    for (let i = 0; i < columns; i++) {
      let lineMaxLength = 0;
      for (let j = i; j < output.length; j += columns) {
        if (dataLen[j] > lineMaxLength) {
          lineMaxLength = dataLen[j];
        }
      }
      lineMaxLength += separatorSpace;
      maxLineLength[i] = lineMaxLength;
    }
    let order = StringPrototypePadStart;
    if (value !== undefined) {
      for (let i = 0; i < output.length; i++) {
        if (typeof value[i] !== "number" && typeof value[i] !== "bigint") {
          order = StringPrototypePadEnd;
          break;
        }
      }
    }
    // Each iteration creates a single line of grouped entries.
    for (let i = 0; i < outputLength; i += columns) {
      // The last lines may contain less entries than columns.
      const max = MathMin(i + columns, outputLength);
      let str = "";
      let j = i;
      for (; j < max - 1; j++) {
        // Calculate extra color padding in case it's active. This has to be
        // done line by line as some lines might contain more colors than
        // others.
        const padding = maxLineLength[j - i] + output[j].length - dataLen[j];
        str += order(`${output[j]}, `, padding, " ");
      }
      if (order === StringPrototypePadStart) {
        const padding = maxLineLength[j - i] +
          output[j].length -
          dataLen[j] -
          separatorSpace;
        str += StringPrototypePadStart(output[j], padding, " ");
      } else {
        str += output[j];
      }
      ArrayPrototypePush(tmp, str);
    }
    if (ctx.maxArrayLength < output.length) {
      ArrayPrototypePush(tmp, output[outputLength]);
    }
    output = tmp;
  }
  return output;
}

function formatMapIterInner(
  ctx,
  recurseTimes,
  entries,
  state,
) {
  const maxArrayLength = MathMax(ctx.maxArrayLength, 0);
  // Entries exist as [key1, val1, key2, val2, ...]
  const len = entries.length / 2;
  const remaining = len - maxArrayLength;
  const maxLength = MathMin(maxArrayLength, len);
  const output = [];
  let i = 0;
  ctx.indentationLvl += 2;
  if (state === kWeak) {
    for (; i < maxLength; i++) {
      const pos = i * 2;
      output[i] = `${formatValue(ctx, entries[pos], recurseTimes)} => ${
        formatValue(ctx, entries[pos + 1], recurseTimes)
      }`;
    }
    // Sort all entries to have a halfway reliable output (if more entries than
    // retrieved ones exist, we can not reliably return the same output) if the
    // output is not sorted anyway.
    if (!ctx.sorted) {
      ArrayPrototypeSort(output);
    }
  } else {
    for (; i < maxLength; i++) {
      const pos = i * 2;
      const res = [
        formatValue(ctx, entries[pos], recurseTimes),
        formatValue(ctx, entries[pos + 1], recurseTimes),
      ];
      output[i] = reduceToSingleString(
        ctx,
        res,
        "",
        ["[", "]"],
        kArrayExtrasType,
        recurseTimes,
      );
    }
  }
  ctx.indentationLvl -= 2;
  if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }
  return output;
}

function formatSetIterInner(
  ctx,
  recurseTimes,
  entries,
  state,
) {
  const maxArrayLength = MathMax(ctx.maxArrayLength, 0);
  const maxLength = MathMin(maxArrayLength, entries.length);
  const output = [];
  ctx.indentationLvl += 2;
  for (let i = 0; i < maxLength; i++) {
    output[i] = formatValue(ctx, entries[i], recurseTimes);
  }
  ctx.indentationLvl -= 2;
  if (state === kWeak && !ctx.sorted) {
    // Sort all entries to have a halfway reliable output (if more entries than
    // retrieved ones exist, we can not reliably return the same output) if the
    // output is not sorted anyway.
    ArrayPrototypeSort(output);
  }
  const remaining = entries.length - maxLength;
  if (remaining > 0) {
    ArrayPrototypePush(
      output,
      `... ${remaining} more item${remaining > 1 ? "s" : ""}`,
    );
  }
  return output;
}

// Regex used for ansi escape code splitting
// Adopted from https://github.com/chalk/ansi-regex/blob/HEAD/index.js
// License: MIT, authors: @sindresorhus, Qix-, arjunmehta and LitoMore
// Matches all ansi escape code sequences in a string
const ansiPattern = "[\\u001B\\u009B][[\\]()#;?]*" +
  "(?:(?:(?:(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]+)*" +
  "|[a-zA-Z\\d]+(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)" +
  "|(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))";
const ansi = new SafeRegExp(ansiPattern, "g");

/**
 * Returns the number of columns required to display the given string.
 */
export function getStringWidth(str, removeControlChars = true) {
  let width = 0;

  if (removeControlChars) {
    str = stripVTControlCharacters(str);
  }
  str = StringPrototypeNormalize(str, "NFC");
  for (const char of new SafeStringIterator(str)) {
    const code = StringPrototypeCodePointAt(char, 0);
    if (isFullWidthCodePoint(code)) {
      width += 2;
    } else if (!isZeroWidthCodePoint(code)) {
      width++;
    }
  }

  return width;
}

const isZeroWidthCodePoint = (code) => {
  return code <= 0x1F || // C0 control codes
    (code >= 0x7F && code <= 0x9F) || // C1 control codes
    (code >= 0x300 && code <= 0x36F) || // Combining Diacritical Marks
    (code >= 0x200B && code <= 0x200F) || // Modifying Invisible Characters
    // Combining Diacritical Marks for Symbols
    (code >= 0x20D0 && code <= 0x20FF) ||
    (code >= 0xFE00 && code <= 0xFE0F) || // Variation Selectors
    (code >= 0xFE20 && code <= 0xFE2F) || // Combining Half Marks
    (code >= 0xE0100 && code <= 0xE01EF); // Variation Selectors
};

/**
 * Remove all VT control characters. Use to estimate displayed string width.
 */
export function stripVTControlCharacters(str) {
  return StringPrototypeReplace(str, ansi, "");
}

function hasOwnProperty(obj, v) {
  if (obj == null) {
    return false;
  }
  return ObjectHasOwn(obj, v);
}

// Copyright Joyent, Inc. and other Node contributors. MIT license.
// Forked from Node's lib/internal/cli_table.js

const tableChars = {
  middleMiddle: "\u2500",
  rowMiddle: "\u253c",
  topRight: "\u2510",
  topLeft: "\u250c",
  leftMiddle: "\u251c",
  topMiddle: "\u252c",
  bottomRight: "\u2518",
  bottomLeft: "\u2514",
  bottomMiddle: "\u2534",
  rightMiddle: "\u2524",
  left: "\u2502 ",
  right: " \u2502",
  middle: " \u2502 ",
};

function isFullWidthCodePoint(code) {
  // Code points are partially derived from:
  // http://www.unicode.org/Public/UNIDATA/EastAsianWidth.txt
  return (
    code >= 0x1100 &&
    (code <= 0x115f || // Hangul Jamo
      code === 0x2329 || // LEFT-POINTING ANGLE BRACKET
      code === 0x232a || // RIGHT-POINTING ANGLE BRACKET
      // CJK Radicals Supplement .. Enclosed CJK Letters and Months
      (code >= 0x2e80 && code <= 0x3247 && code !== 0x303f) ||
      // Enclosed CJK Letters and Months .. CJK Unified Ideographs Extension A
      (code >= 0x3250 && code <= 0x4dbf) ||
      // CJK Unified Ideographs .. Yi Radicals
      (code >= 0x4e00 && code <= 0xa4c6) ||
      // Hangul Jamo Extended-A
      (code >= 0xa960 && code <= 0xa97c) ||
      // Hangul Syllables
      (code >= 0xac00 && code <= 0xd7a3) ||
      // CJK Compatibility Ideographs
      (code >= 0xf900 && code <= 0xfaff) ||
      // Vertical Forms
      (code >= 0xfe10 && code <= 0xfe19) ||
      // CJK Compatibility Forms .. Small Form Variants
      (code >= 0xfe30 && code <= 0xfe6b) ||
      // Halfwidth and Fullwidth Forms
      (code >= 0xff01 && code <= 0xff60) ||
      (code >= 0xffe0 && code <= 0xffe6) ||
      // Kana Supplement
      (code >= 0x1b000 && code <= 0x1b001) ||
      // Enclosed Ideographic Supplement
      (code >= 0x1f200 && code <= 0x1f251) ||
      // Miscellaneous Symbols and Pictographs 0x1f300 - 0x1f5ff
      // Emoticons 0x1f600 - 0x1f64f
      (code >= 0x1f300 && code <= 0x1f64f) ||
      // CJK Unified Ideographs Extension B .. Tertiary Ideographic Plane
      (code >= 0x20000 && code <= 0x3fffd))
  );
}

function renderRow(row, columnWidths, columnRightAlign) {
  let out = tableChars.left;
  for (let i = 0; i < row.length; i++) {
    const cell = row[i];
    const len = getStringWidth(cell);
    const padding = StringPrototypeRepeat(" ", columnWidths[i] - len);
    if (columnRightAlign?.[i]) {
      out += `${padding}${cell}`;
    } else {
      out += `${cell}${padding}`;
    }
    if (i !== row.length - 1) {
      out += tableChars.middle;
    }
  }
  out += tableChars.right;
  return out;
}

function cliTable(head, columns) {
  const rows = [];
  const columnWidths = ArrayPrototypeMap(head, (h) => getStringWidth(h));
  const longestColumn = ArrayPrototypeReduce(
    columns,
    (n, a) => MathMax(n, a.length),
    0,
  );
  const columnRightAlign = ArrayPrototypeFill(
    new Array(columnWidths.length),
    true,
  );

  for (let i = 0; i < head.length; i++) {
    const column = columns[i];
    for (let j = 0; j < longestColumn; j++) {
      if (rows[j] === undefined) {
        rows[j] = [];
      }
      const value = (rows[j][i] = hasOwnProperty(column, j) ? column[j] : "");
      const width = columnWidths[i] || 0;
      const counted = getStringWidth(value);
      columnWidths[i] = MathMax(width, counted);
      columnRightAlign[i] &= NumberIsInteger(+value);
    }
  }

  const divider = ArrayPrototypeMap(
    columnWidths,
    (i) => StringPrototypeRepeat(tableChars.middleMiddle, i + 2),
  );

  let result =
    `${tableChars.topLeft}${
      ArrayPrototypeJoin(divider, tableChars.topMiddle)
    }` +
    `${tableChars.topRight}\n${renderRow(head, columnWidths)}\n` +
    `${tableChars.leftMiddle}${
      ArrayPrototypeJoin(divider, tableChars.rowMiddle)
    }` +
    `${tableChars.rightMiddle}\n`;

  for (let i = 0; i < rows.length; ++i) {
    const row = rows[i];
    result += `${renderRow(row, columnWidths, columnRightAlign)}\n`;
  }

  result +=
    `${tableChars.bottomLeft}${
      ArrayPrototypeJoin(divider, tableChars.bottomMiddle)
    }` +
    tableChars.bottomRight;

  return result;
}
/* End of forked part */

// We can match Node's quoting behavior exactly by swapping the double quote and
// single quote in this array. That would give preference to single quotes.
// However, we prefer double quotes as the default.

const denoInspectDefaultOptions = {
  indentationLvl: 0,
  currentDepth: 0,
  stylize: stylizeNoColor,

  showHidden: false,
  depth: 4,
  colors: false,
  showProxy: false,
  breakLength: 80,
  escapeSequences: true,
  compact: 3,
  sorted: false,
  getters: false,

  // node only
  maxArrayLength: 100,
  maxStringLength: 10_000, // deno: strAbbreviateSize: 10_000
  customInspect: true,

  // deno only
  /** You can override the quotes preference in inspectString.
   * Used by util.inspect() */
  // TODO(kt3k): Consider using symbol as a key to hide this from the public
  // API.
  quotes: ['"', "'", "`"],
  iterableLimit: 100, // similar to node's maxArrayLength, but doesn't only apply to arrays
  trailingComma: false,

  inspect,

  // TODO(@crowlKats): merge into indentationLvl
  indentLevel: 0,
};

function getDefaultInspectOptions() {
  return {
    budget: {},
    seen: [],
    ...denoInspectDefaultOptions,
  };
}

const DEFAULT_INDENT = "  "; // Default indent string

const STR_ABBREVIATE_SIZE = 10_000;

class CSI {
  static kClear = "\x1b[1;1H";
  static kClearScreenDown = "\x1b[0J";
}

const QUOTE_SYMBOL_REG = new SafeRegExp(/^[a-zA-Z_][a-zA-Z_.0-9]*$/);

function maybeQuoteSymbol(symbol, ctx) {
  const description = SymbolPrototypeGetDescription(symbol);

  if (description === undefined) {
    return SymbolPrototypeToString(symbol);
  }

  if (RegExpPrototypeTest(QUOTE_SYMBOL_REG, description)) {
    return SymbolPrototypeToString(symbol);
  }

  return `Symbol(${quoteString(description, ctx)})`;
}

/** Surround the string in quotes.
 *
 * The quote symbol is chosen by taking the first of the `QUOTES` array which
 * does not occur in the string. If they all occur, settle with `QUOTES[0]`.
 *
 * Insert a backslash before any occurrence of the chosen quote symbol and
 * before any backslash.
 */
function quoteString(string, ctx) {
  const quote = ArrayPrototypeFind(
    ctx.quotes,
    (c) => !StringPrototypeIncludes(string, c),
  ) ??
    ctx.quotes[0];
  const escapePattern = new SafeRegExp(`(?=[${quote}\\\\])`, "g");
  string = StringPrototypeReplace(string, escapePattern, "\\");
  if (ctx.escapeSequences) {
    string = replaceEscapeSequences(string);
  }
  return `${quote}${string}${quote}`;
}

const ESCAPE_PATTERN = new SafeRegExp(/([\b\f\n\r\t\v])/g);
const ESCAPE_MAP = ObjectFreeze({
  "\b": "\\b",
  "\f": "\\f",
  "\n": "\\n",
  "\r": "\\r",
  "\t": "\\t",
  "\v": "\\v",
});

const ESCAPE_PATTERN2 = new SafeRegExp("[\x00-\x1f\x7f-\x9f]", "g");

// Replace escape sequences that can modify output.
function replaceEscapeSequences(string) {
  return StringPrototypeReplace(
    StringPrototypeReplace(
      string,
      ESCAPE_PATTERN,
      (c) => ESCAPE_MAP[c],
    ),
    ESCAPE_PATTERN2,
    (c) =>
      "\\x" +
      StringPrototypePadStart(
        NumberPrototypeToString(StringPrototypeCharCodeAt(c, 0), 16),
        2,
        "0",
      ),
  );
}

// Print strings when they are inside of arrays or objects with quotes
function inspectValueWithQuotes(
  value,
  ctx,
) {
  const abbreviateSize = typeof ctx.strAbbreviateSize === "undefined"
    ? STR_ABBREVIATE_SIZE
    : ctx.strAbbreviateSize;
  switch (typeof value) {
    case "string": {
      const trunc = value.length > abbreviateSize
        ? StringPrototypeSlice(value, 0, abbreviateSize) + "..."
        : value;
      return ctx.stylize(quoteString(trunc, ctx), "string"); // Quoted strings are green
    }
    default:
      return formatValue(ctx, value, 0);
  }
}

const colorKeywords = new SafeMap([
  ["black", "#000000"],
  ["silver", "#c0c0c0"],
  ["gray", "#808080"],
  ["white", "#ffffff"],
  ["maroon", "#800000"],
  ["red", "#ff0000"],
  ["purple", "#800080"],
  ["fuchsia", "#ff00ff"],
  ["green", "#008000"],
  ["lime", "#00ff00"],
  ["olive", "#808000"],
  ["yellow", "#ffff00"],
  ["navy", "#000080"],
  ["blue", "#0000ff"],
  ["teal", "#008080"],
  ["aqua", "#00ffff"],
  ["orange", "#ffa500"],
  ["aliceblue", "#f0f8ff"],
  ["antiquewhite", "#faebd7"],
  ["aquamarine", "#7fffd4"],
  ["azure", "#f0ffff"],
  ["beige", "#f5f5dc"],
  ["bisque", "#ffe4c4"],
  ["blanchedalmond", "#ffebcd"],
  ["blueviolet", "#8a2be2"],
  ["brown", "#a52a2a"],
  ["burlywood", "#deb887"],
  ["cadetblue", "#5f9ea0"],
  ["chartreuse", "#7fff00"],
  ["chocolate", "#d2691e"],
  ["coral", "#ff7f50"],
  ["cornflowerblue", "#6495ed"],
  ["cornsilk", "#fff8dc"],
  ["crimson", "#dc143c"],
  ["cyan", "#00ffff"],
  ["darkblue", "#00008b"],
  ["darkcyan", "#008b8b"],
  ["darkgoldenrod", "#b8860b"],
  ["darkgray", "#a9a9a9"],
  ["darkgreen", "#006400"],
  ["darkgrey", "#a9a9a9"],
  ["darkkhaki", "#bdb76b"],
  ["darkmagenta", "#8b008b"],
  ["darkolivegreen", "#556b2f"],
  ["darkorange", "#ff8c00"],
  ["darkorchid", "#9932cc"],
  ["darkred", "#8b0000"],
  ["darksalmon", "#e9967a"],
  ["darkseagreen", "#8fbc8f"],
  ["darkslateblue", "#483d8b"],
  ["darkslategray", "#2f4f4f"],
  ["darkslategrey", "#2f4f4f"],
  ["darkturquoise", "#00ced1"],
  ["darkviolet", "#9400d3"],
  ["deeppink", "#ff1493"],
  ["deepskyblue", "#00bfff"],
  ["dimgray", "#696969"],
  ["dimgrey", "#696969"],
  ["dodgerblue", "#1e90ff"],
  ["firebrick", "#b22222"],
  ["floralwhite", "#fffaf0"],
  ["forestgreen", "#228b22"],
  ["gainsboro", "#dcdcdc"],
  ["ghostwhite", "#f8f8ff"],
  ["gold", "#ffd700"],
  ["goldenrod", "#daa520"],
  ["greenyellow", "#adff2f"],
  ["grey", "#808080"],
  ["honeydew", "#f0fff0"],
  ["hotpink", "#ff69b4"],
  ["indianred", "#cd5c5c"],
  ["indigo", "#4b0082"],
  ["ivory", "#fffff0"],
  ["khaki", "#f0e68c"],
  ["lavender", "#e6e6fa"],
  ["lavenderblush", "#fff0f5"],
  ["lawngreen", "#7cfc00"],
  ["lemonchiffon", "#fffacd"],
  ["lightblue", "#add8e6"],
  ["lightcoral", "#f08080"],
  ["lightcyan", "#e0ffff"],
  ["lightgoldenrodyellow", "#fafad2"],
  ["lightgray", "#d3d3d3"],
  ["lightgreen", "#90ee90"],
  ["lightgrey", "#d3d3d3"],
  ["lightpink", "#ffb6c1"],
  ["lightsalmon", "#ffa07a"],
  ["lightseagreen", "#20b2aa"],
  ["lightskyblue", "#87cefa"],
  ["lightslategray", "#778899"],
  ["lightslategrey", "#778899"],
  ["lightsteelblue", "#b0c4de"],
  ["lightyellow", "#ffffe0"],
  ["limegreen", "#32cd32"],
  ["linen", "#faf0e6"],
  ["magenta", "#ff00ff"],
  ["mediumaquamarine", "#66cdaa"],
  ["mediumblue", "#0000cd"],
  ["mediumorchid", "#ba55d3"],
  ["mediumpurple", "#9370db"],
  ["mediumseagreen", "#3cb371"],
  ["mediumslateblue", "#7b68ee"],
  ["mediumspringgreen", "#00fa9a"],
  ["mediumturquoise", "#48d1cc"],
  ["mediumvioletred", "#c71585"],
  ["midnightblue", "#191970"],
  ["mintcream", "#f5fffa"],
  ["mistyrose", "#ffe4e1"],
  ["moccasin", "#ffe4b5"],
  ["navajowhite", "#ffdead"],
  ["oldlace", "#fdf5e6"],
  ["olivedrab", "#6b8e23"],
  ["orangered", "#ff4500"],
  ["orchid", "#da70d6"],
  ["palegoldenrod", "#eee8aa"],
  ["palegreen", "#98fb98"],
  ["paleturquoise", "#afeeee"],
  ["palevioletred", "#db7093"],
  ["papayawhip", "#ffefd5"],
  ["peachpuff", "#ffdab9"],
  ["peru", "#cd853f"],
  ["pink", "#ffc0cb"],
  ["plum", "#dda0dd"],
  ["powderblue", "#b0e0e6"],
  ["rosybrown", "#bc8f8f"],
  ["royalblue", "#4169e1"],
  ["saddlebrown", "#8b4513"],
  ["salmon", "#fa8072"],
  ["sandybrown", "#f4a460"],
  ["seagreen", "#2e8b57"],
  ["seashell", "#fff5ee"],
  ["sienna", "#a0522d"],
  ["skyblue", "#87ceeb"],
  ["slateblue", "#6a5acd"],
  ["slategray", "#708090"],
  ["slategrey", "#708090"],
  ["snow", "#fffafa"],
  ["springgreen", "#00ff7f"],
  ["steelblue", "#4682b4"],
  ["tan", "#d2b48c"],
  ["thistle", "#d8bfd8"],
  ["tomato", "#ff6347"],
  ["turquoise", "#40e0d0"],
  ["violet", "#ee82ee"],
  ["wheat", "#f5deb3"],
  ["whitesmoke", "#f5f5f5"],
  ["yellowgreen", "#9acd32"],
  ["rebeccapurple", "#663399"],
]);

const HASH_PATTERN = new SafeRegExp(
  /^#([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})?$/,
);
const SMALL_HASH_PATTERN = new SafeRegExp(
  /^#([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])?$/,
);
const RGB_PATTERN = new SafeRegExp(
  /^rgba?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/,
);
const HSL_PATTERN = new SafeRegExp(
  /^hsla?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)%\s*,\s*([+\-]?\d*\.?\d+)%\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/,
);

function parseCssColor(colorString) {
  if (colorKeywords.has(colorString)) {
    colorString = colorKeywords.get(colorString);
  }
  // deno-fmt-ignore
  const hashMatch = StringPrototypeMatch(colorString, HASH_PATTERN);
  if (hashMatch != null) {
    return [
      NumberParseInt(hashMatch[1], 16),
      NumberParseInt(hashMatch[2], 16),
      NumberParseInt(hashMatch[3], 16),
    ];
  }
  // deno-fmt-ignore
  const smallHashMatch = StringPrototypeMatch(colorString, SMALL_HASH_PATTERN);
  if (smallHashMatch != null) {
    return [
      NumberParseInt(`${smallHashMatch[1]}${smallHashMatch[1]}`, 16),
      NumberParseInt(`${smallHashMatch[2]}${smallHashMatch[2]}`, 16),
      NumberParseInt(`${smallHashMatch[3]}${smallHashMatch[3]}`, 16),
    ];
  }
  // deno-fmt-ignore
  const rgbMatch = StringPrototypeMatch(colorString, RGB_PATTERN);
  if (rgbMatch != null) {
    return [
      MathRound(MathMax(0, MathMin(255, rgbMatch[1]))),
      MathRound(MathMax(0, MathMin(255, rgbMatch[2]))),
      MathRound(MathMax(0, MathMin(255, rgbMatch[3]))),
    ];
  }
  // deno-fmt-ignore
  const hslMatch = StringPrototypeMatch(colorString, HSL_PATTERN);
  if (hslMatch != null) {
    // https://www.rapidtables.com/convert/color/hsl-to-rgb.html
    let h = Number(hslMatch[1]) % 360;
    if (h < 0) {
      h += 360;
    }
    const s = MathMax(0, MathMin(100, hslMatch[2])) / 100;
    const l = MathMax(0, MathMin(100, hslMatch[3])) / 100;
    const c = (1 - MathAbs(2 * l - 1)) * s;
    const x = c * (1 - MathAbs((h / 60) % 2 - 1));
    const m = l - c / 2;
    let r_;
    let g_;
    let b_;
    if (h < 60) {
      ({ 0: r_, 1: g_, 2: b_ } = [c, x, 0]);
    } else if (h < 120) {
      ({ 0: r_, 1: g_, 2: b_ } = [x, c, 0]);
    } else if (h < 180) {
      ({ 0: r_, 1: g_, 2: b_ } = [0, c, x]);
    } else if (h < 240) {
      ({ 0: r_, 1: g_, 2: b_ } = [0, x, c]);
    } else if (h < 300) {
      ({ 0: r_, 1: g_, 2: b_ } = [x, 0, c]);
    } else {
      ({ 0: r_, 1: g_, 2: b_ } = [c, 0, x]);
    }
    return [
      MathRound((r_ + m) * 255),
      MathRound((g_ + m) * 255),
      MathRound((b_ + m) * 255),
    ];
  }
  return null;
}

function getDefaultCss() {
  return {
    backgroundColor: null,
    color: null,
    fontWeight: null,
    fontStyle: null,
    textDecorationColor: null,
    textDecorationLine: [],
  };
}

const SPACE_PATTERN = new SafeRegExp(/\s+/g);

function parseCss(cssString) {
  const css = getDefaultCss();

  const rawEntries = [];
  let inValue = false;
  let currentKey = null;
  let parenthesesDepth = 0;
  let currentPart = "";
  for (let i = 0; i < cssString.length; i++) {
    const c = cssString[i];
    if (c == "(") {
      parenthesesDepth++;
    } else if (parenthesesDepth > 0) {
      if (c == ")") {
        parenthesesDepth--;
      }
    } else if (inValue) {
      if (c == ";") {
        const value = StringPrototypeTrim(currentPart);
        if (value != "") {
          ArrayPrototypePush(rawEntries, [currentKey, value]);
        }
        currentKey = null;
        currentPart = "";
        inValue = false;
        continue;
      }
    } else if (c == ":") {
      currentKey = StringPrototypeTrim(currentPart);
      currentPart = "";
      inValue = true;
      continue;
    }
    currentPart += c;
  }
  if (inValue && parenthesesDepth == 0) {
    const value = StringPrototypeTrim(currentPart);
    if (value != "") {
      ArrayPrototypePush(rawEntries, [currentKey, value]);
    }
    currentKey = null;
    currentPart = "";
  }

  for (let i = 0; i < rawEntries.length; ++i) {
    const { 0: key, 1: value } = rawEntries[i];
    if (key == "background-color") {
      if (value != null) {
        css.backgroundColor = value;
      }
    } else if (key == "color") {
      if (value != null) {
        css.color = value;
      }
    } else if (key == "font-weight") {
      if (value == "bold") {
        css.fontWeight = value;
      }
    } else if (key == "font-style") {
      if (
        ArrayPrototypeIncludes(["italic", "oblique", "oblique 14deg"], value)
      ) {
        css.fontStyle = "italic";
      }
    } else if (key == "text-decoration-line") {
      css.textDecorationLine = [];
      const lineTypes = StringPrototypeSplit(value, SPACE_PATTERN);
      for (let i = 0; i < lineTypes.length; ++i) {
        const lineType = lineTypes[i];
        if (
          ArrayPrototypeIncludes(
            ["line-through", "overline", "underline"],
            lineType,
          )
        ) {
          ArrayPrototypePush(css.textDecorationLine, lineType);
        }
      }
    } else if (key == "text-decoration-color") {
      const color = parseCssColor(value);
      if (color != null) {
        css.textDecorationColor = color;
      }
    } else if (key == "text-decoration") {
      css.textDecorationColor = null;
      css.textDecorationLine = [];
      const args = StringPrototypeSplit(value, SPACE_PATTERN);
      for (let i = 0; i < args.length; ++i) {
        const arg = args[i];
        const maybeColor = parseCssColor(arg);
        if (maybeColor != null) {
          css.textDecorationColor = maybeColor;
        } else if (
          ArrayPrototypeIncludes(
            ["line-through", "overline", "underline"],
            arg,
          )
        ) {
          ArrayPrototypePush(css.textDecorationLine, arg);
        }
      }
    }
  }

  return css;
}

function colorEquals(color1, color2) {
  return color1?.[0] == color2?.[0] && color1?.[1] == color2?.[1] &&
    color1?.[2] == color2?.[2];
}

function cssToAnsi(css, prevCss = null) {
  prevCss = prevCss ?? getDefaultCss();
  let ansi = "";
  if (!colorEquals(css.backgroundColor, prevCss.backgroundColor)) {
    if (css.backgroundColor == null) {
      ansi += "\x1b[49m";
    } else if (css.backgroundColor == "black") {
      ansi += `\x1b[40m`;
    } else if (css.backgroundColor == "red") {
      ansi += `\x1b[41m`;
    } else if (css.backgroundColor == "green") {
      ansi += `\x1b[42m`;
    } else if (css.backgroundColor == "yellow") {
      ansi += `\x1b[43m`;
    } else if (css.backgroundColor == "blue") {
      ansi += `\x1b[44m`;
    } else if (css.backgroundColor == "magenta") {
      ansi += `\x1b[45m`;
    } else if (css.backgroundColor == "cyan") {
      ansi += `\x1b[46m`;
    } else if (css.backgroundColor == "white") {
      ansi += `\x1b[47m`;
    } else {
      if (ArrayIsArray(css.backgroundColor)) {
        const { 0: r, 1: g, 2: b } = css.backgroundColor;
        ansi += `\x1b[48;2;${r};${g};${b}m`;
      } else {
        const parsed = parseCssColor(css.backgroundColor);
        if (parsed !== null) {
          const { 0: r, 1: g, 2: b } = parsed;
          ansi += `\x1b[48;2;${r};${g};${b}m`;
        } else {
          ansi += "\x1b[49m";
        }
      }
    }
  }
  if (!colorEquals(css.color, prevCss.color)) {
    if (css.color == null) {
      ansi += "\x1b[39m";
    } else if (css.color == "black") {
      ansi += `\x1b[30m`;
    } else if (css.color == "red") {
      ansi += `\x1b[31m`;
    } else if (css.color == "green") {
      ansi += `\x1b[32m`;
    } else if (css.color == "yellow") {
      ansi += `\x1b[33m`;
    } else if (css.color == "blue") {
      ansi += `\x1b[34m`;
    } else if (css.color == "magenta") {
      ansi += `\x1b[35m`;
    } else if (css.color == "cyan") {
      ansi += `\x1b[36m`;
    } else if (css.color == "white") {
      ansi += `\x1b[37m`;
    } else {
      if (ArrayIsArray(css.color)) {
        const { 0: r, 1: g, 2: b } = css.color;
        ansi += `\x1b[38;2;${r};${g};${b}m`;
      } else {
        const parsed = parseCssColor(css.color);
        if (parsed !== null) {
          const { 0: r, 1: g, 2: b } = parsed;
          ansi += `\x1b[38;2;${r};${g};${b}m`;
        } else {
          ansi += "\x1b[39m";
        }
      }
    }
  }
  if (css.fontWeight != prevCss.fontWeight) {
    if (css.fontWeight == "bold") {
      ansi += `\x1b[1m`;
    } else {
      ansi += "\x1b[22m";
    }
  }
  if (css.fontStyle != prevCss.fontStyle) {
    if (css.fontStyle == "italic") {
      ansi += `\x1b[3m`;
    } else {
      ansi += "\x1b[23m";
    }
  }
  if (!colorEquals(css.textDecorationColor, prevCss.textDecorationColor)) {
    if (css.textDecorationColor != null) {
      const { 0: r, 1: g, 2: b } = css.textDecorationColor;
      ansi += `\x1b[58;2;${r};${g};${b}m`;
    } else {
      ansi += "\x1b[59m";
    }
  }
  if (
    ArrayPrototypeIncludes(css.textDecorationLine, "line-through") !=
      ArrayPrototypeIncludes(prevCss.textDecorationLine, "line-through")
  ) {
    if (ArrayPrototypeIncludes(css.textDecorationLine, "line-through")) {
      ansi += "\x1b[9m";
    } else {
      ansi += "\x1b[29m";
    }
  }
  if (
    ArrayPrototypeIncludes(css.textDecorationLine, "overline") !=
      ArrayPrototypeIncludes(prevCss.textDecorationLine, "overline")
  ) {
    if (ArrayPrototypeIncludes(css.textDecorationLine, "overline")) {
      ansi += "\x1b[53m";
    } else {
      ansi += "\x1b[55m";
    }
  }
  if (
    ArrayPrototypeIncludes(css.textDecorationLine, "underline") !=
      ArrayPrototypeIncludes(prevCss.textDecorationLine, "underline")
  ) {
    if (ArrayPrototypeIncludes(css.textDecorationLine, "underline")) {
      ansi += "\x1b[4m";
    } else {
      ansi += "\x1b[24m";
    }
  }
  return ansi;
}

function inspectArgs(args, inspectOptions = { __proto__: null }) {
  const ctx = {
    ...getDefaultInspectOptions(),
    colors: inspectOptions.colors ?? !noColorStdout(),
    ...inspectOptions,
  };
  if (inspectOptions.iterableLimit !== undefined) {
    ctx.maxArrayLength = inspectOptions.iterableLimit;
  }
  if (inspectOptions.strAbbreviateSize !== undefined) {
    ctx.maxStringLength = inspectOptions.strAbbreviateSize;
  }
  if (ctx.colors) ctx.stylize = createStylizeWithColor(styles, colors);
  if (ctx.maxArrayLength === null) ctx.maxArrayLength = Infinity;
  if (ctx.maxStringLength === null) ctx.maxStringLength = Infinity;

  const noColor = !ctx.colors;
  const first = args[0];
  let a = 0;
  let string = "";

  if (typeof first == "string" && args.length > 1) {
    a++;
    // Index of the first not-yet-appended character. Use this so we only
    // have to append to `string` when a substitution occurs / at the end.
    let appendedChars = 0;
    let usedStyle = false;
    let prevCss = null;
    for (let i = 0; i < first.length - 1; i++) {
      if (first[i] == "%") {
        const char = first[++i];
        if (a < args.length) {
          let formattedArg = null;
          if (char == "s") {
            // Format as a string.
            formattedArg = String(args[a++]);
          } else if (ArrayPrototypeIncludes(["d", "i"], char)) {
            // Format as an integer.
            const value = args[a++];
            if (typeof value == "bigint") {
              formattedArg = `${value}n`;
            } else if (typeof value == "number") {
              formattedArg = `${NumberParseInt(String(value))}`;
            } else {
              formattedArg = "NaN";
            }
          } else if (char == "f") {
            // Format as a floating point value.
            const value = args[a++];
            if (typeof value == "number") {
              formattedArg = `${value}`;
            } else {
              formattedArg = "NaN";
            }
          } else if (ArrayPrototypeIncludes(["O", "o"], char)) {
            // Format as an object.
            formattedArg = formatValue(ctx, args[a++], 0);
          } else if (char == "c") {
            const value = args[a++];
            if (!noColor) {
              const css = parseCss(value);
              formattedArg = cssToAnsi(css, prevCss);
              if (formattedArg != "") {
                usedStyle = true;
                prevCss = css;
              }
            } else {
              formattedArg = "";
            }
          }

          if (formattedArg != null) {
            string += StringPrototypeSlice(first, appendedChars, i - 1) +
              formattedArg;
            appendedChars = i + 1;
          }
        }
        if (char == "%") {
          string += StringPrototypeSlice(first, appendedChars, i - 1) + "%";
          appendedChars = i + 1;
        }
      }
    }
    string += StringPrototypeSlice(first, appendedChars);
    if (usedStyle) {
      string += "\x1b[0m";
    }
  }

  for (; a < args.length; a++) {
    if (a > 0) {
      string += " ";
    }
    if (typeof args[a] == "string") {
      string += args[a];
    } else {
      // Use default maximum depth for null or undefined arguments.
      string += formatValue(ctx, args[a], 0);
    }
  }

  if (ctx.indentLevel > 0) {
    const groupIndent = StringPrototypeRepeat(
      DEFAULT_INDENT,
      ctx.indentLevel,
    );
    string = groupIndent +
      StringPrototypeReplaceAll(string, "\n", `\n${groupIndent}`);
  }

  return string;
}

function createStylizeWithColor(styles, colors) {
  return function stylizeWithColor(str, styleType) {
    const style = styles[styleType];
    if (style !== undefined) {
      const color = colors[style];
      if (color !== undefined) {
        return `\u001b[${color[0]}m${str}\u001b[${color[1]}m`;
      }
    }
    return str;
  };
}

const countMap = new SafeMap();
const timerMap = new SafeMap();
const isConsoleInstance = Symbol("isConsoleInstance");

/** @param noColor {boolean} */
function getConsoleInspectOptions(noColor) {
  return {
    ...getDefaultInspectOptions(),
    colors: !noColor,
    stylize: noColor ? stylizeNoColor : createStylizeWithColor(styles, colors),
  };
}

class Console {
  #printFunc = null;
  [isConsoleInstance] = false;

  constructor(printFunc) {
    this.#printFunc = printFunc;
    this.indentLevel = 0;
    this[isConsoleInstance] = true;

    // ref https://console.spec.whatwg.org/#console-namespace
    // For historical web-compatibility reasons, the namespace object for
    // console must have as its [[Prototype]] an empty object, created as if
    // by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
    const console = ObjectCreate({}, {
      [SymbolToStringTag]: {
        enumerable: false,
        writable: false,
        configurable: true,
        value: "console",
      },
    });
    ObjectAssign(console, this);
    return console;
  }

  log = (...args) => {
    this.#printFunc(
      inspectArgs(args, {
        ...getConsoleInspectOptions(noColorStdout()),
        indentLevel: this.indentLevel,
      }) + "\n",
      1,
    );
  };

  debug = (...args) => {
    this.#printFunc(
      inspectArgs(args, {
        ...getConsoleInspectOptions(noColorStdout()),
        indentLevel: this.indentLevel,
      }) + "\n",
      0,
    );
  };

  info = (...args) => {
    this.#printFunc(
      inspectArgs(args, {
        ...getConsoleInspectOptions(noColorStdout()),
        indentLevel: this.indentLevel,
      }) + "\n",
      1,
    );
  };

  dir = (obj = undefined, options = { __proto__: null }) => {
    this.#printFunc(
      inspectArgs([obj], {
        ...getConsoleInspectOptions(noColorStdout()),
        ...options,
      }) + "\n",
      1,
    );
  };

  dirxml = this.dir;

  warn = (...args) => {
    this.#printFunc(
      inspectArgs(args, {
        ...getConsoleInspectOptions(noColorStderr()),
        indentLevel: this.indentLevel,
      }) + "\n",
      2,
    );
  };

  error = (...args) => {
    this.#printFunc(
      inspectArgs(args, {
        ...getConsoleInspectOptions(noColorStderr()),
        indentLevel: this.indentLevel,
      }) + "\n",
      3,
    );
  };

  assert = (condition = false, ...args) => {
    if (condition) {
      return;
    }

    if (args.length === 0) {
      this.error("Assertion failed");
      return;
    }

    const [first, ...rest] = new SafeArrayIterator(args);

    if (typeof first === "string") {
      this.error(
        `Assertion failed: ${first}`,
        ...new SafeArrayIterator(rest),
      );
      return;
    }

    this.error(`Assertion failed:`, ...new SafeArrayIterator(args));
  };

  count = (label = "default") => {
    label = String(label);

    if (MapPrototypeHas(countMap, label)) {
      const current = MapPrototypeGet(countMap, label) || 0;
      MapPrototypeSet(countMap, label, current + 1);
    } else {
      MapPrototypeSet(countMap, label, 1);
    }

    this.info(`${label}: ${MapPrototypeGet(countMap, label)}`);
  };

  countReset = (label = "default") => {
    label = String(label);

    if (MapPrototypeHas(countMap, label)) {
      MapPrototypeSet(countMap, label, 0);
    } else {
      this.warn(`Count for '${label}' does not exist`);
    }
  };

  table = (data = undefined, properties = undefined) => {
    if (properties !== undefined && !ArrayIsArray(properties)) {
      throw new Error(
        "The 'properties' argument must be of type Array: " +
          "received type " + typeof properties,
      );
    }

    if (data === null || typeof data !== "object") {
      return this.log(data);
    }

    const stringifyValue = (value) =>
      inspectValueWithQuotes(value, {
        ...getDefaultInspectOptions(),
        depth: 1,
        compact: true,
      });
    const toTable = (header, body) => this.log(cliTable(header, body));

    let resultData;
    const isSetObject = isSet(data);
    const isMapObject = isMap(data);
    const valuesKey = "Values";
    const indexKey = isSetObject || isMapObject ? "(iter idx)" : "(idx)";

    if (isSetObject) {
      resultData = [...new SafeSetIterator(data)];
    } else if (isMapObject) {
      let idx = 0;
      resultData = { __proto__: null };

      MapPrototypeForEach(data, (v, k) => {
        resultData[idx] = { Key: k, Values: v };
        idx++;
      });
    } else {
      resultData = data;
    }

    const keys = ObjectKeys(resultData);
    const numRows = keys.length;

    const objectValues = properties
      ? ObjectFromEntries(
        ArrayPrototypeMap(
          properties,
          (name) => [name, ArrayPrototypeFill(new Array(numRows), "")],
        ),
      )
      : {};
    const indexKeys = [];
    const values = [];

    let hasPrimitives = false;
    ArrayPrototypeForEach(keys, (k, idx) => {
      const value = resultData[k];
      const primitive = value === null ||
        (typeof value !== "function" && typeof value !== "object");
      if (properties === undefined && primitive) {
        hasPrimitives = true;
        ArrayPrototypePush(values, stringifyValue(value));
      } else {
        const valueObj = value || {};
        const keys = properties || ObjectKeys(valueObj);
        for (let i = 0; i < keys.length; ++i) {
          const k = keys[i];
          if (!primitive && ReflectHas(valueObj, k)) {
            if (!(ReflectHas(objectValues, k))) {
              objectValues[k] = ArrayPrototypeFill(new Array(numRows), "");
            }
            objectValues[k][idx] = stringifyValue(valueObj[k]);
          }
        }
        ArrayPrototypePush(values, "");
      }

      ArrayPrototypePush(indexKeys, k);
    });

    const headerKeys = ObjectKeys(objectValues);
    const bodyValues = ObjectValues(objectValues);
    const headerProps = properties ||
      [
        ...new SafeArrayIterator(headerKeys),
        !isMapObject && hasPrimitives && valuesKey,
      ];
    const header = ArrayPrototypeFilter([
      indexKey,
      ...new SafeArrayIterator(headerProps),
    ], Boolean);
    const body = [indexKeys, ...new SafeArrayIterator(bodyValues), values];

    toTable(header, body);
  };

  time = (label = "default") => {
    label = String(label);

    if (MapPrototypeHas(timerMap, label)) {
      this.warn(`Timer '${label}' already exists`);
      return;
    }

    MapPrototypeSet(timerMap, label, currentTime());
  };

  timeLog = (label = "default", ...args) => {
    label = String(label);

    if (!MapPrototypeHas(timerMap, label)) {
      this.warn(`Timer '${label}' does not exist`);
      return;
    }

    const startTime = MapPrototypeGet(timerMap, label);
    let duration = currentTime() - startTime;
    if (duration < 1) {
      duration = NumberPrototypeToFixed(duration, 3);
    } else if (duration < 10) {
      duration = NumberPrototypeToFixed(duration, 2);
    } else if (duration < 100) {
      duration = NumberPrototypeToFixed(duration, 1);
    } else {
      duration = NumberPrototypeToFixed(duration, 0);
    }

    this.info(`${label}: ${duration}ms`, ...new SafeArrayIterator(args));
  };

  timeEnd = (label = "default") => {
    label = String(label);

    if (!MapPrototypeHas(timerMap, label)) {
      this.warn(`Timer '${label}' does not exist`);
      return;
    }

    const startTime = MapPrototypeGet(timerMap, label);
    MapPrototypeDelete(timerMap, label);
    let duration = currentTime() - startTime;
    if (duration < 1) {
      duration = NumberPrototypeToFixed(duration, 3);
    } else if (duration < 10) {
      duration = NumberPrototypeToFixed(duration, 2);
    } else if (duration < 100) {
      duration = NumberPrototypeToFixed(duration, 1);
    } else {
      duration = NumberPrototypeToFixed(duration, 0);
    }

    this.info(`${label}: ${duration}ms`);
  };

  group = (...label) => {
    if (label.length > 0) {
      this.log(...new SafeArrayIterator(label));
    }
    this.indentLevel += 2;
  };

  groupCollapsed = this.group;

  groupEnd = () => {
    if (this.indentLevel > 0) {
      this.indentLevel -= 2;
    }
  };

  clear = () => {
    this.indentLevel = 0;
    this.#printFunc(CSI.kClear, 1);
    this.#printFunc(CSI.kClearScreenDown, 1);
  };

  trace = (...args) => {
    const message = inspectArgs(
      args,
      {
        ...getConsoleInspectOptions(noColorStderr()),
        indentLevel: 0,
      },
    );
    const err = {
      name: "Trace",
      message,
    };
    ErrorCaptureStackTrace(err, this.trace);
    this.error(err.stack);
  };

  // These methods are noops, but when the inspector is connected, they
  // call into V8.
  profile = (_label) => {};
  profileEnd = (_label) => {};
  timeStamp = (_label) => {};

  static [SymbolHasInstance](instance) {
    return instance[isConsoleInstance];
  }
}

const customInspect = SymbolFor("Deno.customInspect");

function inspect(
  value,
  inspectOptions = { __proto__: null },
) {
  // Default options
  const ctx = {
    ...getDefaultInspectOptions(),
    ...inspectOptions,
  };
  if (inspectOptions.iterableLimit !== undefined) {
    ctx.maxArrayLength = inspectOptions.iterableLimit;
  }
  if (inspectOptions.strAbbreviateSize !== undefined) {
    ctx.maxStringLength = inspectOptions.strAbbreviateSize;
  }

  if (ctx.colors) ctx.stylize = createStylizeWithColor(styles, colors);
  if (ctx.maxArrayLength === null) ctx.maxArrayLength = Infinity;
  if (ctx.maxStringLength === null) ctx.maxStringLength = Infinity;
  return formatValue(ctx, value, 0);
}

/** Creates a proxy that represents a subset of the properties
 * of the original object optionally without evaluating the properties
 * in order to get the values. */
function createFilteredInspectProxy({ object, keys, evaluate }) {
  const obj = class {};
  if (object.constructor?.name) {
    ObjectDefineProperty(obj, "name", {
      __proto__: null,
      value: object.constructor.name,
    });
  }

  return new Proxy(new obj(), {
    get(_target, key) {
      if (key === SymbolToStringTag) {
        return object.constructor?.name;
      } else if (ArrayPrototypeIncludes(keys, key)) {
        return ReflectGet(object, key);
      } else {
        return undefined;
      }
    },
    getOwnPropertyDescriptor(_target, key) {
      if (!ArrayPrototypeIncludes(keys, key)) {
        return undefined;
      } else if (evaluate) {
        return getEvaluatedDescriptor(object, key);
      } else {
        return getDescendantPropertyDescriptor(object, key) ??
          getEvaluatedDescriptor(object, key);
      }
    },
    has(_target, key) {
      return ArrayPrototypeIncludes(keys, key);
    },
    ownKeys() {
      return keys;
    },
  });

  function getDescendantPropertyDescriptor(object, key) {
    let propertyDescriptor = ReflectGetOwnPropertyDescriptor(object, key);
    if (!propertyDescriptor) {
      const prototype = ReflectGetPrototypeOf(object);
      if (prototype) {
        propertyDescriptor = getDescendantPropertyDescriptor(prototype, key);
      }
    }
    return propertyDescriptor;
  }

  function getEvaluatedDescriptor(object, key) {
    return {
      configurable: true,
      enumerable: true,
      value: object[key],
    };
  }
}

// Expose these fields to internalObject for tests.
internals.Console = Console;
internals.cssToAnsi = cssToAnsi;
internals.inspectArgs = inspectArgs;
internals.parseCss = parseCss;
internals.parseCssColor = parseCssColor;

export {
  colors,
  Console,
  createFilteredInspectProxy,
  createStylizeWithColor,
  CSI,
  customInspect,
  formatBigInt,
  formatNumber,
  formatValue,
  getDefaultInspectOptions,
  getStderrNoColor,
  getStdoutNoColor,
  inspect,
  inspectArgs,
  quoteString,
  setNoColorFns,
  styles,
};
