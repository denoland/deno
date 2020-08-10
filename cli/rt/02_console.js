// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const exposeForTest = window.__bootstrap.internals.exposeForTest;
  const {
    stripColor,
    yellow,
    dim,
    cyan,
    red,
    green,
    magenta,
    bold,
  } = window.__bootstrap.colors;

  const {
    isTypedArray,
    isInvalidDate,
    hasOwnProperty,
  } = window.__bootstrap.webUtil;

  // Copyright Joyent, Inc. and other Node contributors. MIT license.
  // Forked from Node's lib/internal/cli_table.js

  const tableChars = {
    middleMiddle: "─",
    rowMiddle: "┼",
    topRight: "┐",
    topLeft: "┌",
    leftMiddle: "├",
    topMiddle: "┬",
    bottomRight: "┘",
    bottomLeft: "└",
    bottomMiddle: "┴",
    rightMiddle: "┤",
    left: "│ ",
    right: " │",
    middle: " │ ",
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

  function getStringWidth(str) {
    str = stripColor(str).normalize("NFC");
    let width = 0;

    for (const ch of str) {
      width += isFullWidthCodePoint(ch.codePointAt(0)) ? 2 : 1;
    }

    return width;
  }

  function renderRow(row, columnWidths) {
    let out = tableChars.left;
    for (let i = 0; i < row.length; i++) {
      const cell = row[i];
      const len = getStringWidth(cell);
      const needed = (columnWidths[i] - len) / 2;
      // round(needed) + ceil(needed) will always add up to the amount
      // of spaces we need while also left justifying the output.
      out += `${" ".repeat(needed)}${cell}${" ".repeat(Math.ceil(needed))}`;
      if (i !== row.length - 1) {
        out += tableChars.middle;
      }
    }
    out += tableChars.right;
    return out;
  }

  function cliTable(head, columns) {
    const rows = [];
    const columnWidths = head.map((h) => getStringWidth(h));
    const longestColumn = columns.reduce(
      (n, a) => Math.max(n, a.length),
      0,
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
        columnWidths[i] = Math.max(width, counted);
      }
    }

    const divider = columnWidths.map((i) =>
      tableChars.middleMiddle.repeat(i + 2)
    );

    let result = `${tableChars.topLeft}${divider.join(tableChars.topMiddle)}` +
      `${tableChars.topRight}\n${renderRow(head, columnWidths)}\n` +
      `${tableChars.leftMiddle}${divider.join(tableChars.rowMiddle)}` +
      `${tableChars.rightMiddle}\n`;

    for (const row of rows) {
      result += `${renderRow(row, columnWidths)}\n`;
    }

    result +=
      `${tableChars.bottomLeft}${divider.join(tableChars.bottomMiddle)}` +
      tableChars.bottomRight;

    return result;
  }
  /* End of forked part */

  const DEFAULT_INSPECT_OPTIONS = {
    depth: 4,
    indentLevel: 0,
    sorted: false,
    trailingComma: false,
    compact: true,
    iterableLimit: 100,
  };

  const DEFAULT_INDENT = "  "; // Default indent string

  const LINE_BREAKING_LENGTH = 80;
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

  class CSI {
    static kClear = "\x1b[1;1H";
    static kClearScreenDown = "\x1b[0J";
  }

  /* eslint-disable @typescript-eslint/no-use-before-define */

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

  function inspectFunction(value, _ctx) {
    // Might be Function/AsyncFunction/GeneratorFunction
    const cstrName = Object.getPrototypeOf(value).constructor.name;
    if (value.name && value.name !== "anonymous") {
      // from MDN spec
      return `[${cstrName}: ${value.name}]`;
    }
    return `[${cstrName}]`;
  }

  function inspectIterable(
    value,
    ctx,
    level,
    options,
    inspectOptions,
  ) {
    if (level >= inspectOptions.depth) {
      return cyan(`[${options.typeName}]`);
    }
    ctx.add(value);

    const entries = [];

    const iter = value.entries();
    let entriesLength = 0;
    const next = () => {
      return iter.next();
    };
    for (const el of iter) {
      if (entriesLength < inspectOptions.iterableLimit) {
        entries.push(
          options.entryHandler(
            el,
            ctx,
            level + 1,
            inspectOptions,
            next.bind(iter),
          ),
        );
      }
      entriesLength++;
    }
    ctx.delete(value);

    if (options.sort) {
      entries.sort();
    }

    if (entriesLength > inspectOptions.iterableLimit) {
      const nmore = entriesLength - inspectOptions.iterableLimit;
      entries.push(`... ${nmore} more items`);
    }

    const iPrefix = `${options.displayName ? options.displayName + " " : ""}`;

    const initIndentation = `\n${DEFAULT_INDENT.repeat(level + 1)}`;
    const entryIndentation = `,\n${DEFAULT_INDENT.repeat(level + 1)}`;
    const closingIndentation = `${inspectOptions.trailingComma ? "," : ""}\n${
      DEFAULT_INDENT.repeat(level)
    }`;

    let iContent;
    if (options.group && entries.length > MIN_GROUP_LENGTH) {
      const groups = groupEntries(entries, level, value);
      iContent = `${initIndentation}${
        groups.join(entryIndentation)
      }${closingIndentation}`;
    } else {
      iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
      if (
        stripColor(iContent).length > LINE_BREAKING_LENGTH ||
        !inspectOptions.compact
      ) {
        iContent = `${initIndentation}${
          entries.join(entryIndentation)
        }${closingIndentation}`;
      }
    }

    return `${iPrefix}${options.delims[0]}${iContent}${options.delims[1]}`;
  }

  // Ported from Node.js
  // Copyright Node.js contributors. All rights reserved.
  function groupEntries(
    entries,
    level,
    value,
    iterableLimit = 100,
  ) {
    let totalLength = 0;
    let maxLength = 0;
    let entriesLength = entries.length;
    if (iterableLimit < entriesLength) {
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
          Math.sqrt(approxCharHeights * biasedMax * entriesLength) / biasedMax,
        ),
        // Do not exceed the breakLength.
        Math.floor((LINE_BREAKING_LENGTH - (level + 1)) / actualMax),
        // Limit the columns to a maximum of fifteen.
        15,
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
          /* eslint-disable @typescript-eslint/no-explicit-any */
          if (
            typeof value[i] !== "number" &&
            typeof value[i] !== "bigint"
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
          const lengthOfColorCodes = entries[j].length - dataLen[j];
          const padding = maxLineLength[j - i] + lengthOfColorCodes;
          str += `${entries[j]}, `[order](padding, " ");
        }
        if (order === "padStart") {
          const lengthOfColorCodes = entries[j].length - dataLen[j];
          const padding = maxLineLength[j - i] +
            lengthOfColorCodes -
            separatorSpace;
          str += entries[j].padStart(padding, " ");
        } else {
          str += entries[j];
        }
        tmp.push(str);
      }
      if (iterableLimit < entries.length) {
        tmp.push(entries[entriesLength]);
      }
      entries = tmp;
    }
    return entries;
  }

  function inspectValue(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
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
        return cyan(inspectFunction(value, ctx));
      case "object": // null is bold
        if (value === null) {
          return bold("null");
        }

        if (ctx.has(value)) {
          // Circular string is cyan
          return cyan("[Circular]");
        }

        return inspectObject(value, ctx, level, inspectOptions);
      default:
        // Not implemented is red
        return red("[Not Implemented]");
    }
  }

  // We can match Node's quoting behavior exactly by swapping the double quote and
  // single quote in this array. That would give preference to single quotes.
  // However, we prefer double quotes as the default.
  const QUOTES = ['"', "'", "`"];

  /** Surround the string in quotes.
 *
 * The quote symbol is chosen by taking the first of the `QUOTES` array which
 * does not occur in the string. If they all occur, settle with `QUOTES[0]`.
 *
 * Insert a backslash before any occurrence of the chosen quote symbol and
 * before any backslash. */
  function quoteString(string) {
    const quote = QUOTES.find((c) => !string.includes(c)) ?? QUOTES[0];
    const escapePattern = new RegExp(`(?=[${quote}\\\\])`, "g");
    return `${quote}${string.replace(escapePattern, "\\")}${quote}`;
  }

  // Print strings when they are inside of arrays or objects with quotes
  function inspectValueWithQuotes(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    switch (typeof value) {
      case "string":
        const trunc = value.length > STR_ABBREVIATE_SIZE
          ? value.slice(0, STR_ABBREVIATE_SIZE) + "..."
          : value;
        return green(quoteString(trunc)); // Quoted strings are green
      default:
        return inspectValue(value, ctx, level, inspectOptions);
    }
  }

  function inspectArray(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const options = {
      typeName: "Array",
      displayName: "",
      delims: ["[", "]"],
      entryHandler: (entry, ctx, level, inspectOptions, next) => {
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
          return dim(`<${emptyItems} empty item${ending}>`);
        } else {
          return inspectValueWithQuotes(val, ctx, level, inspectOptions);
        }
      },
      group: inspectOptions.compact,
      sort: false,
    };
    return inspectIterable(value, ctx, level, options, inspectOptions);
  }

  function inspectTypedArray(
    typedArrayName,
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const valueLength = value.length;
    const options = {
      typeName: typedArrayName,
      displayName: `${typedArrayName}(${valueLength})`,
      delims: ["[", "]"],
      entryHandler: (entry, ctx, level, inspectOptions) => {
        const val = entry[1];
        return inspectValueWithQuotes(val, ctx, level + 1, inspectOptions);
      },
      group: inspectOptions.compact,
      sort: false,
    };
    return inspectIterable(value, ctx, level, options, inspectOptions);
  }

  function inspectSet(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const options = {
      typeName: "Set",
      displayName: "Set",
      delims: ["{", "}"],
      entryHandler: (entry, ctx, level, inspectOptions) => {
        const val = entry[1];
        return inspectValueWithQuotes(val, ctx, level + 1, inspectOptions);
      },
      group: false,
      sort: inspectOptions.sorted,
    };
    return inspectIterable(value, ctx, level, options, inspectOptions);
  }

  function inspectMap(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const options = {
      typeName: "Map",
      displayName: "Map",
      delims: ["{", "}"],
      entryHandler: (entry, ctx, level, inspectOptions) => {
        const [key, val] = entry;
        return `${
          inspectValueWithQuotes(
            key,
            ctx,
            level + 1,
            inspectOptions,
          )
        } => ${inspectValueWithQuotes(val, ctx, level + 1, inspectOptions)}`;
      },
      group: false,
      sort: inspectOptions.sorted,
    };
    return inspectIterable(
      value,
      ctx,
      level,
      options,
      inspectOptions,
    );
  }

  function inspectWeakSet() {
    return `WeakSet { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
  }

  function inspectWeakMap() {
    return `WeakMap { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
  }

  function inspectDate(value) {
    // without quotes, ISO format, in magenta like before
    return magenta(isInvalidDate(value) ? "Invalid Date" : value.toISOString());
  }

  function inspectRegExp(value) {
    return red(value.toString()); // RegExps are red
  }

  function inspectStringObject(value) {
    return cyan(`[String: "${value.toString()}"]`); // wrappers are in cyan
  }

  function inspectBooleanObject(value) {
    return cyan(`[Boolean: ${value.toString()}]`); // wrappers are in cyan
  }

  function inspectNumberObject(value) {
    return cyan(`[Number: ${value.toString()}]`); // wrappers are in cyan
  }

  const PromiseState = {
    Pending: 0,
    Fulfilled: 1,
    Rejected: 2,
  };

  function inspectPromise(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const [state, result] = Deno.core.getPromiseDetails(value);

    if (state === PromiseState.Pending) {
      return `Promise { ${cyan("<pending>")} }`;
    }

    const prefix = state === PromiseState.Fulfilled
      ? ""
      : `${red("<rejected>")} `;

    const str = `${prefix}${
      inspectValueWithQuotes(
        result,
        ctx,
        level + 1,
        inspectOptions,
      )
    }`;

    if (str.length + PROMISE_STRING_BASE_LENGTH > LINE_BREAKING_LENGTH) {
      return `Promise {\n${DEFAULT_INDENT.repeat(level + 1)}${str}\n}`;
    }

    return `Promise { ${str} }`;
  }

  // TODO: Proxy

  function inspectRawObject(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    if (level >= inspectOptions.depth) {
      return cyan("[Object]"); // wrappers are in cyan
    }
    ctx.add(value);

    let baseString;

    let shouldShowDisplayName = false;
    let displayName = value[
      Symbol.toStringTag
    ];
    if (!displayName) {
      displayName = getClassInstanceName(value);
    }
    if (
      displayName && displayName !== "Object" && displayName !== "anonymous"
    ) {
      shouldShowDisplayName = true;
    }

    const entries = [];
    const stringKeys = Object.keys(value);
    const symbolKeys = Object.getOwnPropertySymbols(value);
    if (inspectOptions.sorted) {
      stringKeys.sort();
      symbolKeys.sort((s1, s2) =>
        (s1.description ?? "").localeCompare(s2.description ?? "")
      );
    }

    for (const key of stringKeys) {
      entries.push(
        `${key}: ${
          inspectValueWithQuotes(
            value[key],
            ctx,
            level + 1,
            inspectOptions,
          )
        }`,
      );
    }
    for (const key of symbolKeys) {
      entries.push(
        `${key.toString()}: ${
          inspectValueWithQuotes(
            value[key],
            ctx,
            level + 1,
            inspectOptions,
          )
        }`,
      );
    }
    // Making sure color codes are ignored when calculating the total length
    const totalLength = entries.length + level +
      stripColor(entries.join("")).length;

    ctx.delete(value);

    if (entries.length === 0) {
      baseString = "{}";
    } else if (totalLength > LINE_BREAKING_LENGTH || !inspectOptions.compact) {
      const entryIndent = DEFAULT_INDENT.repeat(level + 1);
      const closingIndent = DEFAULT_INDENT.repeat(level);
      baseString = `{\n${entryIndent}${entries.join(`,\n${entryIndent}`)}${
        inspectOptions.trailingComma ? "," : ""
      }\n${closingIndent}}`;
    } else {
      baseString = `{ ${entries.join(", ")} }`;
    }

    if (shouldShowDisplayName) {
      baseString = `${displayName} ${baseString}`;
    }

    return baseString;
  }

  function inspectObject(
    value,
    consoleContext,
    level,
    inspectOptions,
  ) {
    if (customInspect in value && typeof value[customInspect] === "function") {
      try {
        return String(value[customInspect]());
      } catch {}
    }
    if (value instanceof Error) {
      return String(value.stack);
    } else if (Array.isArray(value)) {
      return inspectArray(value, consoleContext, level, inspectOptions);
    } else if (value instanceof Number) {
      return inspectNumberObject(value);
    } else if (value instanceof Boolean) {
      return inspectBooleanObject(value);
    } else if (value instanceof String) {
      return inspectStringObject(value);
    } else if (value instanceof Promise) {
      return inspectPromise(value, consoleContext, level, inspectOptions);
    } else if (value instanceof RegExp) {
      return inspectRegExp(value);
    } else if (value instanceof Date) {
      return inspectDate(value);
    } else if (value instanceof Set) {
      return inspectSet(value, consoleContext, level, inspectOptions);
    } else if (value instanceof Map) {
      return inspectMap(value, consoleContext, level, inspectOptions);
    } else if (value instanceof WeakSet) {
      return inspectWeakSet();
    } else if (value instanceof WeakMap) {
      return inspectWeakMap();
    } else if (isTypedArray(value)) {
      return inspectTypedArray(
        Object.getPrototypeOf(value).constructor.name,
        value,
        consoleContext,
        level,
        inspectOptions,
      );
    } else {
      // Otherwise, default object formatting
      return inspectRawObject(value, consoleContext, level, inspectOptions);
    }
  }

  function inspectArgs(
    args,
    inspectOptions = {},
  ) {
    const rInspectOptions = { ...DEFAULT_INSPECT_OPTIONS, ...inspectOptions };
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
                tempStr = inspectValue(
                  args[++a],
                  new Set(),
                  0,
                  rInspectOptions,
                );
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
        str += inspectValue(value, new Set(), 0, rInspectOptions);
      }
      join = " ";
      a++;
    }

    if (rInspectOptions.indentLevel > 0) {
      const groupIndent = DEFAULT_INDENT.repeat(rInspectOptions.indentLevel);
      if (str.indexOf("\n") !== -1) {
        str = str.replace(/\n/g, `\n${groupIndent}`);
      }
      str = groupIndent + str;
    }

    return str;
  }

  const countMap = new Map();
  const timerMap = new Map();
  const isConsoleInstance = Symbol("isConsoleInstance");

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
      const console = Object.create({});
      Object.assign(console, this);
      return console;
    }

    log = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          indentLevel: this.indentLevel,
        }) + "\n",
        false,
      );
    };

    debug = this.log;
    info = this.log;

    dir = (obj, options = {}) => {
      this.#printFunc(inspectArgs([obj], options) + "\n", false);
    };

    dirxml = this.dir;

    warn = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          indentLevel: this.indentLevel,
        }) + "\n",
        true,
      );
    };

    error = this.warn;

    assert = (condition = false, ...args) => {
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

    count = (label = "default") => {
      label = String(label);

      if (countMap.has(label)) {
        const current = countMap.get(label) || 0;
        countMap.set(label, current + 1);
      } else {
        countMap.set(label, 1);
      }

      this.info(`${label}: ${countMap.get(label)}`);
    };

    countReset = (label = "default") => {
      label = String(label);

      if (countMap.has(label)) {
        countMap.set(label, 0);
      } else {
        this.warn(`Count for '${label}' does not exist`);
      }
    };

    table = (data, properties) => {
      if (properties !== undefined && !Array.isArray(properties)) {
        throw new Error(
          "The 'properties' argument must be of type Array. " +
            "Received type string",
        );
      }

      if (data === null || typeof data !== "object") {
        return this.log(data);
      }

      const objectValues = {};
      const indexKeys = [];
      const values = [];

      const stringifyValue = (value) =>
        inspectValueWithQuotes(value, new Set(), 0, {
          ...DEFAULT_INSPECT_OPTIONS,
          depth: 1,
        });
      const toTable = (header, body) => this.log(cliTable(header, body));
      const createColumn = (value, shift) => [
        ...(shift ? [...new Array(shift)].map(() => "") : []),
        stringifyValue(value),
      ];

      let resultData;
      const isSet = data instanceof Set;
      const isMap = data instanceof Map;
      const valuesKey = "Values";
      const indexKey = isSet || isMap ? "(iter idx)" : "(idx)";

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

      let hasPrimitives = false;
      Object.keys(resultData).forEach((k, idx) => {
        const value = resultData[k];
        const primitive = value === null ||
          (typeof value !== "function" && typeof value !== "object");
        if (properties === undefined && primitive) {
          hasPrimitives = true;
          values.push(stringifyValue(value));
        } else {
          const valueObj = value || {};
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
        ...(properties ||
          [...headerKeys, !isMap && hasPrimitives && valuesKey]),
      ].filter(Boolean);
      const body = [indexKeys, ...bodyValues, values];

      toTable(header, body);
    };

    time = (label = "default") => {
      label = String(label);

      if (timerMap.has(label)) {
        this.warn(`Timer '${label}' already exists`);
        return;
      }

      timerMap.set(label, Date.now());
    };

    timeLog = (label = "default", ...args) => {
      label = String(label);

      if (!timerMap.has(label)) {
        this.warn(`Timer '${label}' does not exists`);
        return;
      }

      const startTime = timerMap.get(label);
      const duration = Date.now() - startTime;

      this.info(`${label}: ${duration}ms`, ...args);
    };

    timeEnd = (label = "default") => {
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

    group = (...label) => {
      if (label.length > 0) {
        this.log(...label);
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
      this.#printFunc(CSI.kClear, false);
      this.#printFunc(CSI.kClearScreenDown, false);
    };

    trace = (...args) => {
      const message = inspectArgs(args, { indentLevel: 0 });
      const err = {
        name: "Trace",
        message,
      };
      Error.captureStackTrace(err, this.trace);
      this.error(err.stack);
    };

    static [Symbol.hasInstance](instance) {
      return instance[isConsoleInstance];
    }
  }

  const customInspect = Symbol("Deno.customInspect");

  function inspect(
    value,
    inspectOptions = {},
  ) {
    if (typeof value === "string") {
      return value;
    } else {
      return inspectValue(value, new Set(), 0, {
        ...DEFAULT_INSPECT_OPTIONS,
        ...inspectOptions,
        // TODO(nayeemrmn): Indent level is not supported.
        indentLevel: 0,
      });
    }
  }

  // Expose these fields to internalObject for tests.
  exposeForTest("Console", Console);
  exposeForTest("inspectArgs", inspectArgs);

  window.__bootstrap.console = {
    CSI,
    inspectArgs,
    Console,
    customInspect,
    inspect,
  };
})(this);
