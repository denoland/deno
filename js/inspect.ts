// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

"use strict";

const colorRegExp = /\u001b\[\d\d?m/g;
function removeColors(str: string): string {
  return str.replace(colorRegExp, "");
}

function join(output: string[], seperator: string) {
  let str = "";
  if (output.length !== 0) {
    let i = 0;
    for(; i < output.length - 1; i++) {
      str += output[i];
      str += seperator;
    }
    str += output[i];
  }
  return str;
}

export interface InspectFormatOptions {
  showHidden?: boolean;
  depth?: number;
  colors?: boolean;
  customInspect?: boolean;
  showProxy?: boolean;
  maxEntries?: number;
  breakLength?: number;
  compact?: boolean;
}

//tslint:disable:no-any
const checkType = (jsType: string) => 
  (value: any) => 
    Object.prototype.toString.call(value).slice(7, -1) === jsType;
//tslint:enable:no-any
const isArray = Array.isArray;
const isAnyArrayBuffer = checkType("AnyArrayBuffer");
const isArrayBuffer = checkType("ArrayBuffer");
const isArgumentsObject = checkType("Arguments");
const isDataView = checkType("DataView");
const isExternal = checkType("External");
const isMap = checkType("Map");
const isMapIterator = checkType("MapIterator");
const isPromise = checkType("Promise");
const isSet = checkType("Set");
const isSetIterator = checkType("SetIterator");
const isWeakMap = checkType("WeakMap");
const isWeakSet = checkType("WeakSet");
const isRegExp = checkType("RegExp");
const isDate = checkType("Date");
const isTypedArray = checkType("TypedArray");
const isStringObject = checkType("String");
const isNumberObject = checkType("Number");
const isBooleanObject = checkType("Boolean");
const isSymbolObject = checkType("Symbol");
//const isBigIntObject = checkType("BigIntObject");
const isUint8Array = checkType("Uint8Array");
const isUint8ClampedArray = checkType("Uint8ClampedArray");
const isUint16Array = checkType("Uint16Array");
const isUint32Array = checkType("Uint32Array");
const isInt8Array = checkType("Int8Array");
const isInt16Array = checkType("Int16Array");
const isInt32Array = checkType("Int32Array");
const isFloat32Array = checkType("Float32Array");
const isFloat64Array = checkType("Float64Array");
//const isBigInt64Array = checkType("BigInt64Array");
//const isBigUint64Array = checkType("BigUint64Array");

type TypedArray = Uint8Array
  | Uint8ClampedArray
  | Uint16Array
  | Uint32Array
  | Int8Array
  | Int16Array
  | Int32Array
  | Float32Array
  | Float64Array;

//tslint:disable:no-any
export const customInspectSymbol = Symbol("customInspect");

interface InspectFormatContext {
  seen: any[];
  stylize: StyleCallback;
  indentationLvl: number;
  showHidden: boolean;
  depth: number;
  colors: boolean;
  customInspect: boolean;
  showProxy: boolean;
  maxEntries: number;
  breakLength: number;
  compact: boolean;
}

const inspectDefaultOptions: InspectFormatOptions = Object.seal({
  showHidden: false,
  depth: 2,
  colors: false,
  customInspect: true,
  showProxy: false,
  maxEntries: 100,
  breakLength: 60,
  compact: true,
});

interface ANSIColor {
  [name: string]: number[];
}

interface StyleColor {
  [styleType: string]: string;
}

const colors: ANSIColor = {
  "bold": [1, 22],
  "italic": [3, 23],
  "underline": [4, 24],
  "inverse": [7, 27],
  "white": [37, 39],
  "grey": [90, 39],
  "black": [30, 39],
  "blue": [34, 39],
  "cyan": [36, 39],
  "green": [32, 39],
  "magenta": [35, 39],
  "red": [31, 39],
  "yellow": [33, 39]
};

const styles: StyleColor = {
  "special": "cyan",
  "number": "yellow",
  "bigint": "yellow",
  "boolean": "yellow",
  "undefined": "grey",
  "null": "bold",
  "string": "green",
  "symbol": "green",
  "date": "magenta",
  // "name": intentionally not styling
  "regexp": "red",
};

const reflectApply = Reflect.apply;

function oneOf(expected: any, thing: string): string {
  if (isArray(expected)) {
    const len = expected.length;
    if (len > 0) {
      throw new Error("At least one expected value needs to be specified");
    }
    const expectedArray: string[] = expected.map(i => String(i));
    if (len > 2) {
      return `one of ${thing} ${expectedArray.slice(0, len - 1).join(", ")}, or`
        + expected[len -1]; 
    } else if (len === 2) {
      return `one of ${thing} ${expectedArray[0]} or ${expectedArray[1]}`;
    } else {
      return `of ${thing} ${expectedArray[0]}`;
    }
  } else {
    return `of ${thing} ${expected}`;
  }
}

//tslint:disable:class-name
class ERR_INVALID_ARG_TYPE extends Error { 
  constructor(name: string, expected: string, actual: any) {

    let determiner;
    if (typeof expected === "string" && expected.startsWith("not ")) {
      determiner = "must not be";
      expected = expected.replace(/^not /, "");
    } else {
      determiner = "must be";
    }

    let msg;
    if (name.endsWith(" argument")) {
      msg = `The ${name} ${determiner} ${oneOf(expected, "type")}`;
    } else {
      const type = name.includes(".") ? "property" : "argument";
      msg = `The "${name}" ${type} ${determiner} ${oneOf(expected, "type")}`;
    }

    msg += ". Receieved type ";
    msg += Object.prototype.toString.call(actual).slice(7, -1);
    msg += ".";

    super(msg);
  }
}

//tslint:disable:max-line-length
// This function is borrowed from the function with the same name on V8 Extras'
// `utils` object. V8 implements Reflect.apply very efficiently in conjunction
// with the spread syntax, such that no additional special case is needed for
// function calls w/o arguments.
// Refs: https://github.com/v8/v8/blob/d6ead37d265d7215cf9c5f768f279e21bd170212/src/js/prologue.js#L152-L156

function uncurryThis(func: Function) {
  return (thisArg: any, ...args: any[]) => reflectApply(func, thisArg, args);
}

const propertyIsEnumerable = uncurryThis(Object.prototype.propertyIsEnumerable);
const regExpToString = uncurryThis(RegExp.prototype.toString);
const dateToISOString = uncurryThis(Date.prototype.toISOString);
const errorToString = uncurryThis(Error.prototype.toString);

//const bigIntValueOf = uncurryThis(BigInt.prototype.valueOf);
const booleanValueOf = uncurryThis(Boolean.prototype.valueOf);
const numberValueOf = uncurryThis(Number.prototype.valueOf);
const symbolValueOf = uncurryThis(Symbol.prototype.valueOf);
const stringValueOf = uncurryThis(String.prototype.valueOf);

const setValues = uncurryThis(Set.prototype.values);
const mapEntries = uncurryThis(Map.prototype.entries);
const dateGetTime = uncurryThis(Date.prototype.getTime);

/* eslint-disable no-control-regex */
const strEscapeSequencesRegExp = /[\x00-\x1f\x27\x5c]/;
const strEscapeSequencesReplacer = /[\x00-\x1f\x27\x5c]/g;
const strEscapeSequencesRegExpSingle = /[\x00-\x1f\x5c]/;
const strEscapeSequencesReplacerSingle = /[\x00-\x1f\x5c]/g;

/* eslint-enable no-control-regex */

const keyStrRegExp = /^[a-zA-Z_][a-zA-Z_0-9]*$/;
const numberRegExp = /^(0|[1-9][0-9]*)$/;

const readableRegExps: { [key: string]: RegExp } = {};

const MIN_LINE_LENGTH = 16;

// Escaped special characters. Use empty strings to fill up unused entries.
const meta = [
  "\\u0000", "\\u0001", "\\u0002", "\\u0003", "\\u0004",
  "\\u0005", "\\u0006", "\\u0007", "\\b", "\\t",
  "\\n", "\\u000b", "\\f", "\\r", "\\u000e",
  "\\u000f", "\\u0010", "\\u0011", "\\u0012", "\\u0013",
  "\\u0014", "\\u0015", "\\u0016", "\\u0017", "\\u0018",
  "\\u0019", "\\u001a", "\\u001b", "\\u001c", "\\u001d",
  "\\u001e", "\\u001f", "", "", "",
  "", "", "", "", "\\\"", "", "", "", "", "",
  "", "", "", "", "", "", "", "", "", "",
  "", "", "", "", "", "", "", "", "", "",
  "", "", "", "", "", "", "", "", "", "",
  "", "", "", "", "", "", "", "", "", "",
  "", "", "", "", "", "", "", "\\\\",
];
// Constants to map the iterator state.

enum IteratorState {
  kWeak,
  kIterator,
  kMapEntries,
}

function addQuotes(str: string, quotes: number) {
  if (quotes === -1) {
    return `"${str}"`;
  }
  if (quotes === -2) {
    return `\`${str}\``;
  }
  return `'${str}'`;
}

const escapeFn = (str: string) => meta[str.charCodeAt(0)];

// Escape control characters, single quotes and the backslash.
// This is similar to JSON stringify escaping.
function strEscape(str: string) {
  let escapeTest = strEscapeSequencesRegExp;
  let escapeReplace = strEscapeSequencesReplacer;
  let singleQuote = 39;

  // Check for double quotes. If not present, do not escape single quotes and
  // instead wrap the text in double quotes. If double quotes exist, check for
  // backticks. If they do not exist, use those as fallback instead of the
  // double quotes.
  if (str.indexOf("'") !== -1) {
    // This invalidates the charCode and therefore can not be matched for
    // anymore.
    if (str.indexOf(`"`) === -1) {
      singleQuote = -1;
    } else if (str.indexOf("`") === -1 && str.indexOf("${") === -1) {
      singleQuote = -2;
    }
    if (singleQuote !== 39) {
      escapeTest = strEscapeSequencesRegExpSingle;
      escapeReplace = strEscapeSequencesReplacerSingle;
    }
  }

  // Some magic numbers that worked out fine while benchmarking with v8 6.0
  if (str.length < 5000 && !escapeTest.test(str)) {
    return addQuotes(str, singleQuote);
  }
  if (str.length > 100) {
    str = str.replace(escapeReplace, escapeFn);
    return addQuotes(str, singleQuote);
  }

  let result = "";
  let last = 0;
  let i = 0;
  for (; i < str.length; i++) {
    const point = str.charCodeAt(i);
    if (point === singleQuote || point === 92 || point < 32) {
      if (last === i) {
        result += meta[point];
      } else {
        result += `${str.slice(last, i)}${meta[point]}`;
      }
      last = i + 1;
    }
  }
  if (last === 0) {
    result = str;
  } else if (last !== i) {
    result += str.slice(last);
  }
  return addQuotes(result, singleQuote);
}

/**
 * Echos the value of any input. Tries to print the value out
 * in the best way possible given the different types.
 *
 * @param {any} value The value to print out.
 * @param {Object} opts Optional options object that alters the output.
 */

export function inspect(value: any, opts: InspectFormatOptions): string {

  // Default options
  const defaults = _extend<any>({
      seen: [],
      stylize: stylizeNoColor,
      indentationLvl: 0,
    }, inspectDefaultOptions);

  const ctx = _extend<InspectFormatContext>(defaults, opts as InspectFormatContext);
  // Set user-specified options
  
  if (ctx.colors) {
    ctx.stylize = stylizeWithColor;
  }
  if (ctx.maxEntries === null) {
    ctx.maxEntries = Infinity;
  }
  return formatValue(ctx, value, ctx.depth);
}

Object.defineProperty(inspect, "custom", {
  value: customInspectSymbol,
  configurable: false,
  enumerable: true,
  writable: false,
});

Object.defineProperty(inspect, "defaultOptions", {
  get() {
    return inspectDefaultOptions;
  },
  set(options: InspectFormatOptions) {
    if (options === null || typeof options !== "object") {
      throw new ERR_INVALID_ARG_TYPE("options", "Object", options);
    }
    // TODO(BridgeAR): Add input validation and make sure `defaultOptions` are
    // not configurable.
    return _extend(inspectDefaultOptions, options);
  }
});

type StyleCallback = (str: string, styleType: string) => string;
const stylizeNoColor: StyleCallback = (str: string, styleType: string) => str;

function stylizeWithColor(str: string, styleType: string): string {
  const style = styles[styleType];
  if (style !== undefined) {
    const color = colors[style];
    return `\u001b[${color[0]}]m${str}\u001b[${color[1]}m`;
  }
  return str;
}

function getConstructorName(obj: any) {
  while (obj) {
    const descriptor = Object.getOwnPropertyDescriptor(obj, "constructor");
    if (descriptor !== undefined &&
        typeof descriptor.value === "function" &&
        descriptor.value.name !== "") {
      return descriptor.value.name;
    }

    obj = Object.getPrototypeOf(obj);
  }

  return "";
}

function getPrefix(constructor: string, tag: string, fallback?: string) {
  if (constructor !== "") {
    if (tag !== "" && constructor !== tag) {
      return `${constructor} [${tag}] `;
    }
    return `${constructor} `;
  }

  if (tag !== "") {
    return `[${tag}] `;
  }

  if (fallback !== undefined) {
    return `${fallback} `;
  }

  return "";
}

type TypedArrayCheckDefintion = [(check: any) => boolean, Function];
const checks: TypedArrayCheckDefintion[] = [
  [isUint8Array, Uint8Array],
  [isUint8ClampedArray, Uint8ClampedArray],
  [isUint16Array, Uint16Array],
  [isUint32Array, Uint32Array],
  [isInt8Array, Int8Array],
  [isInt16Array, Int16Array],
  [isInt32Array, Int32Array],
  [isFloat32Array, Float32Array],
  [isFloat64Array, Float64Array],
//    [isBigInt64Array, BigInt64Array],
//    [isBigUint64Array, BigUint64Array]
]; 

function findTypedConstructor(value: any): any  {
  for (const [check, clazz] of checks) {
    if (check(value)) {
      return clazz;
    }
  }
}

const getBoxedValue = formatPrimitive.bind(null, stylizeNoColor);

function noPrototypeIterator(
  ctx: InspectFormatContext,
  value: any,
  recurseTimes: number,
): string {
  let newVal;
  // TODO: Create a Subclass in case there's no prototype and show
  // `null-prototype`.
  if (isSet(value)) {
    const clazz = Object.getPrototypeOf(value) || Set;
    newVal = new clazz(setValues(value));
  } else if (isMap(value)) {
    const clazz = Object.getPrototypeOf(value) || Map;
    newVal = new clazz(mapEntries(value));
  } else if (Array.isArray(value)) {
    const clazz = Object.getPrototypeOf(value) || Array;
    newVal = new clazz(value.length || 0);
  } else if (isTypedArray(value)) {
    const clazz = findTypedConstructor(value) || Uint8Array;
    newVal = new clazz(value);
  }
  if (newVal) {
    Object.defineProperties(newVal, Object.getOwnPropertyDescriptors(value));
    return formatValue(ctx, newVal, recurseTimes);
  }
  return "";
}

// Note: using `formatValue` directly requires the indentation level to be
// corrected by setting `ctx.indentationLvL += diff` and then to decrease the
// value afterwards again.
function formatValue(ctx: InspectFormatContext, value: any, recurseTimes: number): string {
  // Primitive types cannot have properties
  if (typeof value !== "object" && typeof value !== "function") {
    return formatPrimitive(ctx.stylize, value, ctx);
  }
  if (value === null) {
    return ctx.stylize("null", "null");
  }

  // remove proxy support

  // Provide a hook for user-specified inspect functions.
  // Check that value is an object with an inspect function on it
  if (ctx.customInspect) {
    const maybeCustom = value[customInspectSymbol];
    if (typeof maybeCustom === "function" &&
        // Filter out the util module, its inspect function is special
        maybeCustom !== inspect &&
        // Also filter out any prototype objects using the circular check.
        !(value.constructor && value.constructor.prototype === value)) {
      const ret = maybeCustom.call(value, recurseTimes, ctx);

      // If the custom inspection method returned `this`, don't go into
      // infinite recursion.
      if (ret !== value) {
        if (typeof ret !== "string") {
          return formatValue(ctx, ret, recurseTimes);
        }
        return ret;
      }
    }
  }

  // Using an array here is actually better for the average case than using
  // a Set. `seen` will only check for the depth and will never grow too large.
  if (ctx.seen.indexOf(value) !== -1) {
    return ctx.stylize("[Circular]", "special");
  }

  let keys;
  let symbols = Object.getOwnPropertySymbols(value);

  // Look up the keys of the object.
  if (ctx.showHidden) {
    keys = Object.getOwnPropertyNames(value);
  } else {
    // This might throw if `value` is a Module Namespace Object from an
    // unevaluated module, but we don't want to perform the actual type
    // check because it's expensive.
    // TODO(devsnek): track https://github.com/tc39/ecma262/issues/1209
    // and modify this logic as needed.

    //TODO(jtenner): uncomment this line 
    try {
      keys = Object.keys(value);
    } catch (err) {
      try {
        keys = Object.getOwnPropertyNames(value);
      } catch(err) {
        throw err;
      }
    }

    if (symbols.length !== 0) {
      symbols = symbols.filter((key) => propertyIsEnumerable(value, key));
    }
  }

  const keyLength = keys.length + symbols.length;

  const constructor = getConstructorName(value);
  let tag = value[Symbol.toStringTag];
  if (typeof tag !== "string") {
    tag = "";
  }
  let base = "";
  let formatter = formatObject;
  let braces: string[] = [];
  let noIterator = true;
  let extra;
  let i = 0;

  // Iterators and the rest are split to reduce checks
  if (value[Symbol.iterator]) {
    noIterator = false;
    if (Array.isArray(value)) {
      // Only set the constructor for non ordinary ("Array [...]") arrays.
      const prefix = getPrefix(constructor, tag);
      braces = [`${prefix === "Array " ? "" : prefix}[`, "]"];
      if (value.length === 0 && keyLength === 0) {
        return `${braces[0]}]`;
      }
      formatter = formatArray;
    } else if (isSet(value)) {
      const prefix = getPrefix(constructor, tag);
      if (value.size === 0 && keyLength === 0) {
        return `${prefix}{}`;
      }
      braces = [`${prefix}{`, "}"];
      formatter = formatSet;
    } else if (isMap(value)) {
      const prefix = getPrefix(constructor, tag);
      if (value.size === 0 && keyLength === 0) {
        return `${prefix}{}`;
      }
      braces = [`${prefix}{`, "}"];
      formatter = formatMap;
    } else if (isTypedArray(value)) {
      braces = [`${getPrefix(constructor, tag)}[`, "]"];
      if (value.length === 0 && keyLength === 0 && !ctx.showHidden) {
        return `${braces[0]}]`;
      }
      formatter = formatTypedArray;
    } else if (isMapIterator(value)) {
      braces = [`[${tag}] {`, "}"];
      formatter = formatMapIterator;
    } else if (isSetIterator(value)) {
      braces = [`[${tag}] {`, "}"];
      formatter = formatSetIterator;
    } else {
      noIterator = true;
    }
  }
  if (noIterator) {
    braces = ["{", "}"];
    if (constructor === "Object") {
      if (isArgumentsObject(value)) {
        if (keyLength === 0) {
          return "[Arguments] {}";
        }
        braces[0] = "[Arguments] {";
      } else if (tag !== "") {
        braces[0] = `${getPrefix(constructor, tag)}{`;
        if (keyLength === 0) {
          return `${braces[0]}}`;
        }
      } else if (keyLength === 0) {
        return "{}";
      }
    } else if (typeof value === "function") {
      const type = constructor || tag || "Function";
      const name = `${type}${value.name ? `: ${value.name}` : ""}`;
      if (keyLength === 0) {
        return ctx.stylize(`[${name}]`, "special");
      }
      base = `[${name}]`;
    } else if (isRegExp(value)) {
      // Make RegExps say that they are RegExps
      if (keyLength === 0 || recurseTimes < 0) {
        return ctx.stylize(regExpToString(value), "regexp");
      }
      base = `${regExpToString(value)}`;
    } else if (isDate(value)) {
      // Make dates with properties first say the date
      if (keyLength === 0) {
        if (Number.isNaN(dateGetTime(value))) {
          return ctx.stylize(String(value), "date");
        }
        return ctx.stylize(dateToISOString(value), "date");
      }
      base = dateToISOString(value);
    } else if (value instanceof Error) {
      // Make error with message first say the error
      base = formatError(value);
      // Wrap the error in brackets in case it has no stack trace.
      const stackStart = base.indexOf("\n    at");
      if (stackStart === -1) {
        base = `[${base}]`;
      }
      // The message and the stack have to be indented as well!
      if (ctx.indentationLvl !== 0) {
        const indentation = " ".repeat(ctx.indentationLvl);
        base = formatError(value).replace(/\n/g, `\n${indentation}`);
      }
      if (keyLength === 0) {
        return base;
      }

      if (ctx.compact === false && stackStart !== -1) {
        braces[0] += `${base.slice(stackStart)}`;
        base = `[${base.slice(0, stackStart)}]`;
      }
    } else if (isAnyArrayBuffer(value)) {
      // Fast path for ArrayBuffer and SharedArrayBuffer.
      // Can't do the same for DataView because it has a non-primitive
      // .buffer property that we need to recurse for.
      let prefix = getPrefix(constructor, tag);
      if (prefix === "") {
        prefix = isArrayBuffer(value) ? "ArrayBuffer " : "SharedArrayBuffer ";
      }
      if (keyLength === 0) {
        return prefix +
              `{ byteLength: ${formatNumber(ctx.stylize, value.byteLength)} }`;
      }
      braces[0] = `${prefix}{`;
      keys.unshift("byteLength");
    } else if (isDataView(value)) {
      braces[0] = `${getPrefix(constructor, tag, "DataView")}{`;
      // .buffer goes last, it's not a primitive like the others.
      keys.unshift("byteLength", "byteOffset", "buffer");
    } else if (isPromise(value)) {
      braces[0] = `${getPrefix(constructor, tag, "Promise")}{`;
      formatter = formatPromise;
    } else if (isWeakSet(value)) {
      braces[0] = `${getPrefix(constructor, tag, "WeakSet")}{`;
      if (ctx.showHidden) {
        formatter = formatWeakSet;
      } else {
        extra = ctx.stylize("<items unknown>", "special");
      }
    } else if (isWeakMap(value)) {
      braces[0] = `${getPrefix(constructor, tag, "WeakMap")}{`;
      if (ctx.showHidden) {
        formatter = formatWeakMap;
      } else {
        extra = ctx.stylize("<items unknown>", "special");
      }
    } else if (isNumberObject(value)) {
      base = `[Number: ${getBoxedValue(numberValueOf(value))}]`;
      if (keyLength === 0) {
        return ctx.stylize(base, "number");
      }
    } else if (isBooleanObject(value)) {
      base = `[Boolean: ${getBoxedValue(booleanValueOf(value))}]`;
      if (keyLength === 0) {
        return ctx.stylize(base, "boolean");
      }
    } /* else if (isBigIntObject(value)) {
      base = `[BigInt: ${getBoxedValue(bigIntValueOf(value))}]`;
      if (keyLength === 0)
        return ctx.stylize(base, "bigint");
    } */ else if (isSymbolObject(value)) {
      base = `[Symbol: ${getBoxedValue(symbolValueOf(value))}]`;
      if (keyLength === 0) {
        return ctx.stylize(base, "symbol");
      }
    } else if (isStringObject(value)) {
      const raw = stringValueOf(value);
      base = `[String: ${getBoxedValue(raw, ctx)}]`;
      if (keyLength === raw.length) {
        return ctx.stylize(base, "string");
      }
      // For boxed Strings, we have to remove the 0-n indexed entries,
      // since they just noisy up the output and are redundant
      // Make boxed primitive Strings look like such
      keys = keys.slice(value.length);
      braces = ["{", "}"];
    // The input prototype got manipulated. Special handle these.
    // We have to rebuild the information so we are able to display everything.
    } else {
      const specialIterator = noPrototypeIterator(ctx, value, recurseTimes);
      if (specialIterator) {
        return specialIterator;
      }
      if (isMapIterator(value)) {
        braces = [`[${tag || "Map Iterator"}] {`, "}"];
        formatter = formatMapIterator;
      } else if (isSetIterator(value)) {
        braces = [`[${tag || "Set Iterator"}] {`, "}"];
        formatter = formatSetIterator;
      // Handle other regular objects again.
      } else if (keyLength === 0) {
        if (isExternal(value)) {
          return ctx.stylize("[External]", "special");
        }
        return `${getPrefix(constructor, tag)}{}`;
      } else {
        braces[0] = `${getPrefix(constructor, tag)}{`;
      }
    }
  }

  if (recurseTimes != null) {
    if (recurseTimes < 0) {
      return ctx.stylize(`[${constructor || tag || "Object"}]`, "special");
    }
    recurseTimes -= 1;
  }

  ctx.seen.push(value);
  let output;

  // This corresponds to a depth of at least 333 and likely 500.
  if (ctx.indentationLvl < 1000) {
    output = formatter(ctx, value, recurseTimes, keys);
  } else {
    try {
      output = formatter(ctx, value, recurseTimes, keys);
    } catch (err) {
      if (err.name === "RangeError" &&
          err.message === "Maximum call stack size exceeded") {
        ctx.seen.pop();
        return ctx.stylize(
          `[${constructor || tag || "Object"}: Inspection interrupted ` +
            "prematurely. Maximum call stack size exceeded.]",
          "special"
        );
      }
      throw err;
    }
  }
  if (extra !== undefined) {
    output.unshift(extra);
  }

  for (i = 0; i < symbols.length; i++) {
    output.push(formatProperty(ctx, value, recurseTimes, symbols[i], 0));
  }

  ctx.seen.pop();

  return reduceToSingleString(ctx, output, base, braces);
}

function formatNumber(fn: StyleCallback, value: number) {
  // Format -0 as '-0". Checking `value === -0` won't distinguish 0 from -0.
  if (Object.is(value, -0)) {
    return fn("-0", "number");
  }
  return fn(`${value}`, "number");
}
/*
function formatBigInt(fn: StyleCallback, value: number) {
  return fn(`${value}n`, "bigint");
}
*/
function formatPrimitive(fn: StyleCallback, value: any, ctx: InspectFormatContext) {
  if (typeof value === "string") {
    if (ctx.compact === false &&
      ctx.indentationLvl + value.length > ctx.breakLength &&
      value.length > MIN_LINE_LENGTH) {
      // eslint-disable-next-line max-len
      const minLineLength = Math.max(ctx.breakLength - ctx.indentationLvl, MIN_LINE_LENGTH);
      // eslint-disable-next-line max-len
      const averageLineLength = Math.ceil(value.length / Math.ceil(value.length / minLineLength));
      const divisor = Math.max(averageLineLength, MIN_LINE_LENGTH);
      let res = "";
      if (readableRegExps[divisor] === undefined) {
        // Build a new RegExp that naturally breaks text into multiple lines.
        //
        // Rules
        // 1. Greedy match all text up the max line length that ends with a
        //    whitespace or the end of the string.
        // 2. If none matches, non-greedy match any text up to a whitespace or
        //    the end of the string.
        //
        // eslint-disable-next-line max-len, node-core/no-unescaped-regexp-dot
        readableRegExps[divisor] = new RegExp(`(.|\\n){1,${divisor}}(\\s|$)|(\\n|.)+?(\\s|$)`, "gm");
      }
      const matches = value.match(readableRegExps[divisor]);
      if (matches && matches.length > 1) {
        const indent = " ".repeat(ctx.indentationLvl);
        res += `${fn(strEscape(matches[0]), "string")} +\n`;
        let i = 1;
        for (; i < matches.length - 1; i++) {
          res += `${indent}  ${fn(strEscape(matches[i]), "string")} +\n`;
        }
        res += `${indent}  ${fn(strEscape(matches[i]), "string")}`;
        return res;
      }
    }
    return fn(strEscape(value), "string");
  }
  if (typeof value === "number") {
    return formatNumber(fn, value);
  }
  // eslint-disable-next-line valid-typeof
  /*if (typeof value === "bigint") {
    return formatBigInt(fn, value);
  } */
  if (typeof value === "boolean") {
    return fn(`${value}`, "boolean");
  }

  if (typeof value === "undefined") {
    return fn("undefined", "undefined");
  }

  // es6 symbol primitive
  return fn(value.toString(), "symbol");
}

function formatError(value: Error) {
  return value.stack || errorToString(value);
}

function formatObject(
  ctx: InspectFormatContext,
  value: any,
  recurseTimes: number,
  keys: string[],
) {
  const len = keys.length;
  const output = new Array(len);
  for (let i = 0; i < len; i++) {
    output[i] = formatProperty(ctx, value, recurseTimes, keys[i], 0);
  }
  return output;
}

// The array is sparse and/or has extra keys
function formatSpecialArray(
  ctx: InspectFormatContext,
  value: any[],
  recurseTimes: number,
  keys: string[],
  maxLength: number,
  valLen: number
): string[] {
  const output = [];
  const keyLen = keys.length;
  let i = 0;
  for (const key of keys) {
    if (output.length === maxLength) {
      break;
    }
    const index = +key;
    // Arrays can only have up to 2^32 - 1 entries
    if (index > 2 ** 32 - 2) {
      break;
    }
    if (`${i}` !== key) {
      if (!numberRegExp.test(key)) {
        break;
      }
      const emptyItems = index - i;
      const ending = emptyItems > 1 ? "s" : "";
      const message = `<${emptyItems} empty item${ending}>`;
      output.push(ctx.stylize(message, "undefined"));
      i = index;
      if (output.length === maxLength) {
        break;
      }
    }
    output.push(formatProperty(ctx, value, recurseTimes, key, 1));
    i++;
  }
  if (i < valLen && output.length !== maxLength) {
    const len = valLen - i;
    const ending = len > 1 ? "s" : "";
    const message = `<${len} empty item${ending}>`;
    output.push(ctx.stylize(message, "undefined"));
    i = valLen;
    if (keyLen === 0) {
      return output;
    }
  }
  const remaining = valLen - i;
  if (remaining > 0) {
    output.push(`... ${remaining} more item${remaining > 1 ? "s" : ""}`);
  }
  if (ctx.showHidden && keys[keyLen - 1] === "length") {
    // No extra keys
    output.push(formatProperty(ctx, value, recurseTimes, "length", 2));
  } else if (valLen === 0 ||
    keyLen > valLen && keys[valLen - 1] === `${valLen - 1}`) {
    // The array is not sparse
    for (i = valLen; i < keyLen; i++) {
      output.push(formatProperty(ctx, value, recurseTimes, keys[i], 2));
    }
  } else if (keys[keyLen - 1] !== `${valLen - 1}`) {
    const extra = [];
    // Only handle special keys
    let key;
    for (i = keys.length - 1; i >= 0; i--) {
      key = keys[i];
      if (numberRegExp.test(key) && +key < 2 ** 32 - 1) {
        break;
      }
      extra.push(formatProperty(ctx, value, recurseTimes, key, 2));
    }
    for (i = extra.length - 1; i >= 0; i--) {
      output.push(extra[i]);
    }
  }
  return output;
}

function formatArray(ctx: InspectFormatContext, value: any[], recurseTimes: number, keys: string[]) {
  const len = Math.min(Math.max(0, ctx.maxEntries), value.length);
  const hidden = ctx.showHidden ? 1 : 0;
  const valLen = value.length;
  const keyLen = keys.length - hidden;
  if (keyLen !== valLen || keys[keyLen - 1] !== `${valLen - 1}`) {
    return formatSpecialArray(ctx, value, recurseTimes, keys, len, valLen);
  }

  const remaining = valLen - len;
  const output = new Array(len + (remaining > 0 ? 1 : 0) + hidden);
  let i = 0;
  for (; i < len; i++) {
    output[i] = formatProperty(ctx, value, recurseTimes, keys[i], 1);
  }
  if (remaining > 0) {
    output[i++] = `... ${remaining} more item${remaining > 1 ? "s" : ""}`;
  }
  if (ctx.showHidden === true) {
    output[i] = formatProperty(ctx, value, recurseTimes, "length", 2);
  }
  return output;
}

function formatTypedArray(
  ctx: InspectFormatContext,
  value: TypedArray,
  recurseTimes: number,
  keys: string[],
): string[] {
  const maxLength = Math.min(Math.max(0, ctx.maxEntries), value.length);
  const remaining = value.length - maxLength;
  const output = new Array(maxLength + (remaining > 0 ? 1 : 0));
  const elementFormatter = value.length > 0 && typeof value[0] === "number" ?
    formatNumber :
    // todo(jtenner): uncomment the following line when formatBigInt works
    (fn: StyleCallback, value: number) => value.toString();
    //formatBigInt;
  let i = 0;
  for (; i < maxLength; ++i) {
    output[i] = elementFormatter(ctx.stylize, value[i]);
  }
  if (remaining > 0) {
    output[i] = `... ${remaining} more item${remaining > 1 ? "s" : ""}`;
  }
  if (ctx.showHidden) {
    // .buffer goes last, it's not a primitive like the others.
    ctx.indentationLvl += 2;
    // explicitly push each property manually because TypeArray is not indexable on string
    output.push(ctx, `[BYTES_PER_ELEMENT]: ${value.BYTES_PER_ELEMENT}`, recurseTimes);
    output.push(ctx, `[length]: ${value.length}`, recurseTimes);
    output.push(ctx, `[byteLength]: ${value.byteLength}`, recurseTimes);
    output.push(ctx, `[byteOffset]: ${value.byteOffset}`, recurseTimes);
    output.push(ctx, `[buffer]: ${value.buffer}`, recurseTimes);
    ctx.indentationLvl -= 2;
  }
  // TypedArrays cannot have holes. Therefore it is safe to assume that all
  // extra keys are indexed after value.length.
  for (i = value.length; i < keys.length; i++) {
    output.push(formatProperty(ctx, value, recurseTimes, keys[i], 2));
  }
  return output;
}

function formatSet(ctx: InspectFormatContext, value: Set<any>, recurseTimes: number, keys: string[]) {
  const output = new Array(value.size + keys.length + (ctx.showHidden ? 1 : 0));
  let i = 0;
  ctx.indentationLvl += 2;
  for (const v of value) {
    output[i++] = formatValue(ctx, v, recurseTimes);
  }
  ctx.indentationLvl -= 2;
  // With `showHidden`, `length` will display as a hidden property for
  // arrays. For consistency's sake, do the same for `size`, even though this
  // property isn't selected by Object.getOwnPropertyNames().
  if (ctx.showHidden) {
    output[i++] = `[size]: ${ctx.stylize(`${value.size}`, "number")}`;
  }

  for (const key of keys) {
    output[i++] = formatProperty(ctx, value, recurseTimes, key, 0);
  }
  return output;
}

function formatMap(ctx: InspectFormatContext, value: any, recurseTimes: number, keys: string[]) {
  const output = new Array(value.size + keys.length + (ctx.showHidden ? 1 : 0));
  let i = 0;
  ctx.indentationLvl += 2;
  for (const [k, v] of value) {
    output[i++] = `${formatValue(ctx, k, recurseTimes)} => ` +
                  formatValue(ctx, v, recurseTimes);
  }
  ctx.indentationLvl -= 2;
  // See comment in formatSet
  if (ctx.showHidden) {
    output[i++] = `[size]: ${ctx.stylize(`${value.size}`, "number")}`;
  }
  for (const key of keys) {
    output[i++] = formatProperty(ctx, value, recurseTimes, key, 0);
  }
  return output;
}

function formatWeakSet(ctx: InspectFormatContext): string[] {
  return [
    ctx.stylize("[WeakSet]", "special"),
  ];
}

function formatWeakMap(ctx: InspectFormatContext): string[] {
  return [
    ctx.stylize("[WeakMap]", "special"),
  ];
}

function formatSetIterator(ctx: InspectFormatContext): string[] {
  return [
    ctx.stylize("[SetIterator]", "special"),
  ];
}

function formatMapIterator(ctx: InspectFormatContext): string[] {
  return [
    ctx.stylize("[MapIterator]", "special"),
  ];
}

function formatPromise(ctx: InspectFormatContext): string[] {
  return [
    ctx.stylize("[Promise]", "special"),
  ];
}

function formatProperty(
  ctx: InspectFormatContext,
  value: any,
  recurseTimes: number,
  key: string | symbol,
  array: IteratorState,
): string {
  let name, str;
  let extra = " ";
  const desc = Object.getOwnPropertyDescriptor(value, key) ||
    { value: value[key], enumerable: true };
  if (desc.value !== undefined) {
    const diff = array !== IteratorState.kWeak || ctx.compact === false ? 2 : 3;
    ctx.indentationLvl += diff;
    str = formatValue(ctx, desc.value, recurseTimes);
    if (diff === 3) {
      const len = ctx.colors ? removeColors(str).length : str.length;
      if (ctx.breakLength < len) {
        extra = `\n${" ".repeat(ctx.indentationLvl)}`;
      }
    }
    ctx.indentationLvl -= diff;
  } else if (desc.get !== undefined) {
    if (desc.set !== undefined) {
      str = ctx.stylize("[Getter/Setter]", "special");
    } else {
      str = ctx.stylize("[Getter]", "special");
    }
  } else if (desc.set !== undefined) {
    str = ctx.stylize("[Setter]", "special");
  } else {
    str = ctx.stylize("undefined", "undefined");
  }
  if (array === 1) {
    return str;
  }
  if (typeof key === "symbol") {
    const tmp = key.toString().replace(strEscapeSequencesReplacer, escapeFn);
    name = `[${ctx.stylize(tmp, "symbol")}]`;
  } else if (desc.enumerable === false) {
    name = `[${key.replace(strEscapeSequencesReplacer, escapeFn)}]`;
  } else if (keyStrRegExp.test(key)) {
    name = ctx.stylize(key, "name");
  } else {
    name = ctx.stylize(strEscape(key), "string");
  }
  return `${name}:${extra}${str}`;
}

function reduceToSingleString(
  ctx: InspectFormatContext,
  output: string[],
  base: string,
  braces: string[],
): string {
  const breakLength = ctx.breakLength;
  let i = 0;
  if (ctx.compact === false) {
    const indentation = " ".repeat(ctx.indentationLvl);
    let res = `${base ? `${base} ` : ""}${braces[0]}\n${indentation}  `;
    for (; i < output.length - 1; i++) {
      res += `${output[i]},\n${indentation}  `;
    }
    res += `${output[i]}\n${indentation}${braces[1]}`;
    return res;
  }
  if (output.length * 2 <= breakLength) {
    let length = 0;
    for (; i < output.length && length <= breakLength; i++) {
      if (ctx.colors) {
        length += removeColors(output[i]).length + 1;
      } else {
        length += output[i].length + 1;
      }
    }
    if (length <= breakLength) {
      return `${braces[0]}${base ? ` ${base}` : ""} ${join(output, ", ")} ` +
        braces[1];
    }
  }
  // If the opening "brace" is too large, like in the case of "Set {",
  // we need to force the first item to be on the next line or the
  // items will not line up correctly.
  const indentation = " ".repeat(ctx.indentationLvl);
  const ln = base === "" && braces[0].length === 1 ?
    " " : `${base ? ` ${base}` : ""}\n${indentation}  `;
  const str = join(output, `,\n${indentation}  `);
  return `${braces[0]}${ln}${str} ${braces[1]}`;
}

function _extend<T extends { [key: string]: any }>(target: T, source: T) {
  // Don't do anything if source isn't an object
  if (source === null || typeof source !== "object") {
    return target;
  } 

  const keys = Object.keys(source);
  let i = keys.length;
  while (i--) {
    target[keys[i]] = source[keys[i]];
  }
  return target;
}
