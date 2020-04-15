System.register(
  "$deno$/web/console.ts",
  [
    "$deno$/web/util.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/files.ts",
    "$deno$/web/console_table.ts",
    "$deno$/internals.ts",
    "$deno$/web/promise.ts",
  ],
  function (exports_35, context_35) {
    "use strict";
    let _a,
      util_ts_6,
      text_encoding_ts_4,
      files_ts_1,
      console_table_ts_1,
      internals_ts_2,
      promise_ts_1,
      DEFAULT_MAX_DEPTH,
      LINE_BREAKING_LENGTH,
      MAX_ITERABLE_LENGTH,
      MIN_GROUP_LENGTH,
      STR_ABBREVIATE_SIZE,
      CHAR_PERCENT,
      CHAR_LOWERCASE_S,
      CHAR_LOWERCASE_D,
      CHAR_LOWERCASE_I,
      CHAR_LOWERCASE_F,
      CHAR_LOWERCASE_O,
      CHAR_UPPERCASE_O,
      CHAR_LOWERCASE_C,
      PROMISE_STRING_BASE_LENGTH,
      CSI,
      countMap,
      timerMap,
      isConsoleInstance,
      Console,
      customInspect;
    const __moduleName = context_35 && context_35.id;
    /* eslint-disable @typescript-eslint/no-use-before-define */
    function cursorTo(stream, _x, _y) {
      const uint8 = new text_encoding_ts_4.TextEncoder().encode(CSI.kClear);
      stream.writeSync(uint8);
    }
    function clearScreenDown(stream) {
      const uint8 = new text_encoding_ts_4.TextEncoder().encode(
        CSI.kClearScreenDown
      );
      stream.writeSync(uint8);
    }
    function getClassInstanceName(instance) {
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
    function createFunctionString(value, _ctx) {
      // Might be Function/AsyncFunction/GeneratorFunction
      const cstrName = Object.getPrototypeOf(value).constructor.name;
      if (value.name && value.name !== "anonymous") {
        // from MDN spec
        return `[${cstrName}: ${value.name}]`;
      }
      return `[${cstrName}]`;
    }
    function createIterableString(value, ctx, level, maxLevel, config) {
      if (level >= maxLevel) {
        return `[${config.typeName}]`;
      }
      ctx.add(value);
      const entries = [];
      const iter = value.entries();
      let entriesLength = 0;
      const next = () => {
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
      let iContent;
      if (config.group && entries.length > MIN_GROUP_LENGTH) {
        const groups = groupEntries(entries, level, value);
        const initIndentation = `\n${"  ".repeat(level + 1)}`;
        const entryIndetation = `,\n${"  ".repeat(level + 1)}`;
        const closingIndentation = `\n${"  ".repeat(level)}`;
        iContent = `${initIndentation}${groups.join(
          entryIndetation
        )}${closingIndentation}`;
      } else {
        iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
        if (iContent.length > LINE_BREAKING_LENGTH) {
          const initIndentation = `\n${" ".repeat(level + 1)}`;
          const entryIndetation = `,\n${" ".repeat(level + 1)}`;
          const closingIndentation = `\n`;
          iContent = `${initIndentation}${entries.join(
            entryIndetation
          )}${closingIndentation}`;
        }
      }
      return `${iPrefix}${config.delims[0]}${iContent}${config.delims[1]}`;
    }
    // Ported from Node.js
    // Copyright Node.js contributors. All rights reserved.
    function groupEntries(entries, level, value) {
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
        if (MAX_ITERABLE_LENGTH < entries.length) {
          tmp.push(entries[entriesLength]);
        }
        entries = tmp;
      }
      return entries;
    }
    function stringify(value, ctx, level, maxLevel) {
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
          return createFunctionString(value, ctx);
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
    function stringifyWithQuotes(value, ctx, level, maxLevel) {
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
    function createArrayString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Array",
        displayName: "",
        delims: ["[", "]"],
        entryHandler: (entry, ctx, level, maxLevel, next) => {
          const [index, val] = entry;
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
            return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
          }
        },
        group: true,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createTypedArrayString(
      typedArrayName,
      value,
      ctx,
      level,
      maxLevel
    ) {
      const valueLength = value.length;
      const printConfig = {
        typeName: typedArrayName,
        displayName: `${typedArrayName}(${valueLength})`,
        delims: ["[", "]"],
        entryHandler: (entry, ctx, level, maxLevel) => {
          const [_, val] = entry;
          return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
        },
        group: true,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createSetString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Set",
        displayName: "Set",
        delims: ["{", "}"],
        entryHandler: (entry, ctx, level, maxLevel) => {
          const [_, val] = entry;
          return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
        },
        group: false,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createMapString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Map",
        displayName: "Map",
        delims: ["{", "}"],
        entryHandler: (entry, ctx, level, maxLevel) => {
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
      //@ts-ignore
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createWeakSetString() {
      return "WeakSet { [items unknown] }"; // as seen in Node
    }
    function createWeakMapString() {
      return "WeakMap { [items unknown] }"; // as seen in Node
    }
    function createDateString(value) {
      // without quotes, ISO format
      return value.toISOString();
    }
    function createRegExpString(value) {
      return value.toString();
    }
    /* eslint-disable @typescript-eslint/ban-types */
    function createStringWrapperString(value) {
      return `[String: "${value.toString()}"]`;
    }
    function createBooleanWrapperString(value) {
      return `[Boolean: ${value.toString()}]`;
    }
    function createNumberWrapperString(value) {
      return `[Number: ${value.toString()}]`;
    }
    /* eslint-enable @typescript-eslint/ban-types */
    function createPromiseString(value, ctx, level, maxLevel) {
      const [state, result] = Deno.core.getPromiseDetails(value);
      if (state === promise_ts_1.PromiseState.Pending) {
        return "Promise { <pending> }";
      }
      const prefix =
        state === promise_ts_1.PromiseState.Fulfilled ? "" : "<rejected> ";
      const str = `${prefix}${stringifyWithQuotes(
        result,
        ctx,
        level + 1,
        maxLevel
      )}`;
      if (str.length + PROMISE_STRING_BASE_LENGTH > LINE_BREAKING_LENGTH) {
        return `Promise {\n${" ".repeat(level + 1)}${str}\n}`;
      }
      return `Promise { ${str} }`;
    }
    // TODO: Proxy
    function createRawObjectString(value, ctx, level, maxLevel) {
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
      if (
        displayName &&
        displayName !== "Object" &&
        displayName !== "anonymous"
      ) {
        shouldShowDisplayName = true;
      }
      const entries = [];
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
    function createObjectString(value, ...args) {
      if (
        customInspect in value &&
        typeof value[customInspect] === "function"
      ) {
        try {
          return String(value[customInspect]());
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
      } else if (util_ts_6.isTypedArray(value)) {
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
    function stringifyArgs(
      args,
      { depth = DEFAULT_MAX_DEPTH, indentLevel = 0 } = {}
    ) {
      const first = args[0];
      let a = 0;
      let str = "";
      let join = "";
      if (typeof first === "string") {
        let tempStr;
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
                  tempStr = stringify(args[++a], new Set(), 0, depth);
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
          str += stringify(value, new Set(), 0, depth);
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
    exports_35("stringifyArgs", stringifyArgs);
    function inspect(value, { depth = DEFAULT_MAX_DEPTH } = {}) {
      if (typeof value === "string") {
        return value;
      } else {
        return stringify(value, new Set(), 0, depth);
      }
    }
    exports_35("inspect", inspect);
    return {
      setters: [
        function (util_ts_6_1) {
          util_ts_6 = util_ts_6_1;
        },
        function (text_encoding_ts_4_1) {
          text_encoding_ts_4 = text_encoding_ts_4_1;
        },
        function (files_ts_1_1) {
          files_ts_1 = files_ts_1_1;
        },
        function (console_table_ts_1_1) {
          console_table_ts_1 = console_table_ts_1_1;
        },
        function (internals_ts_2_1) {
          internals_ts_2 = internals_ts_2_1;
        },
        function (promise_ts_1_1) {
          promise_ts_1 = promise_ts_1_1;
        },
      ],
      execute: function () {
        DEFAULT_MAX_DEPTH = 4; // Default depth of logging nested objects
        LINE_BREAKING_LENGTH = 80;
        MAX_ITERABLE_LENGTH = 100;
        MIN_GROUP_LENGTH = 6;
        STR_ABBREVIATE_SIZE = 100;
        // Char codes
        CHAR_PERCENT = 37; /* % */
        CHAR_LOWERCASE_S = 115; /* s */
        CHAR_LOWERCASE_D = 100; /* d */
        CHAR_LOWERCASE_I = 105; /* i */
        CHAR_LOWERCASE_F = 102; /* f */
        CHAR_LOWERCASE_O = 111; /* o */
        CHAR_UPPERCASE_O = 79; /* O */
        CHAR_LOWERCASE_C = 99; /* c */
        PROMISE_STRING_BASE_LENGTH = 12;
        CSI = class CSI {};
        exports_35("CSI", CSI);
        CSI.kClear = "\x1b[1;1H";
        CSI.kClearScreenDown = "\x1b[0J";
        countMap = new Map();
        timerMap = new Map();
        isConsoleInstance = Symbol("isConsoleInstance");
        Console = class Console {
          constructor(printFunc) {
            this[_a] = false;
            this.log = (...args) => {
              this.#printFunc(
                stringifyArgs(args, {
                  indentLevel: this.indentLevel,
                }) + "\n",
                false
              );
            };
            this.debug = this.log;
            this.info = this.log;
            this.dir = (obj, options = {}) => {
              this.#printFunc(stringifyArgs([obj], options) + "\n", false);
            };
            this.dirxml = this.dir;
            this.warn = (...args) => {
              this.#printFunc(
                stringifyArgs(args, {
                  indentLevel: this.indentLevel,
                }) + "\n",
                true
              );
            };
            this.error = this.warn;
            this.assert = (condition = false, ...args) => {
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
            this.count = (label = "default") => {
              label = String(label);
              if (countMap.has(label)) {
                const current = countMap.get(label) || 0;
                countMap.set(label, current + 1);
              } else {
                countMap.set(label, 1);
              }
              this.info(`${label}: ${countMap.get(label)}`);
            };
            this.countReset = (label = "default") => {
              label = String(label);
              if (countMap.has(label)) {
                countMap.set(label, 0);
              } else {
                this.warn(`Count for '${label}' does not exist`);
              }
            };
            this.table = (data, properties) => {
              if (properties !== undefined && !Array.isArray(properties)) {
                throw new Error(
                  "The 'properties' argument must be of type Array. " +
                    "Received type string"
                );
              }
              if (data === null || typeof data !== "object") {
                return this.log(data);
              }
              const objectValues = {};
              const indexKeys = [];
              const values = [];
              const stringifyValue = (value) =>
                stringifyWithQuotes(value, new Set(), 0, 1);
              const toTable = (header, body) =>
                this.log(console_table_ts_1.cliTable(header, body));
              const createColumn = (value, shift) => [
                ...(shift ? [...new Array(shift)].map(() => "") : []),
                stringifyValue(value),
              ];
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              let resultData;
              const isSet = data instanceof Set;
              const isMap = data instanceof Map;
              const valuesKey = "Values";
              const indexKey = isSet || isMap ? "(iteration index)" : "(index)";
              if (data instanceof Set) {
                resultData = [...data];
              } else if (data instanceof Map) {
                let idx = 0;
                resultData = {};
                data.forEach((v, k) => {
                  resultData[idx] = { Key: k, Values: v };
                  idx++;
                });
              } else {
                resultData = data;
              }
              Object.keys(resultData).forEach((k, idx) => {
                const value = resultData[k];
                if (value !== null && typeof value === "object") {
                  Object.entries(value).forEach(([k, v]) => {
                    if (properties && !properties.includes(k)) {
                      return;
                    }
                    if (objectValues[k]) {
                      objectValues[k].push(stringifyValue(v));
                    } else {
                      objectValues[k] = createColumn(v, idx);
                    }
                  });
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
              ].filter(Boolean);
              const body = [indexKeys, ...bodyValues, values];
              toTable(header, body);
            };
            this.time = (label = "default") => {
              label = String(label);
              if (timerMap.has(label)) {
                this.warn(`Timer '${label}' already exists`);
                return;
              }
              timerMap.set(label, Date.now());
            };
            this.timeLog = (label = "default", ...args) => {
              label = String(label);
              if (!timerMap.has(label)) {
                this.warn(`Timer '${label}' does not exists`);
                return;
              }
              const startTime = timerMap.get(label);
              const duration = Date.now() - startTime;
              this.info(`${label}: ${duration}ms`, ...args);
            };
            this.timeEnd = (label = "default") => {
              label = String(label);
              if (!timerMap.has(label)) {
                this.warn(`Timer '${label}' does not exists`);
                return;
              }
              const startTime = timerMap.get(label);
              timerMap.delete(label);
              const duration = Date.now() - startTime;
              this.info(`${label}: ${duration}ms`);
            };
            this.group = (...label) => {
              if (label.length > 0) {
                this.log(...label);
              }
              this.indentLevel += 2;
            };
            this.groupCollapsed = this.group;
            this.groupEnd = () => {
              if (this.indentLevel > 0) {
                this.indentLevel -= 2;
              }
            };
            this.clear = () => {
              this.indentLevel = 0;
              cursorTo(files_ts_1.stdout, 0, 0);
              clearScreenDown(files_ts_1.stdout);
            };
            this.trace = (...args) => {
              const message = stringifyArgs(args, { indentLevel: 0 });
              const err = {
                name: "Trace",
                message,
              };
              // @ts-ignore
              Error.captureStackTrace(err, this.trace);
              this.error(err.stack);
            };
            this.#printFunc = printFunc;
            this.indentLevel = 0;
            this[isConsoleInstance] = true;
            // ref https://console.spec.whatwg.org/#console-namespace
            // For historical web-compatibility reasons, the namespace object for
            // console must have as its [[Prototype]] an empty object, created as if
            // by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
            const console = Object.create({});
            Object.assign(console, this);
            return console;
          }
          #printFunc;
          static [((_a = isConsoleInstance), Symbol.hasInstance)](instance) {
            return instance[isConsoleInstance];
          }
        };
        exports_35("Console", Console);
        exports_35(
          "customInspect",
          (customInspect = Symbol.for("Deno.customInspect"))
        );
        // Expose these fields to internalObject for tests.
        internals_ts_2.exposeForTest("Console", Console);
        internals_ts_2.exposeForTest("stringifyArgs", stringifyArgs);
      },
    };
  }
);
