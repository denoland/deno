// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { isInvalidDate, isTypedArray, TypedArray } from "./util.ts";
import { cliTable } from "./console_table.ts";
import { exposeForTest } from "../internals.ts";
import { PromiseState } from "./promise.ts";
import {
  stripColor,
  yellow,
  dim,
  cyan,
  red,
  green,
  magenta,
  bold,
} from "../colors.ts";

type ConsoleContext = Set<unknown>;
type InspectOptions = Partial<{
  depth: number;
  indentLevel: number;
}>;

const DEFAULT_INDENT = "  "; // Default indent string

const DEFAULT_MAX_DEPTH = 4; // Default depth of logging nested objects
const LINE_BREAKING_LENGTH = 80;
const MAX_ITERABLE_LENGTH = 100;
const MIN_GROUP_LENGTH = 6;
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

const PROMISE_STRING_BASE_LENGTH = 12;

export class CSI {
  static kClear = "\x1b[1;1H";
  static kClearScreenDown = "\x1b[0J";
}

/* eslint-disable @typescript-eslint/no-use-before-define */

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
    maxLevel: number,
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
  maxLevel: number,
  config: IterablePrintConfig<T>
): string {
  if (level >= maxLevel) {
    return cyan(`[${config.typeName}]`);
  }
  ctx.add(value);

  const entries: string[] = [];

  const iter = value.entries();
  let entriesLength = 0;
  const next = (): IteratorResult<[unknown, T], unknown> => {
    return iter.next();
  };
  for (const el of iter) {
    if (entriesLength < MAX_ITERABLE_LENGTH) {
      entries.push(
        config.entryHandler(el, ctx, level + 1, maxLevel, next.bind(iter))
      );
    }
    entriesLength++;
  }
  ctx.delete(value);

  if (entriesLength > MAX_ITERABLE_LENGTH) {
    const nmore = entriesLength - MAX_ITERABLE_LENGTH;
    entries.push(`... ${nmore} more items`);
  }

  const iPrefix = `${config.displayName ? config.displayName + " " : ""}`;

  const initIndentation = `\n${DEFAULT_INDENT.repeat(level + 1)}`;
  const entryIndentation = `,\n${DEFAULT_INDENT.repeat(level + 1)}`;
  const closingIndentation = `\n${DEFAULT_INDENT.repeat(level)}`;

  let iContent: string;
  if (config.group && entries.length > MIN_GROUP_LENGTH) {
    const groups = groupEntries(entries, level, value);
    iContent = `${initIndentation}${groups.join(
      entryIndentation
    )}${closingIndentation}`;
  } else {
    iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
    if (stripColor(iContent).length > LINE_BREAKING_LENGTH) {
      iContent = `${initIndentation}${entries.join(
        entryIndentation
      )}${closingIndentation}`;
    }
  }

  return `${iPrefix}${config.delims[0]}${iContent}${config.delims[1]}`;
}

// Ported from Node.js
// Copyright Node.js contributors. All rights reserved.
function groupEntries<T>(
  entries: string[],
  level: number,
  value: Iterable<T>
): string[] {
  let totalLength = 0;
  let maxLength = 0;
  let entriesLength = entries.length;
  if (MAX_ITERABLE_LENGTH < entriesLength) {
    // This makes sure the "... n more items" part is not taken into account.
    entriesLength--;
  }
  const separatorSpace = 2; // Add 1 for the space and 1 for the separator.
  const dataLen = new Array(entriesLength);
  // Calculate the total length of all output entries and the individual max
  // entries length of all output entries.
  // IN PROGRESS: Colors are being taken into account.
  for (let i = 0; i < entriesLength; i++) {
    // Taking colors into account: removing the ANSI color
    // codes from the string before measuring its length
    const len = stripColor(entries[i]).length;
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
    actualMax * 3 + (level + 1) < LINE_BREAKING_LENGTH &&
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
      Math.floor((LINE_BREAKING_LENGTH - (level + 1)) / actualMax),
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
    let order: "padStart" | "padEnd" = "padStart";
    if (value !== undefined) {
      for (let i = 0; i < entries.length; i++) {
        /* eslint-disable @typescript-eslint/no-explicit-any */
        if (
          typeof (value as any)[i] !== "number" &&
          typeof (value as any)[i] !== "bigint"
        ) {
          order = "padEnd";
          break;
        }
        /* eslint-enable */
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
    if (MAX_ITERABLE_LENGTH < entries.length) {
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
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      return value;
    case "number": // Numbers are yellow
      // Special handling of -0
      return yellow(Object.is(value, -0) ? "-0" : `${value}`);
    case "boolean": // booleans are yellow
      return yellow(String(value));
    case "undefined": // undefined is dim
      return dim(String(value));
    case "symbol": // Symbols are green
      return green(String(value));
    case "bigint": // Bigints are yellow
      return yellow(`${value}n`);
    case "function": // Function string is cyan
      return cyan(createFunctionString(value as Function, ctx));
    case "object": // null is bold
      if (value === null) {
        return bold("null");
      }

      if (ctx.has(value)) {
        // Circular string is cyan
        return cyan("[Circular]");
      }

      return createObjectString(value, ctx, level, maxLevel);
    default:
      // Not implemented is red
      return red("[Not Implemented]");
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
      return green(`"${trunc}"`); // Quoted strings are green
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
    entryHandler: (entry, ctx, level, maxLevel, next): string => {
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
        return dim(`<${emptyItems} empty item${ending}>`);
      } else {
        return stringifyWithQuotes(val, ctx, level, maxLevel);
      }
    },
    group: true,
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
  const valueLength = value.length;
  const printConfig: IterablePrintConfig<unknown> = {
    typeName: typedArrayName,
    displayName: `${typedArrayName}(${valueLength})`,
    delims: ["[", "]"],
    entryHandler: (entry, ctx, level, maxLevel): string => {
      const [_, val] = entry;
      return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
    },
    group: true,
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
    entryHandler: (entry, ctx, level, maxLevel): string => {
      const [_, val] = entry;
      return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
    },
    group: false,
  };
  return createIterableString(value, ctx, level, maxLevel, printConfig);
}

function createMapString(
  value: Map<unknown, unknown>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const printConfig: IterablePrintConfig<[unknown]> = {
    typeName: "Map",
    displayName: "Map",
    delims: ["{", "}"],
    entryHandler: (entry, ctx, level, maxLevel): string => {
      const [key, val] = entry;
      return `${stringifyWithQuotes(
        key,
        ctx,
        level + 1,
        maxLevel
      )} => ${stringifyWithQuotes(val, ctx, level + 1, maxLevel)}`;
    },
    group: false,
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return createIterableString(value as any, ctx, level, maxLevel, printConfig);
}

function createWeakSetString(): string {
  return `WeakSet { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
}

function createWeakMapString(): string {
  return `WeakMap { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
}

function createDateString(value: Date): string {
  // without quotes, ISO format, in magenta like before
  return magenta(isInvalidDate(value) ? "Invalid Date" : value.toISOString());
}

function createRegExpString(value: RegExp): string {
  return red(value.toString()); // RegExps are red
}

/* eslint-disable @typescript-eslint/ban-types */

function createStringWrapperString(value: String): string {
  return cyan(`[String: "${value.toString()}"]`); // wrappers are in cyan
}

function createBooleanWrapperString(value: Boolean): string {
  return cyan(`[Boolean: ${value.toString()}]`); // wrappers are in cyan
}

function createNumberWrapperString(value: Number): string {
  return cyan(`[Number: ${value.toString()}]`); // wrappers are in cyan
}

/* eslint-enable @typescript-eslint/ban-types */

function createPromiseString(
  value: Promise<unknown>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const [state, result] = Deno.core.getPromiseDetails(value);

  if (state === PromiseState.Pending) {
    return `Promise { ${cyan("<pending>")} }`;
  }

  const prefix =
    state === PromiseState.Fulfilled ? "" : `${red("<rejected>")} `;

  const str = `${prefix}${stringifyWithQuotes(
    result,
    ctx,
    level + 1,
    maxLevel
  )}`;

  if (str.length + PROMISE_STRING_BASE_LENGTH > LINE_BREAKING_LENGTH) {
    return `Promise {\n${DEFAULT_INDENT.repeat(level + 1)}${str}\n}`;
  }

  return `Promise { ${str} }`;
}

// TODO: Proxy

function createRawObjectString(
  value: Record<string, unknown>,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  if (level >= maxLevel) {
    return cyan("[Object]"); // wrappers are in cyan
  }
  ctx.add(value);

  let baseString = "";

  let shouldShowDisplayName = false;
  let displayName = (value as { [Symbol.toStringTag]: string })[
    Symbol.toStringTag
  ];
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
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        value[key as any],
        ctx,
        level + 1,
        maxLevel
      )}`
    );
  }
  // Making sure color codes are ignored when calculating the total length
  const totalLength =
    entries.length + level + stripColor(entries.join("")).length;

  ctx.delete(value);

  if (entries.length === 0) {
    baseString = "{}";
  } else if (totalLength > LINE_BREAKING_LENGTH) {
    const entryIndent = DEFAULT_INDENT.repeat(level + 1);
    const closingIndent = DEFAULT_INDENT.repeat(level);
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
    const groupIndent = DEFAULT_INDENT.repeat(indentLevel);
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
    const indexKey = isSet || isMap ? "(iter idx)" : "(idx)";

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

    let hasPrimitives = false;
    Object.keys(resultData).forEach((k, idx): void => {
      const value: unknown = resultData[k]!;
      const primitive =
        value === null ||
        (typeof value !== "function" && typeof value !== "object");
      if (properties === undefined && primitive) {
        hasPrimitives = true;
        values.push(stringifyValue(value));
      } else {
        const valueObj = (value as { [key: string]: unknown }) || {};
        const keys = properties || Object.keys(valueObj);
        for (const k of keys) {
          if (primitive || !valueObj.hasOwnProperty(k)) {
            if (objectValues[k]) {
              // fill with blanks for idx to avoid misplacing from later values
              objectValues[k].push("");
            }
          } else {
            if (objectValues[k]) {
              objectValues[k].push(stringifyValue(valueObj[k]));
            } else {
              objectValues[k] = createColumn(valueObj[k], idx);
            }
          }
        }
        values.push("");
      }

      indexKeys.push(k);
    });

    const headerKeys = Object.keys(objectValues);
    const bodyValues = Object.values(objectValues);
    const header = [
      indexKey,
      ...(properties || [...headerKeys, !isMap && hasPrimitives && valuesKey]),
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
    this.#printFunc(CSI.kClear, false);
    this.#printFunc(CSI.kClearScreenDown, false);
  };

  trace = (...args: unknown[]): void => {
    const message = stringifyArgs(args, { indentLevel: 0 });
    const err = {
      name: "Trace",
      message,
    };
    Error.captureStackTrace(err, this.trace);
    this.error((err as Error).stack);
  };

  static [Symbol.hasInstance](instance: Console): boolean {
    return instance[isConsoleInstance];
  }
}

export const customInspect = Symbol("Deno.symbols.customInspect");

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
