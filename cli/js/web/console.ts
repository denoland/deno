// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { isTypedArray, TypedArray } from "./util.ts";
import { TextEncoder } from "./text_encoding.ts";
import { File, stdout } from "../files.ts";
import { cliTable } from "./console_table.ts";
import { exposeForTest } from "../internals.ts";

type ConsoleContext = Set<unknown>;
type InspectOptions = Partial<{
  showHidden: boolean;
  depth: number;
  colors: boolean;
  indentLevel: number;
}>;

// Default depth of logging nested objects
const DEFAULT_MAX_DEPTH = 4;

const LINE_BREAKING_LENGTH = 80;

const STR_ABBREVIATE_SIZE = 100;

// Char codes
const CHAR_PERCENT = 37; /* % */
const CHAR_LOWERCASE_S = 115; /* s */
const CHAR_LOWERCASE_D = 100; /* d */
const CHAR_LOWERCASE_I = 105; /* i */
const CHAR_LOWERCASE_F = 102; /* f */
const CHAR_LOWERCASE_O = 111; /* o */
const CHAR_UPPERCASE_O = 79; /* O */
const CHAR_LOWERCASE_C = 99; /* c */
export class CSI {
  static kClear = "\x1b[1;1H";
  static kClearScreenDown = "\x1b[0J";
}

/* eslint-disable @typescript-eslint/no-use-before-define */

function cursorTo(stream: File, _x: number, _y?: number): void {
  const uint8 = new TextEncoder().encode(CSI.kClear);
  stream.writeSync(uint8);
}

function clearScreenDown(stream: File): void {
  const uint8 = new TextEncoder().encode(CSI.kClearScreenDown);
  stream.writeSync(uint8);
}

function getClassInstanceName(instance: unknown): string {
  if (typeof instance !== "object") {
    return "";
  }
  if (!instance) {
    return "";
  }

  const proto = Object.getPrototypeOf(instance);
  if (proto && proto.constructor) {
    return proto.constructor.name; // could be "Object" or "Array"
  }

  return "";
}

function createFunctionString(value: Function, _ctx: ConsoleContext): string {
  // Might be Function/AsyncFunction/GeneratorFunction
  const cstrName = Object.getPrototypeOf(value).constructor.name;
  if (value.name && value.name !== "anonymous") {
    // from MDN spec
    return `[${cstrName}: ${value.name}]`;
  }
  return `[${cstrName}]`;
}

interface IterablePrintConfig<T> {
  typeName: string;
  displayName: string;
  delims: [string, string];
  entryHandler: (
    entry: T,
    ctx: ConsoleContext,
    level: number,
    maxLevel: number
  ) => string;
}

function createIterableString<T>(
  value: Iterable<T>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number,
  config: IterablePrintConfig<T>
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

function stringify(
  value: unknown,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      return value;
    case "number":
      // Special handling of -0
      return Object.is(value, -0) ? "-0" : `${value}`;
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
  value: unknown,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      const trunc =
        value.length > STR_ABBREVIATE_SIZE
          ? value.slice(0, STR_ABBREVIATE_SIZE) + "..."
          : value;
      return JSON.stringify(trunc);
    default:
      return stringify(value, ctx, level, maxLevel);
  }
}

function createArrayString(
  value: unknown[],
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig<unknown> = {
    typeName: "Array",
    displayName: "",
    delims: ["[", "]"],
    entryHandler: (el, ctx, level, maxLevel): string =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel),
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createTypedArrayString(
  typedArrayName: string,
  value: TypedArray,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig<unknown> = {
    typeName: typedArrayName,
    displayName: typedArrayName,
    delims: ["[", "]"],
    entryHandler: (el, ctx, level, maxLevel): string =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel),
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createSetString(
  value: Set<unknown>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig<unknown> = {
    typeName: "Set",
    displayName: "Set",
    delims: ["{", "}"],
    entryHandler: (el, ctx, level, maxLevel): string =>
      stringifyWithQuotes(el, ctx, level + 1, maxLevel),
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createMapString(
  value: Map<unknown, unknown>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig<[unknown, unknown]> = {
    typeName: "Map",
    displayName: "Map",
    delims: ["{", "}"],
    entryHandler: (el, ctx, level, maxLevel): string => {
      const [key, val] = el;
      return `${stringifyWithQuotes(
        key,
        ctx,
        level + 1,
        maxLevel
      )} => ${stringifyWithQuotes(val, ctx, level + 1, maxLevel)}`;
    },
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createWeakSetString(): string {
  return "WeakSet { [items unknown] }"; // as seen in Node
}

function createWeakMapString(): string {
  return "WeakMap { [items unknown] }"; // as seen in Node
}

function createDateString(value: Date): string {
  // without quotes, ISO format
  return value.toISOString();
}

function createRegExpString(value: RegExp): string {
  return value.toString();
}

/* eslint-disable @typescript-eslint/ban-types */

function createStringWrapperString(value: String): string {
  return `[String: "${value.toString()}"]`;
}

function createBooleanWrapperString(value: Boolean): string {
  return `[Boolean: ${value.toString()}]`;
}

function createNumberWrapperString(value: Number): string {
  return `[Number: ${value.toString()}]`;
}

/* eslint-enable @typescript-eslint/ban-types */

// TODO: Promise, requires v8 bindings to get info
// TODO: Proxy

function createRawObjectString(
  value: { [key: string]: unknown },
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  if (level >= maxLevel) {
    return "[Object]";
  }
  ctx.add(value);

  let baseString = "";

  let shouldShowDisplayName = false;
  // @ts-ignore
  let displayName = value[Symbol.toStringTag];
  if (!displayName) {
    displayName = getClassInstanceName(value);
  }
  if (displayName && displayName !== "Object" && displayName !== "anonymous") {
    shouldShowDisplayName = true;
  }

  const entries: string[] = [];
  const stringKeys = Object.keys(value);
  const symbolKeys = Object.getOwnPropertySymbols(value);

  for (const key of stringKeys) {
    entries.push(
      `${key}: ${stringifyWithQuotes(value[key], ctx, level + 1, maxLevel)}`
    );
  }
  for (const key of symbolKeys) {
    entries.push(
      `${key.toString()}: ${stringifyWithQuotes(
        // @ts-ignore
        value[key],
        ctx,
        level + 1,
        maxLevel
      )}`
    );
  }

  const totalLength = entries.length + level + entries.join("").length;

  ctx.delete(value);

  if (entries.length === 0) {
    baseString = "{}";
  } else if (totalLength > LINE_BREAKING_LENGTH) {
    const entryIndent = " ".repeat(level + 1);
    const closingIndent = " ".repeat(level);
    baseString = `{\n${entryIndent}${entries.join(
      `,\n${entryIndent}`
    )}\n${closingIndent}}`;
  } else {
    baseString = `{ ${entries.join(", ")} }`;
  }

  if (shouldShowDisplayName) {
    baseString = `${displayName} ${baseString}`;
  }

  return baseString;
}

function createObjectString(
  value: {},
  ...args: [ConsoleContext, number, number]
): string {
  if (customInspect in value && typeof value[customInspect] === "function") {
    try {
      return String(value[customInspect]!());
    } catch {}
  }
  if (value instanceof Error) {
    return String(value.stack);
  } else if (Array.isArray(value)) {
    return createArrayString(value, ...args);
  } else if (value instanceof Number) {
    return createNumberWrapperString(value);
  } else if (value instanceof Boolean) {
    return createBooleanWrapperString(value);
  } else if (value instanceof String) {
    return createStringWrapperString(value);
  } else if (value instanceof RegExp) {
    return createRegExpString(value);
  } else if (value instanceof Date) {
    return createDateString(value);
  } else if (value instanceof Set) {
    return createSetString(value, ...args);
  } else if (value instanceof Map) {
    return createMapString(value, ...args);
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

export function stringifyArgs(
  args: unknown[],
  { depth = DEFAULT_MAX_DEPTH, indentLevel = 0 }: InspectOptions = {}
): string {
  const first = args[0];
  let a = 0;
  let str = "";
  let join = "";

  if (typeof first === "string") {
    let tempStr: string;
    let lastPos = 0;

    for (let i = 0; i < first.length - 1; i++) {
      if (first.charCodeAt(i) === CHAR_PERCENT) {
        const nextChar = first.charCodeAt(++i);
        if (a + 1 !== args.length) {
          switch (nextChar) {
            case CHAR_LOWERCASE_S:
              // format as a string
              tempStr = String(args[++a]);
              break;
            case CHAR_LOWERCASE_D:
            case CHAR_LOWERCASE_I:
              // format as an integer
              const tempInteger = args[++a];
              if (typeof tempInteger === "bigint") {
                tempStr = `${tempInteger}n`;
              } else if (typeof tempInteger === "symbol") {
                tempStr = "NaN";
              } else {
                tempStr = `${parseInt(String(tempInteger), 10)}`;
              }
              break;
            case CHAR_LOWERCASE_F:
              // format as a floating point value
              const tempFloat = args[++a];
              if (typeof tempFloat === "symbol") {
                tempStr = "NaN";
              } else {
                tempStr = `${parseFloat(String(tempFloat))}`;
              }
              break;
            case CHAR_LOWERCASE_O:
            case CHAR_UPPERCASE_O:
              // format as an object
              tempStr = stringify(args[++a], new Set<unknown>(), 0, depth);
              break;
            case CHAR_PERCENT:
              str += first.slice(lastPos, i);
              lastPos = i + 1;
              continue;
            case CHAR_LOWERCASE_C:
              // TODO: applies CSS style rules to the output string as specified
              continue;
            default:
              // any other character is not a correct placeholder
              continue;
          }

          if (lastPos !== i - 1) {
            str += first.slice(lastPos, i - 1);
          }

          str += tempStr;
          lastPos = i + 1;
        } else if (nextChar === CHAR_PERCENT) {
          str += first.slice(lastPos, i);
          lastPos = i + 1;
        }
      }
    }

    if (lastPos !== 0) {
      a++;
      join = " ";
      if (lastPos < first.length) {
        str += first.slice(lastPos);
      }
    }
  }

  while (a < args.length) {
    const value = args[a];
    str += join;
    if (typeof value === "string") {
      str += value;
    } else {
      // use default maximum depth for null or undefined argument
      str += stringify(value, new Set<unknown>(), 0, depth);
    }
    join = " ";
    a++;
  }

  if (indentLevel > 0) {
    const groupIndent = " ".repeat(indentLevel);
    if (str.indexOf("\n") !== -1) {
      str = str.replace(/\n/g, `\n${groupIndent}`);
    }
    str = groupIndent + str;
  }

  return str;
}

type PrintFunc = (x: string, isErr?: boolean) => void;

const countMap = new Map<string, number>();
const timerMap = new Map<string, number>();
const isConsoleInstance = Symbol("isConsoleInstance");

export class Console {
  #printFunc: PrintFunc;
  indentLevel: number;
  [isConsoleInstance] = false;

  constructor(printFunc: PrintFunc) {
    this.#printFunc = printFunc;
    this.indentLevel = 0;
    this[isConsoleInstance] = true;

    // ref https://console.spec.whatwg.org/#console-namespace
    // For historical web-compatibility reasons, the namespace object for
    // console must have as its [[Prototype]] an empty object, created as if
    // by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
    const console = Object.create({}) as Console;
    Object.assign(console, this);
    return console;
  }

  log = (...args: unknown[]): void => {
    this.#printFunc(
      stringifyArgs(args, {
        indentLevel: this.indentLevel,
      }) + "\n",
      false
    );
  };

  debug = this.log;
  info = this.log;

  dir = (obj: unknown, options: InspectOptions = {}): void => {
    this.#printFunc(stringifyArgs([obj], options) + "\n", false);
  };

  dirxml = this.dir;

  warn = (...args: unknown[]): void => {
    this.#printFunc(
      stringifyArgs(args, {
        indentLevel: this.indentLevel,
      }) + "\n",
      true
    );
  };

  error = this.warn;

  assert = (condition = false, ...args: unknown[]): void => {
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

  table = (data: unknown, properties?: string[]): void => {
    if (properties !== undefined && !Array.isArray(properties)) {
      throw new Error(
        "The 'properties' argument must be of type Array. " +
          "Received type string"
      );
    }

    if (data === null || typeof data !== "object") {
      return this.log(data);
    }

    const objectValues: { [key: string]: string[] } = {};
    const indexKeys: string[] = [];
    const values: string[] = [];

    const stringifyValue = (value: unknown): string =>
      stringifyWithQuotes(value, new Set<unknown>(), 0, 1);
    const toTable = (header: string[], body: string[][]): void =>
      this.log(cliTable(header, body));
    const createColumn = (value: unknown, shift?: number): string[] => [
      ...(shift ? [...new Array(shift)].map((): string => "") : []),
      stringifyValue(value),
    ];

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let resultData: any;
    const isSet = data instanceof Set;
    const isMap = data instanceof Map;
    const valuesKey = "Values";
    const indexKey = isSet || isMap ? "(iteration index)" : "(index)";

    if (data instanceof Set) {
      resultData = [...data];
    } else if (data instanceof Map) {
      let idx = 0;
      resultData = {};

      data.forEach((v: unknown, k: unknown): void => {
        resultData[idx] = { Key: k, Values: v };
        idx++;
      });
    } else {
      resultData = data!;
    }

    Object.keys(resultData).forEach((k, idx): void => {
      const value: unknown = resultData[k]!;

      if (value !== null && typeof value === "object") {
        Object.entries(value as { [key: string]: unknown }).forEach(
          ([k, v]): void => {
            if (properties && !properties.includes(k)) {
              return;
            }

            if (objectValues[k]) {
              objectValues[k].push(stringifyValue(v));
            } else {
              objectValues[k] = createColumn(v, idx);
            }
          }
        );

        values.push("");
      } else {
        values.push(stringifyValue(value));
      }

      indexKeys.push(k);
    });

    const headerKeys = Object.keys(objectValues);
    const bodyValues = Object.values(objectValues);
    const header = [
      indexKey,
      ...(properties || [
        ...headerKeys,
        !isMap && values.length > 0 && valuesKey,
      ]),
    ].filter(Boolean) as string[];
    const body = [indexKeys, ...bodyValues, values];

    toTable(header, body);
  };

  time = (label = "default"): void => {
    label = String(label);

    if (timerMap.has(label)) {
      this.warn(`Timer '${label}' already exists`);
      return;
    }

    timerMap.set(label, Date.now());
  };

  timeLog = (label = "default", ...args: unknown[]): void => {
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

  group = (...label: unknown[]): void => {
    if (label.length > 0) {
      this.log(...label);
    }
    this.indentLevel += 2;
  };

  groupCollapsed = this.group;

  groupEnd = (): void => {
    if (this.indentLevel > 0) {
      this.indentLevel -= 2;
    }
  };

  clear = (): void => {
    this.indentLevel = 0;
    cursorTo(stdout, 0, 0);
    clearScreenDown(stdout);
  };

  trace = (...args: unknown[]): void => {
    const message = stringifyArgs(args, { indentLevel: 0 });
    const err = {
      name: "Trace",
      message,
    };
    // @ts-ignore
    Error.captureStackTrace(err, this.trace);
    this.error((err as Error).stack);
  };

  static [Symbol.hasInstance](instance: Console): boolean {
    return instance[isConsoleInstance];
  }
}

export const customInspect = Symbol.for("Deno.customInspect");

export function inspect(
  value: unknown,
  { depth = DEFAULT_MAX_DEPTH }: InspectOptions = {}
): string {
  if (typeof value === "string") {
    return value;
  } else {
    return stringify(value, new Set<unknown>(), 0, depth);
  }
}

// Expose these fields to internalObject for tests.
exposeForTest("Console", Console);
exposeForTest("stringifyArgs", stringifyArgs);
