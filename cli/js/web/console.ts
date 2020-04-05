// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { isTypedArray, TypedArray } from "./util.ts";
import { TextEncoder } from "./text_encoding.ts";
import { File, stdout } from "../files.ts";
import { cliTable } from "./console_table.ts";
import { exposeForTest } from "../internals.ts";
import { PromiseState } from "./promise.ts";

type ConsoleContext = Set<unknown>;

/**
 * @property `depth` Default depth of logging nested objects
 * @property `indentLevel` Indentation level.
 * @property `lineBreakLength` The maximum length until input values is split into multiple lines.
 * @property `maxIterableLength` The maximum number of elements in an iterable, which would be printed.
 * @property `strAbbreviateSize` The maximum length of the string until printed in abbreviate form.
 * @property `minGroupLength` The minimum number of elements in an array until elements will be printed in groups.
 */
type InspectOptions = Partial<PrintConfig>;
type PrintConfig = {
  depth: number;
  indentLevel: number;
  lineBreakLength: number;
  maxIterableLength: number;
  strAbbreviateSize: number;
  minGroupLength: number;
};

// Char codes
const CHAR_PERCENT = 37; /* % */
const CHAR_LOWERCASE_S = 115; /* s */
const CHAR_LOWERCASE_D = 100; /* d */
const CHAR_LOWERCASE_I = 105; /* i */
const CHAR_LOWERCASE_F = 102; /* f */
const CHAR_LOWERCASE_O = 111; /* o */
const CHAR_UPPERCASE_O = 79; /* O */
const CHAR_LOWERCASE_C = 99; /* c */

const PROMISE_STRING_BASE_LENGTH = 12;

const DEFAULT_INSPECT_OPTIONS: PrintConfig = {
  depth: 4, // Default depth of logging nested objects
  indentLevel: 0,
  lineBreakLength: 80,
  maxIterableLength: 100,
  strAbbreviateSize: 100,
  minGroupLength: 6,
};

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
    entry: [unknown, T],
    ctx: ConsoleContext,
    level: number,
    config: PrintConfig,
    next: () => IteratorResult<[unknown, T], unknown>
  ) => string;
  group: boolean;
}
type IterableEntries<T> = Iterable<T> & {
  entries(): IterableIterator<[unknown, T]>;
};
function createIterableString<T>(
  value: IterableEntries<T>,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig,
  iterableConfig: IterablePrintConfig<T>
): string {
  if (level >= config.depth) {
    return `[${iterableConfig.typeName}]`;
  }
  ctx.add(value);

  const entries: string[] = [];

  const iter = value.entries();
  let entriesLength = 0;
  const next = (): IteratorResult<[unknown, T], unknown> => {
    return iter.next();
  };
  for (const el of iter) {
    if (entriesLength < config.maxIterableLength) {
      entries.push(
        iterableConfig.entryHandler(el, ctx, level + 1, config, next.bind(iter))
      );
    }
    entriesLength++;
  }
  ctx.delete(value);

  if (entriesLength > config.maxIterableLength) {
    const nmore = entriesLength - config.maxIterableLength;
    entries.push(`... ${nmore} more items`);
  }

  const iPrefix = `${
    iterableConfig.displayName ? iterableConfig.displayName + " " : ""
  }`;

  let iContent: string;
  if (iterableConfig.group && entries.length > config.minGroupLength) {
    const groups = groupEntries(entries, level, value, config);
    const initIndentation = `\n${"  ".repeat(level + 1)}`;
    const entryIndetation = `,\n${"  ".repeat(level + 1)}`;
    const closingIndentation = `\n${"  ".repeat(level)}`;

    iContent = `${initIndentation}${groups.join(
      entryIndetation
    )}${closingIndentation}`;
  } else {
    iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
    if (iContent.length > config.lineBreakLength) {
      const initIndentation = `\n${" ".repeat(level + 1)}`;
      const entryIndetation = `,\n${" ".repeat(level + 1)}`;
      const closingIndentation = `\n`;

      iContent = `${initIndentation}${entries.join(
        entryIndetation
      )}${closingIndentation}`;
    }
  }

  return `${iPrefix}${iterableConfig.delims[0]}${iContent}${iterableConfig.delims[1]}`;
}

// Ported from Node.js
// Copyright Node.js contributors. All rights reserved.
function groupEntries<T>(
  entries: string[],
  level: number,
  value: Iterable<T>,
  config: PrintConfig
): string[] {
  let totalLength = 0;
  let maxLength = 0;
  let entriesLength = entries.length;
  if (config.maxIterableLength < entriesLength) {
    // This makes sure the "... n more items" part is not taken into account.
    entriesLength--;
  }
  const separatorSpace = 2; // Add 1 for the space and 1 for the separator.
  const dataLen = new Array(entriesLength);
  // Calculate the total length of all output entries and the individual max
  // entries length of all output entries. In future colors should be taken
  // here into the account
  for (let i = 0; i < entriesLength; i++) {
    const len = entries[i].length;
    dataLen[i] = len;
    totalLength += len + separatorSpace;
    if (maxLength < len) maxLength = len;
  }
  // Add two to `maxLength` as we add a single whitespace character plus a comma
  // in-between two entries.
  const actualMax = maxLength + separatorSpace;
  // Check if at least three entries fit next to each other and prevent grouping
  // of arrays that contains entries of very different length (i.e., if a single
  // entry is longer than 1/5 of all other entries combined). Otherwise the
  // space in-between small entries would be enormous.
  if (
    actualMax * 3 + (level + 1) < config.lineBreakLength &&
    (totalLength / actualMax > 5 || maxLength <= 6)
  ) {
    const approxCharHeights = 2.5;
    const averageBias = Math.sqrt(actualMax - totalLength / entries.length);
    const biasedMax = Math.max(actualMax - 3 - averageBias, 1);
    // Dynamically check how many columns seem possible.
    const columns = Math.min(
      // Ideally a square should be drawn. We expect a character to be about 2.5
      // times as high as wide. This is the area formula to calculate a square
      // which contains n rectangles of size `actualMax * approxCharHeights`.
      // Divide that by `actualMax` to receive the correct number of columns.
      // The added bias increases the columns for short entries.
      Math.round(
        Math.sqrt(approxCharHeights * biasedMax * entriesLength) / biasedMax
      ),
      // Do not exceed the breakLength.
      Math.floor((config.lineBreakLength - (level + 1)) / actualMax),
      // Limit the columns to a maximum of fifteen.
      15
    );
    // Return with the original output if no grouping should happen.
    if (columns <= 1) {
      return entries;
    }
    const tmp = [];
    const maxLineLength = [];
    for (let i = 0; i < columns; i++) {
      let lineMaxLength = 0;
      for (let j = i; j < entries.length; j += columns) {
        if (dataLen[j] > lineMaxLength) lineMaxLength = dataLen[j];
      }
      lineMaxLength += separatorSpace;
      maxLineLength[i] = lineMaxLength;
    }
    let order = "padStart";
    if (value !== undefined) {
      for (let i = 0; i < entries.length; i++) {
        //@ts-ignore
        if (typeof value[i] !== "number" && typeof value[i] !== "bigint") {
          order = "padEnd";
          break;
        }
      }
    }
    // Each iteration creates a single line of grouped entries.
    for (let i = 0; i < entriesLength; i += columns) {
      // The last lines may contain less entries than columns.
      const max = Math.min(i + columns, entriesLength);
      let str = "";
      let j = i;
      for (; j < max - 1; j++) {
        // In future, colors should be taken here into the account
        const padding = maxLineLength[j - i];
        //@ts-ignore
        str += `${entries[j]}, `[order](padding, " ");
      }
      if (order === "padStart") {
        const padding =
          maxLineLength[j - i] +
          entries[j].length -
          dataLen[j] -
          separatorSpace;
        str += entries[j].padStart(padding, " ");
      } else {
        str += entries[j];
      }
      tmp.push(str);
    }
    if (config.maxIterableLength < entries.length) {
      tmp.push(entries[entriesLength]);
    }
    entries = tmp;
  }
  return entries;
}

function stringify(
  value: unknown,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
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

      return createObjectString(value, ctx, level, config);
    default:
      return "[Not Implemented]";
  }
}

// Print strings when they are inside of arrays or objects with quotes
function stringifyWithQuotes(
  value: unknown,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  switch (typeof value) {
    case "string":
      const trunc =
        value.length > config.strAbbreviateSize
          ? value.slice(0, config.strAbbreviateSize) + "..."
          : value;
      return JSON.stringify(trunc);
    default:
      return stringify(value, ctx, level, config);
  }
}

function createArrayString(
  value: unknown[],
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  const iterablePrintConfig: IterablePrintConfig<unknown> = {
    typeName: "Array",
    displayName: "",
    delims: ["[", "]"],
    entryHandler: (entry, ctx, level, config, next): string => {
      const [index, val] = entry as [number, unknown];
      let i = index;
      if (!value.hasOwnProperty(i)) {
        i++;
        while (!value.hasOwnProperty(i) && i < value.length) {
          next();
          i++;
        }
        const emptyItems = i - index;
        const ending = emptyItems > 1 ? "s" : "";
        return `<${emptyItems} empty item${ending}>`;
      } else {
        return stringifyWithQuotes(val, ctx, level + 1, config);
      }
    },
    group: true,
  };
  return createIterableString(value, ctx, level, config, iterablePrintConfig);
}

function createTypedArrayString(
  typedArrayName: string,
  value: TypedArray,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  const valueLength = value.length;
  const iterablePrintConfig: IterablePrintConfig<unknown> = {
    typeName: typedArrayName,
    displayName: `${typedArrayName}(${valueLength})`,
    delims: ["[", "]"],
    entryHandler: (entry, ctx, level, config): string => {
      const [_, val] = entry;
      return stringifyWithQuotes(val, ctx, level + 1, config);
    },
    group: true,
  };
  return createIterableString(value, ctx, level, config, iterablePrintConfig);
}

function createSetString(
  value: Set<unknown>,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  const iterablePrintConfig: IterablePrintConfig<unknown> = {
    typeName: "Set",
    displayName: "Set",
    delims: ["{", "}"],
    entryHandler: (entry, ctx, level, config): string => {
      const [_, val] = entry;
      return stringifyWithQuotes(val, ctx, level + 1, config);
    },
    group: false,
  };
  return createIterableString(value, ctx, level, config, iterablePrintConfig);
}

function createMapString(
  value: Map<unknown, unknown>,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  const iterablePrintConfig: IterablePrintConfig<[unknown]> = {
    typeName: "Map",
    displayName: "Map",
    delims: ["{", "}"],
    entryHandler: (entry, ctx, level, config): string => {
      const [key, val] = entry;
      return `${stringifyWithQuotes(
        key,
        ctx,
        level + 1,
        config
      )} => ${stringifyWithQuotes(val, ctx, level + 1, config)}`;
    },
    group: false,
  };
  //@ts-ignore
  return createIterableString(value, ctx, level, config, iterablePrintConfig);
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

function createPromiseString(
  value: Promise<unknown>,
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  const [state, result] = Deno.core.getPromiseDetails(value);

  if (state === PromiseState.Pending) {
    return "Promise { <pending> }";
  }

  const prefix = state === PromiseState.Fulfilled ? "" : "<rejected> ";

  const str = `${prefix}${stringifyWithQuotes(result, ctx, level + 1, config)}`;

  if (str.length + PROMISE_STRING_BASE_LENGTH > config.lineBreakLength) {
    return `Promise {\n${" ".repeat(level + 1)}${str}\n}`;
  }

  return `Promise { ${str} }`;
}

// TODO: Proxy

function createRawObjectString(
  value: { [key: string]: unknown },
  ctx: ConsoleContext,
  level: number,
  config: PrintConfig
): string {
  if (level >= config.depth) {
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
      `${key}: ${stringifyWithQuotes(value[key], ctx, level + 1, config)}`
    );
  }
  for (const key of symbolKeys) {
    entries.push(
      `${key.toString()}: ${stringifyWithQuotes(
        // @ts-ignore
        value[key],
        ctx,
        level + 1,
        config
      )}`
    );
  }

  const totalLength = entries.length + level + entries.join("").length;

  ctx.delete(value);

  if (entries.length === 0) {
    baseString = "{}";
  } else if (totalLength > config.lineBreakLength) {
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
  ...args: [ConsoleContext, number, PrintConfig]
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
  } else if (value instanceof Promise) {
    return createPromiseString(value, ...args);
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
  options: InspectOptions = {}
): string {
  const opts = { ...DEFAULT_INSPECT_OPTIONS, ...options };
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
              tempStr = stringify(args[++a], new Set<unknown>(), 0, opts);
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
      str += stringify(value, new Set<unknown>(), 0, opts);
    }
    join = " ";
    a++;
  }

  if (opts.indentLevel > 0) {
    const groupIndent = " ".repeat(opts.indentLevel);
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
      stringifyWithQuotes(value, new Set<unknown>(), 0, {
        ...DEFAULT_INSPECT_OPTIONS,
        depth: 1,
      });
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

export function inspect(value: unknown, options: InspectOptions = {}): string {
  const opts: PrintConfig = { ...DEFAULT_INSPECT_OPTIONS, ...options };
  if (typeof value === "string") {
    return value;
  } else {
    return stringify(value, new Set<unknown>(), 0, opts);
  }
}

// Expose these fields to internalObject for tests.
exposeForTest("Console", Console);
exposeForTest("stringifyArgs", stringifyArgs);
