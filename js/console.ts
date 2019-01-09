// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { isTypedArray } from "./util";

// tslint:disable-next-line:no-any
type ConsoleContext = Set<any>;
type ConsoleOptions = Partial<{
  showHidden: boolean;
  depth: number;
  colors: boolean;
  indentLevel: number;
  collapsedAt: number | null;
}>;

// Default depth of logging nested objects
const DEFAULT_MAX_DEPTH = 4;

// tslint:disable-next-line:no-any
function getClassInstanceName(instance: any): string {
  if (typeof instance !== "object") {
    return "";
  }
  if (instance) {
    const proto = Object.getPrototypeOf(instance);
    if (proto && proto.constructor) {
      return proto.constructor.name; // could be "Object" or "Array"
    }
  }
  return "";
}

function createFunctionString(value: Function, ctx: ConsoleContext): string {
  // Might be Function/AsyncFunction/GeneratorFunction
  const cstrName = Object.getPrototypeOf(value).constructor.name;
  if (value.name && value.name !== "anonymous") {
    // from MDN spec
    return `[${cstrName}: ${value.name}]`;
  }
  return `[${cstrName}]`;
}

interface IterablePrintConfig {
  typeName: string;
  displayName: string;
  delims: [string, string];
  entryHandler: (
    // tslint:disable-next-line:no-any
    entry: any,
    ctx: ConsoleContext,
    level: number,
    maxLevel: number
  ) => string;
}

function createIterableString(
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number,
  config: IterablePrintConfig
): string {
  if (level >= maxLevel) {
    return `[${config.typeName}]`;
  }
  ctx.add(value);

  const entries: string[] = [];
  // In cases e.g. Uint8Array.prototype
  try {
    for (const el of value) {
      entries.push(config.entryHandler(el, ctx, level + 1, maxLevel));
    }
  } catch (e) {}
  ctx.delete(value);
  const iPrefix = `${config.displayName ? config.displayName + " " : ""}`;
  const iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
  return `${iPrefix}${config.delims[0]}${iContent}${config.delims[1]}`;
}

function createArrayString(
  // tslint:disable-next-line:no-any
  value: any[],
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig = {
    typeName: "Array",
    displayName: "",
    delims: ["[", "]"],
    entryHandler: (el, ctx, level, maxLevel) =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel)
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createTypedArrayString(
  typedArrayName: string,
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig = {
    typeName: typedArrayName,
    displayName: typedArrayName,
    delims: ["[", "]"],
    entryHandler: (el, ctx, level, maxLevel) =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel)
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createSetString(
  // tslint:disable-next-line:no-any
  value: Set<any>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig = {
    typeName: "Set",
    displayName: "Set",
    delims: ["{", "}"],
    entryHandler: (el, ctx, level, maxLevel) =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel)
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createMapString(
  // tslint:disable-next-line:no-any
  value: Map<any, any>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig = {
    typeName: "Map",
    displayName: "Map",
    delims: ["{", "}"],
    entryHandler: (el, ctx, level, maxLevel) => {
      const [key, val] = el;
      return `${stringifyWithQuotes(
        key,
        ctx,
        level + 1,
        maxLevel
      )} => ${stringifyWithQuotes(val, ctx, level + 1, maxLevel)}`;
    }
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createWeakSetString(): string {
  return "WeakSet { [items unknown] }"; // as seen in Node
}

function createWeakMapString(): string {
  return "WeakMap { [items unknown] }"; // as seen in Node
}

function createDateString(value: Date) {
  // without quotes, ISO format
  return value.toISOString();
}

function createRegExpString(value: RegExp) {
  return value.toString();
}

// tslint:disable-next-line:ban-types
function createStringWrapperString(value: String) {
  return `[String: "${value.toString()}"]`;
}

// tslint:disable-next-line:ban-types
function createBooleanWrapperString(value: Boolean) {
  return `[Boolean: ${value.toString()}]`;
}

// tslint:disable-next-line:ban-types
function createNumberWrapperString(value: Number) {
  return `[Number: ${value.toString()}]`;
}

// TODO: Promise, requires v8 bindings to get info
// TODO: Proxy

function createRawObjectString(
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  if (level >= maxLevel) {
    return "[Object]";
  }
  ctx.add(value);

  const entries: string[] = [];
  let baseString = "";

  const className = getClassInstanceName(value);
  let shouldShowClassName = false;
  if (className && className !== "Object" && className !== "anonymous") {
    shouldShowClassName = true;
  }

  for (const key of Object.keys(value)) {
    entries.push(
      `${key}: ${stringifyWithQuotes(value[key], ctx, level + 1, maxLevel)}`
    );
  }

  ctx.delete(value);

  if (entries.length === 0) {
    baseString = "{}";
  } else {
    baseString = `{ ${entries.join(", ")} }`;
  }

  if (shouldShowClassName) {
    baseString = `${className} ${baseString}`;
  }

  return baseString;
}

function createObjectString(
  // tslint:disable-next-line:no-any
  value: any,
  ...args: [ConsoleContext, number, number]
): string {
  if (value instanceof Error) {
    return value.stack! || "";
  } else if (Array.isArray(value)) {
    return createArrayString(value, ...args);
  } else if (value instanceof Number) {
    // tslint:disable-next-line:ban-types
    return createNumberWrapperString(value as Number);
  } else if (value instanceof Boolean) {
    // tslint:disable-next-line:ban-types
    return createBooleanWrapperString(value as Boolean);
  } else if (value instanceof String) {
    // tslint:disable-next-line:ban-types
    return createStringWrapperString(value as String);
  } else if (value instanceof RegExp) {
    return createRegExpString(value as RegExp);
  } else if (value instanceof Date) {
    return createDateString(value as Date);
  } else if (value instanceof Set) {
    // tslint:disable-next-line:no-any
    return createSetString(value as Set<any>, ...args);
  } else if (value instanceof Map) {
    // tslint:disable-next-line:no-any
    return createMapString(value as Map<any, any>, ...args);
  } else if (value instanceof WeakSet) {
    return createWeakSetString();
  } else if (value instanceof WeakMap) {
    return createWeakMapString();
  } else if (isTypedArray(value)) {
    return createTypedArrayString(
      Object.getPrototypeOf(value).constructor.name,
      value,
      ...args
    );
  } else {
    // Otherwise, default object formatting
    return createRawObjectString(value, ...args);
  }
}

function stringify(
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      return value;
    case "number":
    case "boolean":
    case "undefined":
    case "symbol":
      return String(value);
    case "bigint":
      return `${value}n`;
    case "function":
      return createFunctionString(value as Function, ctx);
    case "object":
      if (value === null) {
        return "null";
      }

      if (ctx.has(value)) {
        return "[Circular]";
      }

      return createObjectString(value, ctx, level, maxLevel);
    default:
      return "[Not Implemented]";
  }
}

// Print strings when they are inside of arrays or objects with quotes
function stringifyWithQuotes(
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      return `"${value}"`;
    default:
      return stringify(value, ctx, level, maxLevel);
  }
}

// Returns true when the console is collapsed.
function isCollapsed(
  collapsedAt: number | null | undefined,
  indentLevel: number | null | undefined
) {
  if (collapsedAt == null || indentLevel == null) {
    return false;
  }

  return collapsedAt <= indentLevel;
}

/** TODO Do not expose this from "deno" namespace. */
export function stringifyArgs(
  // tslint:disable-next-line:no-any
  args: any[],
  options: ConsoleOptions = {}
): string {
  const out: string[] = [];
  const { collapsedAt, indentLevel } = options;
  for (const a of args) {
    if (typeof a === "string") {
      out.push(a);
    } else {
      out.push(
        // use default maximum depth for null or undefined argument
        stringify(
          a,
          // tslint:disable-next-line:no-any
          new Set<any>(),
          0,
          // tslint:disable-next-line:triple-equals
          options.depth != undefined ? options.depth : DEFAULT_MAX_DEPTH
        )
      );
    }
  }
  let outstr = out.join(" ");
  if (
    !isCollapsed(collapsedAt, indentLevel) &&
    indentLevel != null &&
    indentLevel > 0
  ) {
    const groupIndent = " ".repeat(indentLevel);
    if (outstr.indexOf("\n") !== -1) {
      outstr = outstr.replace(/\n/g, `\n${groupIndent}`);
    }
    outstr = groupIndent + outstr;
  }
  return outstr;
}

type PrintFunc = (x: string, isErr?: boolean, printsNewline?: boolean) => void;

const countMap = new Map<string, number>();
const timerMap = new Map<string, number>();

export class Console {
  indentLevel: number;
  collapsedAt: number | null;
  constructor(private printFunc: PrintFunc) {
    this.indentLevel = 0;
    this.collapsedAt = null;
  }

  /** Writes the arguments to stdout */
  // tslint:disable-next-line:no-any
  log = (...args: any[]): void => {
    this.printFunc(
      stringifyArgs(args, {
        indentLevel: this.indentLevel,
        collapsedAt: this.collapsedAt
      }),
      false,
      !isCollapsed(this.collapsedAt, this.indentLevel)
    );
  };

  /** Writes the arguments to stdout */
  debug = this.log;
  /** Writes the arguments to stdout */
  info = this.log;

  /** Writes the properties of the supplied `obj` to stdout */
  // tslint:disable-next-line:no-any
  dir = (obj: any, options: ConsoleOptions = {}) => {
    this.log(stringifyArgs([obj], options));
  };

  /** Writes the arguments to stdout */
  // tslint:disable-next-line:no-any
  warn = (...args: any[]): void => {
    this.printFunc(
      stringifyArgs(args, {
        indentLevel: this.indentLevel,
        collapsedAt: this.collapsedAt
      }),
      true,
      !isCollapsed(this.collapsedAt, this.indentLevel)
    );
  };

  /** Writes the arguments to stdout */
  error = this.warn;

  /** Writes an error message to stdout if the assertion is `false`. If the
   * assertion is `true`, nothing happens.
   *
   * ref: https://console.spec.whatwg.org/#assert
   */
  // tslint:disable-next-line:no-any
  assert = (condition = false, ...args: any[]): void => {
    if (condition) {
      return;
    }

    if (args.length === 0) {
      this.error("Assertion failed");
      return;
    }

    const [first, ...rest] = args;

    if (typeof first === "string") {
      this.error(`Assertion failed: ${first}`, ...rest);
      return;
    }

    this.error(`Assertion failed:`, ...args);
  };

  count = (label = "default"): void => {
    label = String(label);

    if (countMap.has(label)) {
      const current = countMap.get(label) || 0;
      countMap.set(label, current + 1);
    } else {
      countMap.set(label, 1);
    }

    this.info(`${label}: ${countMap.get(label)}`);
  };

  countReset = (label = "default"): void => {
    label = String(label);

    if (countMap.has(label)) {
      countMap.set(label, 0);
    } else {
      this.warn(`Count for '${label}' does not exist`);
    }
  };

  time = (label = "default"): void => {
    label = String(label);

    if (timerMap.has(label)) {
      this.warn(`Timer '${label}' already exists`);
      return;
    }

    timerMap.set(label, Date.now());
  };

  // tslint:disable-next-line:no-any
  timeLog = (label = "default", ...args: any[]): void => {
    label = String(label);

    if (!timerMap.has(label)) {
      this.warn(`Timer '${label}' does not exists`);
      return;
    }

    const startTime = timerMap.get(label) as number;
    const duration = Date.now() - startTime;

    this.info(`${label}: ${duration}ms`, ...args);
  };

  timeEnd = (label = "default"): void => {
    label = String(label);

    if (!timerMap.has(label)) {
      this.warn(`Timer '${label}' does not exists`);
      return;
    }

    const startTime = timerMap.get(label) as number;
    timerMap.delete(label);
    const duration = Date.now() - startTime;

    this.info(`${label}: ${duration}ms`);
  };

  group = (...label: Array<unknown>): void => {
    if (label.length > 0) {
      this.log(...label);
    }
    this.indentLevel += 2;
  };

  groupCollapsed = (...label: Array<unknown>): void => {
    if (this.collapsedAt == null) {
      this.collapsedAt = this.indentLevel;
    }
    this.group(...label);
  };

  groupEnd = (): void => {
    if (this.indentLevel > 0) {
      this.indentLevel -= 2;
    }
    if (this.collapsedAt != null && this.collapsedAt >= this.indentLevel) {
      this.collapsedAt = null;
      this.log(); // When the collapsed state ended, outputs a sinle new line.
    }
  };
}

/**
 * inspect() converts input into string that has the same format
 * as printed by console.log(...);
 */
export function inspect(
  value: any, // tslint:disable-line:no-any
  options?: ConsoleOptions
) {
  const opts = options || {};
  if (typeof value === "string") {
    return value;
  } else {
    return stringify(
      value,
      // tslint:disable-next-line:no-any
      new Set<any>(),
      0,
      // tslint:disable-next-line:triple-equals
      opts.depth != undefined ? opts.depth : DEFAULT_MAX_DEPTH
    );
  }
}
