import { assert } from "./asserts.ts";

// This file is ported from pretty-format@24.0.0
/**
 * Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Refs = any[];
export type Optional<T> = { [K in keyof T]?: T[K] };

export interface Options {
  callToJSON: boolean;
  escapeRegex: boolean;
  escapeString: boolean;
  indent: number;
  maxDepth: number;
  min: boolean;
  printFunctionName: boolean;
}

export interface Config {
  callToJSON: boolean;
  escapeRegex: boolean;
  escapeString: boolean;
  indent: string;
  maxDepth: number;
  min: boolean;
  printFunctionName: boolean;
  spacingInner: string;
  spacingOuter: string;
}

export type Printer = (
  val: unknown,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  hasCalledToJSON?: boolean
) => string;

const toString = Object.prototype.toString;
const toISOString = Date.prototype.toISOString;
const errorToString = Error.prototype.toString;
const regExpToString = RegExp.prototype.toString;
const symbolToString = Symbol.prototype.toString;

const DEFAULT_OPTIONS: Options = {
  callToJSON: true,
  escapeRegex: false,
  escapeString: true,
  indent: 2,
  maxDepth: Infinity,
  min: false,
  printFunctionName: true,
};

interface BasicValueOptions {
  printFunctionName: boolean;
  escapeRegex: boolean;
  escapeString: boolean;
}

/**
 * Explicitly comparing typeof constructor to function avoids undefined as name
 * when mock identity-obj-proxy returns the key as the value for any key.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const getConstructorName = (val: new (...args: any[]) => any): string =>
  (typeof val.constructor === "function" && val.constructor.name) || "Object";

/* global window */
/** Is val is equal to global window object?
 *  Works even if it does not exist :)
 * */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const isWindow = (val: any): val is Window =>
  typeof window !== "undefined" && val === window;

const SYMBOL_REGEXP = /^Symbol\((.*)\)(.*)$/;

function isToStringedArrayType(toStringed: string): boolean {
  return (
    toStringed === "[object Array]" ||
    toStringed === "[object ArrayBuffer]" ||
    toStringed === "[object DataView]" ||
    toStringed === "[object Float32Array]" ||
    toStringed === "[object Float64Array]" ||
    toStringed === "[object Int8Array]" ||
    toStringed === "[object Int16Array]" ||
    toStringed === "[object Int32Array]" ||
    toStringed === "[object Uint8Array]" ||
    toStringed === "[object Uint8ClampedArray]" ||
    toStringed === "[object Uint16Array]" ||
    toStringed === "[object Uint32Array]"
  );
}

function printNumber(val: number): string {
  return Object.is(val, -0) ? "-0" : String(val);
}

function printFunction(val: () => void, printFunctionName: boolean): string {
  if (!printFunctionName) {
    return "[Function]";
  }
  return "[Function " + (val.name || "anonymous") + "]";
}

function printSymbol(val: symbol): string {
  return symbolToString.call(val).replace(SYMBOL_REGEXP, "Symbol($1)");
}

function printError(val: Error): string {
  return "[" + errorToString.call(val) + "]";
}

/**
 * The first port of call for printing an object, handles most of the
 * data-types in JS.
 */
function printBasicValue(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  val: any,
  { printFunctionName, escapeRegex, escapeString }: BasicValueOptions
): string | null {
  if (val === true || val === false) {
    return String(val);
  }
  if (val === undefined) {
    return "undefined";
  }
  if (val === null) {
    return "null";
  }

  const typeOf = typeof val;

  if (typeOf === "number") {
    return printNumber(val);
  }
  if (typeOf === "string") {
    if (escapeString) {
      return `"${val.replace(/"|\\/g, "\\$&")}"`;
    }
    return `"${val}"`;
  }
  if (typeOf === "function") {
    return printFunction(val, printFunctionName);
  }
  if (typeOf === "symbol") {
    return printSymbol(val);
  }

  const toStringed = toString.call(val);

  if (toStringed === "[object WeakMap]") {
    return "WeakMap {}";
  }
  if (toStringed === "[object WeakSet]") {
    return "WeakSet {}";
  }
  if (
    toStringed === "[object Function]" ||
    toStringed === "[object GeneratorFunction]"
  ) {
    return printFunction(val, printFunctionName);
  }
  if (toStringed === "[object Symbol]") {
    return printSymbol(val);
  }
  if (toStringed === "[object Date]") {
    return isNaN(+val) ? "Date { NaN }" : toISOString.call(val);
  }
  if (toStringed === "[object Error]") {
    return printError(val);
  }
  if (toStringed === "[object RegExp]") {
    if (escapeRegex) {
      // https://github.com/benjamingr/RegExp.escape/blob/master/polyfill.js
      return regExpToString.call(val).replace(/[\\^$*+?.()|[\]{}]/g, "\\$&");
    }
    return regExpToString.call(val);
  }

  if (val instanceof Error) {
    return printError(val);
  }

  return null;
}

function printer(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  val: any,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  hasCalledToJSON?: boolean
): string {
  const basicResult = printBasicValue(val, config);
  if (basicResult !== null) {
    return basicResult;
  }
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  return printComplexValue(
    val,
    config,
    indentation,
    depth,
    refs,
    hasCalledToJSON
  );
}

/**
 * Return items (for example, of an array)
 * with spacing, indentation, and comma
 * without surrounding punctuation (for example, brackets)
 */
function printListItems(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  list: any,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  printer: Printer
): string {
  let result = "";

  if (list.length) {
    result += config.spacingOuter;

    const indentationNext = indentation + config.indent;

    for (let i = 0; i < list.length; i++) {
      result +=
        indentationNext +
        printer(list[i], config, indentationNext, depth, refs);

      if (i < list.length - 1) {
        result += "," + config.spacingInner;
      } else if (!config.min) {
        result += ",";
      }
    }

    result += config.spacingOuter + indentation;
  }

  return result;
}

/**
 * Return entries (for example, of a map)
 * with spacing, indentation, and comma
 * without surrounding punctuation (for example, braces)
 */
function printIteratorEntries(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  iterator: any,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  printer: Printer,
  // Too bad, so sad that separator for ECMAScript Map has been ' => '
  // What a distracting diff if you change a data structure to/from
  // ECMAScript Object or Immutable.Map/OrderedMap which use the default.
  separator = ": "
): string {
  let result = "";
  let current = iterator.next();

  if (!current.done) {
    result += config.spacingOuter;

    const indentationNext = indentation + config.indent;

    while (!current.done) {
      const name = printer(
        current.value[0],
        config,
        indentationNext,
        depth,
        refs
      );
      const value = printer(
        current.value[1],
        config,
        indentationNext,
        depth,
        refs
      );

      result += indentationNext + name + separator + value;

      current = iterator.next();

      if (!current.done) {
        result += "," + config.spacingInner;
      } else if (!config.min) {
        result += ",";
      }
    }

    result += config.spacingOuter + indentation;
  }

  return result;
}

/**
 * Return values (for example, of a set)
 * with spacing, indentation, and comma
 * without surrounding punctuation (braces or brackets)
 */
function printIteratorValues(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  iterator: Iterator<any>,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  printer: Printer
): string {
  let result = "";
  let current = iterator.next();

  if (!current.done) {
    result += config.spacingOuter;

    const indentationNext = indentation + config.indent;

    while (!current.done) {
      result +=
        indentationNext +
        printer(current.value, config, indentationNext, depth, refs);

      current = iterator.next();

      if (!current.done) {
        result += "," + config.spacingInner;
      } else if (!config.min) {
        result += ",";
      }
    }

    result += config.spacingOuter + indentation;
  }

  return result;
}

function getKeysOfEnumerableProperties<T>(object: T): Array<keyof T | symbol> {
  const keys = Object.keys(object).sort() as Array<keyof T | symbol>;

  if (Object.getOwnPropertySymbols) {
    Object.getOwnPropertySymbols(object).forEach((symbol): void => {
      const d = Object.getOwnPropertyDescriptor(object, symbol);
      assert(d != null);
      if (d.enumerable) {
        keys.push(symbol);
      }
    });
  }

  return keys;
}

/**
 * Return properties of an object
 * with spacing, indentation, and comma
 * without surrounding punctuation (for example, braces)
 */
function printObjectProperties(
  val: {},
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  printer: Printer
): string {
  let result = "";
  const keys = getKeysOfEnumerableProperties(val);

  if (keys.length) {
    result += config.spacingOuter;

    const indentationNext = indentation + config.indent;

    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      const name = printer(key, config, indentationNext, depth, refs);
      const value = printer(
        val[key as keyof typeof val],
        config,
        indentationNext,
        depth,
        refs
      );

      result += indentationNext + name + ": " + value;

      if (i < keys.length - 1) {
        result += "," + config.spacingInner;
      } else if (!config.min) {
        result += ",";
      }
    }

    result += config.spacingOuter + indentation;
  }

  return result;
}

/**
 * Handles more complex objects ( such as objects with circular references.
 * maps and sets etc )
 */
function printComplexValue(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  val: any,
  config: Config,
  indentation: string,
  depth: number,
  refs: Refs,
  hasCalledToJSON?: boolean
): string {
  if (refs.indexOf(val) !== -1) {
    return "[Circular]";
  }
  refs = refs.slice();
  refs.push(val);

  const hitMaxDepth = ++depth > config.maxDepth;
  const { min, callToJSON } = config;

  if (
    callToJSON &&
    !hitMaxDepth &&
    val.toJSON &&
    typeof val.toJSON === "function" &&
    !hasCalledToJSON
  ) {
    return printer(val.toJSON(), config, indentation, depth, refs, true);
  }

  const toStringed = toString.call(val);
  if (toStringed === "[object Arguments]") {
    return hitMaxDepth
      ? "[Arguments]"
      : (min ? "" : "Arguments ") +
          "[" +
          printListItems(val, config, indentation, depth, refs, printer) +
          "]";
  }
  if (isToStringedArrayType(toStringed)) {
    return hitMaxDepth
      ? `[${val.constructor.name}]`
      : (min ? "" : `${val.constructor.name} `) +
          "[" +
          printListItems(val, config, indentation, depth, refs, printer) +
          "]";
  }
  if (toStringed === "[object Map]") {
    return hitMaxDepth
      ? "[Map]"
      : "Map {" +
          printIteratorEntries(
            val.entries(),
            config,
            indentation,
            depth,
            refs,
            printer,
            " => "
          ) +
          "}";
  }
  if (toStringed === "[object Set]") {
    return hitMaxDepth
      ? "[Set]"
      : "Set {" +
          printIteratorValues(
            val.values(),
            config,
            indentation,
            depth,
            refs,
            printer
          ) +
          "}";
  }

  // Avoid failure to serialize global window object in jsdom test environment.
  // For example, not even relevant if window is prop of React element.
  return hitMaxDepth || isWindow(val)
    ? "[" + getConstructorName(val) + "]"
    : (min ? "" : getConstructorName(val) + " ") +
        "{" +
        printObjectProperties(val, config, indentation, depth, refs, printer) +
        "}";
}

// TODO this is better done with `.padStart()`
function createIndent(indent: number): string {
  return new Array(indent + 1).join(" ");
}

const getConfig = (options: Options): Config => ({
  ...options,
  indent: options.min ? "" : createIndent(options.indent),
  spacingInner: options.min ? " " : "\n",
  spacingOuter: options.min ? "" : "\n",
});

/**
 * Returns a presentation string of your `val` object
 * @param val any potential JavaScript object
 * @param options Custom settings
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function format(val: any, options: Optional<Options> = {}): string {
  const opts: Options = {
    ...DEFAULT_OPTIONS,
    ...options,
  };
  const basicResult = printBasicValue(val, opts);
  if (basicResult !== null) {
    return basicResult;
  }

  return printComplexValue(val, getConfig(opts), "", 0, []);
}
