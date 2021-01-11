// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const exposeForTest = window.__bootstrap.internals.exposeForTest;
  const colors = window.__bootstrap.colors;
  const { PermissionDenied } = window.__bootstrap.errors;

  function isInvalidDate(x) {
    return isNaN(x.getTime());
  }

  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return Object.prototype.hasOwnProperty.call(obj, v);
  }

  // Copyright Joyent, Inc. and other Node contributors. MIT license.
  // Forked from Node's lib/internal/cli_table.js

  function isTypedArray(x) {
    return ArrayBuffer.isView(x) && !(x instanceof DataView);
  }

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
    str = colors.stripColor(str).normalize("NFC");
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
    showProxy: false,
    colors: false,
    getters: false,
  };

  const DEFAULT_INDENT = "  "; // Default indent string

  const LINE_BREAKING_LENGTH = 80;
  const MIN_GROUP_LENGTH = 6;
  const STR_ABBREVIATE_SIZE = 100;

  const PROMISE_STRING_BASE_LENGTH = 12;

  class CSI {
    static kClear = "\x1b[1;1H";
    static kClearScreenDown = "\x1b[0J";
  }

  function getClassInstanceName(instance) {
    if (typeof instance != "object") {
      return "";
    }
    const constructor = instance?.constructor;
    if (typeof constructor == "function") {
      return constructor.name ?? "";
    }
    return "";
  }

  function maybeColor(fn, inspectOptions) {
    return inspectOptions.colors ? fn : (s) => s;
  }

  function inspectFunction(value, _ctx) {
    if (customInspect in value && typeof value[customInspect] === "function") {
      return String(value[customInspect]());
    }
    // Might be Function/AsyncFunction/GeneratorFunction/AsyncGeneratorFunction
    let cstrName = Object.getPrototypeOf(value)?.constructor?.name;
    if (!cstrName) {
      // If prototype is removed or broken,
      // use generic 'Function' instead.
      cstrName = "Function";
    }
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
    const cyan = maybeColor(colors.cyan, inspectOptions);
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
        colors.stripColor(iContent).length > LINE_BREAKING_LENGTH ||
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
      const len = colors.stripColor(entries[i]).length;
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
          if (
            typeof value[i] !== "number" &&
            typeof value[i] !== "bigint"
          ) {
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
    const proxyDetails = core.getProxyDetails(value);
    if (proxyDetails != null) {
      return inspectOptions.showProxy
        ? inspectProxy(proxyDetails, ctx, level, inspectOptions)
        : inspectValue(proxyDetails[0], ctx, level, inspectOptions);
    }

    const green = maybeColor(colors.green, inspectOptions);
    const yellow = maybeColor(colors.yellow, inspectOptions);
    const dim = maybeColor(colors.dim, inspectOptions);
    const cyan = maybeColor(colors.cyan, inspectOptions);
    const bold = maybeColor(colors.bold, inspectOptions);
    const red = maybeColor(colors.red, inspectOptions);

    switch (typeof value) {
      case "string":
        return green(quoteString(value));
      case "number": // Numbers are yellow
        // Special handling of -0
        return yellow(Object.is(value, -0) ? "-0" : `${value}`);
      case "boolean": // booleans are yellow
        return yellow(String(value));
      case "undefined": // undefined is dim
        return dim(String(value));
      case "symbol": // Symbols are green
        return green(maybeQuoteSymbol(value));
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
   * before any backslash.
   */
  function quoteString(string) {
    const quote = QUOTES.find((c) => !string.includes(c)) ?? QUOTES[0];
    const escapePattern = new RegExp(`(?=[${quote}\\\\])`, "g");
    string = string.replace(escapePattern, "\\");
    string = replaceEscapeSequences(string);
    return `${quote}${string}${quote}`;
  }

  // Replace escape sequences that can modify output.
  function replaceEscapeSequences(string) {
    return string
      .replace(/[\b]/g, "\\b")
      .replace(/\f/g, "\\f")
      .replace(/\n/g, "\\n")
      .replace(/\r/g, "\\r")
      .replace(/\t/g, "\\t")
      .replace(/\v/g, "\\v")
      .replace(
        // deno-lint-ignore no-control-regex
        /[\x00-\x1f\x7f-\x9f]/g,
        (c) => "\\x" + c.charCodeAt(0).toString(16).padStart(2, "0"),
      );
  }

  // Surround a string with quotes when it is required (e.g the string not a valid identifier).
  function maybeQuoteString(string) {
    if (/^[a-zA-Z_][a-zA-Z_0-9]*$/.test(string)) {
      return replaceEscapeSequences(string);
    }

    return quoteString(string);
  }

  // Surround a symbol's description in quotes when it is required (e.g the description has non printable characters).
  function maybeQuoteSymbol(symbol) {
    if (symbol.description === undefined) {
      return symbol.toString();
    }

    if (/^[a-zA-Z_][a-zA-Z_.0-9]*$/.test(symbol.description)) {
      return symbol.toString();
    }

    return `Symbol(${quoteString(symbol.description)})`;
  }

  // Print strings when they are inside of arrays or objects with quotes
  function inspectValueWithQuotes(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const green = maybeColor(colors.green, inspectOptions);
    switch (typeof value) {
      case "string": {
        const trunc = value.length > STR_ABBREVIATE_SIZE
          ? value.slice(0, STR_ABBREVIATE_SIZE) + "..."
          : value;
        return green(quoteString(trunc)); // Quoted strings are green
      }
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
    const dim = maybeColor(colors.dim, inspectOptions);
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

  function inspectWeakSet(inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return `WeakSet { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
  }

  function inspectWeakMap(inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return `WeakMap { ${cyan("[items unknown]")} }`; // as seen in Node, with cyan color
  }

  function inspectDate(value, inspectOptions) {
    // without quotes, ISO format, in magenta like before
    const magenta = maybeColor(colors.magenta, inspectOptions);
    return magenta(isInvalidDate(value) ? "Invalid Date" : value.toISOString());
  }

  function inspectRegExp(value, inspectOptions) {
    const red = maybeColor(colors.red, inspectOptions);
    return red(value.toString()); // RegExps are red
  }

  function inspectStringObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[String: "${value.toString()}"]`); // wrappers are in cyan
  }

  function inspectBooleanObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[Boolean: ${value.toString()}]`); // wrappers are in cyan
  }

  function inspectNumberObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
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
    const cyan = maybeColor(colors.cyan, inspectOptions);
    const red = maybeColor(colors.red, inspectOptions);

    const [state, result] = core.getPromiseDetails(value);

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

  function inspectProxy(
    targetAndHandler,
    ctx,
    level,
    inspectOptions,
  ) {
    return `Proxy ${
      inspectArray(targetAndHandler, ctx, level, inspectOptions)
    }`;
  }

  function inspectRawObject(
    value,
    ctx,
    level,
    inspectOptions,
  ) {
    const cyan = maybeColor(colors.cyan, inspectOptions);

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

    const red = maybeColor(colors.red, inspectOptions);

    for (const key of stringKeys) {
      if (inspectOptions.getters) {
        let propertyValue;
        let error = null;
        try {
          propertyValue = value[key];
        } catch (error_) {
          error = error_;
        }
        const inspectedValue = error == null
          ? inspectValueWithQuotes(
            propertyValue,
            ctx,
            level + 1,
            inspectOptions,
          )
          : red(`[Thrown ${error.name}: ${error.message}]`);
        entries.push(`${maybeQuoteString(key)}: ${inspectedValue}`);
      } else {
        const descriptor = Object.getOwnPropertyDescriptor(value, key);
        if (descriptor.get !== undefined && descriptor.set !== undefined) {
          entries.push(`${maybeQuoteString(key)}: [Getter/Setter]`);
        } else if (descriptor.get !== undefined) {
          entries.push(`${maybeQuoteString(key)}: [Getter]`);
        } else {
          entries.push(
            `${maybeQuoteString(key)}: ${
              inspectValueWithQuotes(value[key], ctx, level + 1, inspectOptions)
            }`,
          );
        }
      }
    }

    for (const key of symbolKeys) {
      if (inspectOptions.getters) {
        let propertyValue;
        let error;
        try {
          propertyValue = value[key];
        } catch (error_) {
          error = error_;
        }
        const inspectedValue = error == null
          ? inspectValueWithQuotes(
            propertyValue,
            ctx,
            level + 1,
            inspectOptions,
          )
          : red(`Thrown ${error.name}: ${error.message}`);
        entries.push(`[${maybeQuoteSymbol(key)}]: ${inspectedValue}`);
      } else {
        const descriptor = Object.getOwnPropertyDescriptor(value, key);
        if (descriptor.get !== undefined && descriptor.set !== undefined) {
          entries.push(`[${maybeQuoteSymbol(key)}]: [Getter/Setter]`);
        } else if (descriptor.get !== undefined) {
          entries.push(`[${maybeQuoteSymbol(key)}]: [Getter]`);
        } else {
          entries.push(
            `[${maybeQuoteSymbol(key)}]: ${
              inspectValueWithQuotes(value[key], ctx, level + 1, inspectOptions)
            }`,
          );
        }
      }
    }

    // Making sure color codes are ignored when calculating the total length
    const totalLength = entries.length + level +
      colors.stripColor(entries.join("")).length;

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
      return String(value[customInspect]());
    }
    // This non-unique symbol is used to support op_crates, ie.
    // in op_crates/web we don't want to depend on unique "Deno.customInspect"
    // symbol defined in the public API. Internal only, shouldn't be used
    // by users.
    const nonUniqueCustomInspect = Symbol.for("Deno.customInspect");
    if (
      nonUniqueCustomInspect in value &&
      typeof value[nonUniqueCustomInspect] === "function"
    ) {
      return String(value[nonUniqueCustomInspect]());
    }
    if (value instanceof Error) {
      return String(value.stack);
    } else if (Array.isArray(value)) {
      return inspectArray(value, consoleContext, level, inspectOptions);
    } else if (value instanceof Number) {
      return inspectNumberObject(value, inspectOptions);
    } else if (value instanceof Boolean) {
      return inspectBooleanObject(value, inspectOptions);
    } else if (value instanceof String) {
      return inspectStringObject(value, inspectOptions);
    } else if (value instanceof Promise) {
      return inspectPromise(value, consoleContext, level, inspectOptions);
    } else if (value instanceof RegExp) {
      return inspectRegExp(value, inspectOptions);
    } else if (value instanceof Date) {
      return inspectDate(value, inspectOptions);
    } else if (value instanceof Set) {
      return inspectSet(value, consoleContext, level, inspectOptions);
    } else if (value instanceof Map) {
      return inspectMap(value, consoleContext, level, inspectOptions);
    } else if (value instanceof WeakSet) {
      return inspectWeakSet(inspectOptions);
    } else if (value instanceof WeakMap) {
      return inspectWeakMap(inspectOptions);
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

  const colorKeywords = new Map([
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

  function parseCssColor(colorString) {
    if (colorKeywords.has(colorString)) {
      colorString = colorKeywords.get(colorString);
    }
    // deno-fmt-ignore
    const hashMatch = colorString.match(/^#([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})?$/);
    if (hashMatch != null) {
      return [
        Number(`0x${hashMatch[1]}`),
        Number(`0x${hashMatch[2]}`),
        Number(`0x${hashMatch[3]}`),
      ];
    }
    // deno-fmt-ignore
    const smallHashMatch = colorString.match(/^#([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])?$/);
    if (smallHashMatch != null) {
      return [
        Number(`0x${smallHashMatch[1]}0`),
        Number(`0x${smallHashMatch[2]}0`),
        Number(`0x${smallHashMatch[3]}0`),
      ];
    }
    // deno-fmt-ignore
    const rgbMatch = colorString.match(/^rgba?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/);
    if (rgbMatch != null) {
      return [
        Math.round(Math.max(0, Math.min(255, Number(rgbMatch[1])))),
        Math.round(Math.max(0, Math.min(255, Number(rgbMatch[2])))),
        Math.round(Math.max(0, Math.min(255, Number(rgbMatch[3])))),
      ];
    }
    // deno-fmt-ignore
    const hslMatch = colorString.match(/^hsla?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)%\s*,\s*([+\-]?\d*\.?\d+)%\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/);
    if (hslMatch != null) {
      // https://www.rapidtables.com/convert/color/hsl-to-rgb.html
      let h = Number(hslMatch[1]) % 360;
      if (h < 0) {
        h += 360;
      }
      const s = Math.max(0, Math.min(100, Number(hslMatch[2]))) / 100;
      const l = Math.max(0, Math.min(100, Number(hslMatch[3]))) / 100;
      const c = (1 - Math.abs(2 * l - 1)) * s;
      const x = c * (1 - Math.abs((h / 60) % 2 - 1));
      const m = l - c / 2;
      let r_;
      let g_;
      let b_;
      if (h < 60) {
        [r_, g_, b_] = [c, x, 0];
      } else if (h < 120) {
        [r_, g_, b_] = [x, c, 0];
      } else if (h < 180) {
        [r_, g_, b_] = [0, c, x];
      } else if (h < 240) {
        [r_, g_, b_] = [0, x, c];
      } else if (h < 300) {
        [r_, g_, b_] = [x, 0, c];
      } else {
        [r_, g_, b_] = [c, 0, x];
      }
      return [
        Math.round((r_ + m) * 255),
        Math.round((g_ + m) * 255),
        Math.round((b_ + m) * 255),
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
          const value = currentPart.trim();
          if (value != "") {
            rawEntries.push([currentKey, value]);
          }
          currentKey = null;
          currentPart = "";
          inValue = false;
          continue;
        }
      } else if (c == ":") {
        currentKey = currentPart.trim();
        currentPart = "";
        inValue = true;
        continue;
      }
      currentPart += c;
    }
    if (inValue && parenthesesDepth == 0) {
      const value = currentPart.trim();
      if (value != "") {
        rawEntries.push([currentKey, value]);
      }
      currentKey = null;
      currentPart = "";
    }

    for (const [key, value] of rawEntries) {
      if (key == "background-color") {
        const color = parseCssColor(value);
        if (color != null) {
          css.backgroundColor = color;
        }
      } else if (key == "color") {
        const color = parseCssColor(value);
        if (color != null) {
          css.color = color;
        }
      } else if (key == "font-weight") {
        if (value == "bold") {
          css.fontWeight = value;
        }
      } else if (key == "font-style") {
        if (["italic", "oblique", "oblique 14deg"].includes(value)) {
          css.fontStyle = "italic";
        }
      } else if (key == "text-decoration-line") {
        css.textDecorationLine = [];
        for (const lineType of value.split(/\s+/g)) {
          if (["line-through", "overline", "underline"].includes(lineType)) {
            css.textDecorationLine.push(lineType);
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
        for (const arg of value.split(/\s+/g)) {
          const maybeColor = parseCssColor(arg);
          if (maybeColor != null) {
            css.textDecorationColor = maybeColor;
          } else if (["line-through", "overline", "underline"].includes(arg)) {
            css.textDecorationLine.push(arg);
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
      if (css.backgroundColor != null) {
        const [r, g, b] = css.backgroundColor;
        ansi += `\x1b[48;2;${r};${g};${b}m`;
      } else {
        ansi += "\x1b[49m";
      }
    }
    if (!colorEquals(css.color, prevCss.color)) {
      if (css.color != null) {
        const [r, g, b] = css.color;
        ansi += `\x1b[38;2;${r};${g};${b}m`;
      } else {
        ansi += "\x1b[39m";
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
        const [r, g, b] = css.textDecorationColor;
        ansi += `\x1b[58;2;${r};${g};${b}m`;
      } else {
        ansi += "\x1b[59m";
      }
    }
    if (
      css.textDecorationLine.includes("line-through") !=
        prevCss.textDecorationLine.includes("line-through")
    ) {
      if (css.textDecorationLine.includes("line-through")) {
        ansi += "\x1b[9m";
      } else {
        ansi += "\x1b[29m";
      }
    }
    if (
      css.textDecorationLine.includes("overline") !=
        prevCss.textDecorationLine.includes("overline")
    ) {
      if (css.textDecorationLine.includes("overline")) {
        ansi += "\x1b[53m";
      } else {
        ansi += "\x1b[55m";
      }
    }
    if (
      css.textDecorationLine.includes("underline") !=
        prevCss.textDecorationLine.includes("underline")
    ) {
      if (css.textDecorationLine.includes("underline")) {
        ansi += "\x1b[4m";
      } else {
        ansi += "\x1b[24m";
      }
    }
    return ansi;
  }

  function inspectArgs(args, inspectOptions = {}) {
    const noColor = globalThis.Deno?.noColor ?? true;
    const rInspectOptions = { ...DEFAULT_INSPECT_OPTIONS, ...inspectOptions };
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
            } else if (["d", "i"].includes(char)) {
              // Format as an integer.
              const value = args[a++];
              if (typeof value == "bigint") {
                formattedArg = `${value}n`;
              } else if (typeof value == "number") {
                formattedArg = `${parseInt(String(value))}`;
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
            } else if (["O", "o"].includes(char)) {
              // Format as an object.
              formattedArg = inspectValue(
                args[a++],
                new Set(),
                0,
                rInspectOptions,
              );
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
              string += first.slice(appendedChars, i - 1) + formattedArg;
              appendedChars = i + 1;
            }
          }
          if (char == "%") {
            string += first.slice(appendedChars, i - 1) + "%";
            appendedChars = i + 1;
          }
        }
      }
      string += first.slice(appendedChars);
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
        string += inspectValue(args[a], new Set(), 0, rInspectOptions);
      }
    }

    if (rInspectOptions.indentLevel > 0) {
      const groupIndent = DEFAULT_INDENT.repeat(rInspectOptions.indentLevel);
      string = groupIndent + string.replaceAll("\n", `\n${groupIndent}`);
    }

    return string;
  }

  const countMap = new Map();
  const timerMap = new Map();
  const isConsoleInstance = Symbol("isConsoleInstance");

  function getConsoleInspectOptions() {
    return {
      ...DEFAULT_INSPECT_OPTIONS,
      colors: !(globalThis.Deno?.noColor ?? false),
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
      const console = Object.create({}, {
        [Symbol.toStringTag]: {
          enumerable: false,
          writable: false,
          configurable: true,
          value: "console",
        },
      });
      Object.assign(console, this);
      return console;
    }

    log = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
          indentLevel: this.indentLevel,
        }) + "\n",
        false,
      );
    };

    debug = this.log;
    info = this.log;

    dir = (obj, options = {}) => {
      this.#printFunc(
        inspectArgs([obj], { ...getConsoleInspectOptions(), ...options }) +
          "\n",
        false,
      );
    };

    dirxml = this.dir;

    warn = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
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
      const message = inspectArgs(
        args,
        { ...getConsoleInspectOptions(), indentLevel: 0 },
      );
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
    return inspectValue(value, new Set(), 0, {
      ...DEFAULT_INSPECT_OPTIONS,
      ...inspectOptions,
      // TODO(nayeemrmn): Indent level is not supported.
      indentLevel: 0,
    });
  }

  // Expose these fields to internalObject for tests.
  exposeForTest("Console", Console);
  exposeForTest("cssToAnsi", cssToAnsi);
  exposeForTest("inspectArgs", inspectArgs);
  exposeForTest("parseCss", parseCss);
  exposeForTest("parseCssColor", parseCssColor);

  window.__bootstrap.console = {
    CSI,
    inspectArgs,
    Console,
    customInspect,
    inspect,
  };
})(this);
