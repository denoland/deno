// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const colors = window.__bootstrap.colors;
  const {
    ArrayBufferIsView,
    AggregateErrorPrototype,
    ArrayPrototypeUnshift,
    isNaN,
    DataViewPrototype,
    DatePrototype,
    DateNow,
    DatePrototypeGetTime,
    DatePrototypeToISOString,
    Boolean,
    BooleanPrototype,
    BooleanPrototypeToString,
    ObjectKeys,
    ObjectCreate,
    ObjectAssign,
    ObjectIs,
    ObjectValues,
    ObjectFromEntries,
    ObjectGetPrototypeOf,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetOwnPropertySymbols,
    ObjectPrototypeHasOwnProperty,
    ObjectPrototypeIsPrototypeOf,
    ObjectPrototypePropertyIsEnumerable,
    PromisePrototype,
    String,
    StringPrototype,
    StringPrototypeRepeat,
    StringPrototypeReplace,
    StringPrototypeReplaceAll,
    StringPrototypeSplit,
    StringPrototypeSlice,
    StringPrototypeCodePointAt,
    StringPrototypeCharCodeAt,
    StringPrototypeNormalize,
    StringPrototypeMatch,
    StringPrototypePadStart,
    StringPrototypeLocaleCompare,
    StringPrototypeToString,
    StringPrototypeTrim,
    StringPrototypeIncludes,
    TypeError,
    NumberParseInt,
    RegExp,
    RegExpPrototype,
    RegExpPrototypeTest,
    RegExpPrototypeToString,
    SafeArrayIterator,
    SafeSet,
    SetPrototype,
    SetPrototypeEntries,
    Symbol,
    SymbolPrototype,
    SymbolPrototypeToString,
    SymbolPrototypeValueOf,
    SymbolToStringTag,
    SymbolHasInstance,
    SymbolFor,
    Array,
    ArrayIsArray,
    ArrayPrototypeJoin,
    ArrayPrototypeMap,
    ArrayPrototypeReduce,
    ArrayPrototypeEntries,
    ArrayPrototypePush,
    ArrayPrototypePop,
    ArrayPrototypeSort,
    ArrayPrototypeSlice,
    ArrayPrototypeShift,
    ArrayPrototypeIncludes,
    ArrayPrototypeFill,
    ArrayPrototypeFilter,
    ArrayPrototypeFind,
    FunctionPrototypeBind,
    Map,
    MapPrototype,
    MapPrototypeHas,
    MapPrototypeGet,
    MapPrototypeSet,
    MapPrototypeDelete,
    MapPrototypeEntries,
    MapPrototypeForEach,
    Error,
    ErrorPrototype,
    ErrorCaptureStackTrace,
    MathAbs,
    MathMax,
    MathMin,
    MathSqrt,
    MathRound,
    MathFloor,
    Number,
    NumberPrototype,
    NumberPrototypeToString,
    NumberPrototypeValueOf,
    BigIntPrototype,
    BigIntPrototypeToString,
    Proxy,
    ReflectGet,
    ReflectGetOwnPropertyDescriptor,
    ReflectGetPrototypeOf,
    ReflectHas,
    WeakMapPrototype,
    WeakSetPrototype,
  } = window.__bootstrap.primordials;

  function isInvalidDate(x) {
    return isNaN(DatePrototypeGetTime(x));
  }

  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return ObjectPrototypeHasOwnProperty(obj, v);
  }

  function propertyIsEnumerable(obj, prop) {
    if (
      obj == null ||
      typeof obj.propertyIsEnumerable !== "function"
    ) {
      return false;
    }

    return ObjectPrototypePropertyIsEnumerable(obj, prop);
  }

  // Copyright Joyent, Inc. and other Node contributors. MIT license.
  // Forked from Node's lib/internal/cli_table.js

  function isTypedArray(x) {
    return ArrayBufferIsView(x) &&
      !ObjectPrototypeIsPrototypeOf(DataViewPrototype, x);
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
    str = StringPrototypeNormalize(colors.stripColor(str), "NFC");
    let width = 0;

    for (const ch of str) {
      width += isFullWidthCodePoint(StringPrototypeCodePointAt(ch, 0)) ? 2 : 1;
    }

    return width;
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

  function canRightAlign(value) {
    const isNumber = !isNaN(value);
    return isNumber;
  }

  function cliTable(head, columns) {
    const rows = [];
    const columnWidths = ArrayPrototypeMap(head, (h) => getStringWidth(h));
    const longestColumn = ArrayPrototypeReduce(
      columns,
      (n, a) => MathMax(n, a.length),
      0,
    );
    const columnRightAlign = new Array(columnWidths.length).fill(true);

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
        columnRightAlign[i] &= canRightAlign(value);
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

    for (const row of rows) {
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
    showHidden: false,
    strAbbreviateSize: 100,
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

  function inspectFunction(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    if (
      ReflectHas(value, customInspect) &&
      typeof value[customInspect] === "function"
    ) {
      return String(value[customInspect](inspect, inspectOptions));
    }
    // Might be Function/AsyncFunction/GeneratorFunction/AsyncGeneratorFunction
    let cstrName = ObjectGetPrototypeOf(value)?.constructor?.name;
    if (!cstrName) {
      // If prototype is removed or broken,
      // use generic 'Function' instead.
      cstrName = "Function";
    }

    // Our function may have properties, so we want to format those
    // as if our function was an object
    // If we didn't find any properties, we will just append an
    // empty suffix.
    let suffix = ``;
    let refStr = "";
    if (
      ObjectKeys(value).length > 0 ||
      ObjectGetOwnPropertySymbols(value).length > 0
    ) {
      const [propString, refIndex] = inspectRawObject(
        value,
        inspectOptions,
      );
      refStr = refIndex;
      // Filter out the empty string for the case we only have
      // non-enumerable symbols.
      if (
        propString.length > 0 &&
        propString !== "{}"
      ) {
        suffix = ` ${propString}`;
      }
    }

    if (value.name && value.name !== "anonymous") {
      // from MDN spec
      return cyan(`${refStr}[${cstrName}: ${value.name}]`) + suffix;
    }
    return cyan(`${refStr}[${cstrName}]`) + suffix;
  }

  function inspectIterable(
    value,
    options,
    inspectOptions,
  ) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    if (inspectOptions.indentLevel >= inspectOptions.depth) {
      return cyan(`[${options.typeName}]`);
    }

    const entries = [];
    let iter;
    let valueIsTypedArray = false;

    switch (options.typeName) {
      case "Map":
        iter = MapPrototypeEntries(value);
        break;
      case "Set":
        iter = SetPrototypeEntries(value);
        break;
      case "Array":
        iter = ArrayPrototypeEntries(value);
        break;
      default:
        if (isTypedArray(value)) {
          iter = ArrayPrototypeEntries(value);
          valueIsTypedArray = true;
        } else {
          throw new TypeError("unreachable");
        }
    }

    let entriesLength = 0;
    const next = () => {
      return iter.next();
    };
    while (true) {
      let el;
      try {
        const res = iter.next();
        if (res.done) {
          break;
        }
        el = res.value;
      } catch (err) {
        if (valueIsTypedArray) {
          // TypedArray.prototype.entries doesn't throw, unless the ArrayBuffer
          // is detached. We don't want to show the exception in that case, so
          // we catch it here and pretend the ArrayBuffer has no entries (like
          // Chrome DevTools does).
          break;
        }
        throw err;
      }
      if (entriesLength < inspectOptions.iterableLimit) {
        inspectOptions.indentLevel++;
        ArrayPrototypePush(
          entries,
          options.entryHandler(
            el,
            inspectOptions,
            FunctionPrototypeBind(next, iter),
          ),
        );
        inspectOptions.indentLevel--;
      }
      entriesLength++;
    }

    if (options.sort) {
      ArrayPrototypeSort(entries);
    }

    if (entriesLength > inspectOptions.iterableLimit) {
      const nmore = entriesLength - inspectOptions.iterableLimit;
      ArrayPrototypePush(entries, `... ${nmore} more items`);
    }

    const iPrefix = `${options.displayName ? options.displayName + " " : ""}`;

    const level = inspectOptions.indentLevel;
    const initIndentation = `\n${
      StringPrototypeRepeat(DEFAULT_INDENT, level + 1)
    }`;
    const entryIndentation = `,\n${
      StringPrototypeRepeat(DEFAULT_INDENT, level + 1)
    }`;
    const closingDelimIndentation = StringPrototypeRepeat(
      DEFAULT_INDENT,
      level,
    );
    const closingIndentation = `${
      inspectOptions.trailingComma ? "," : ""
    }\n${closingDelimIndentation}`;

    let iContent;
    if (entries.length === 0 && !inspectOptions.compact) {
      iContent = `\n${closingDelimIndentation}`;
    } else if (options.group && entries.length > MIN_GROUP_LENGTH) {
      const groups = groupEntries(entries, level, value);
      iContent = `${initIndentation}${
        ArrayPrototypeJoin(groups, entryIndentation)
      }${closingIndentation}`;
    } else {
      iContent = entries.length === 0
        ? ""
        : ` ${ArrayPrototypeJoin(entries, ", ")} `;
      if (
        colors.stripColor(iContent).length > LINE_BREAKING_LENGTH ||
        !inspectOptions.compact
      ) {
        iContent = `${initIndentation}${
          ArrayPrototypeJoin(entries, entryIndentation)
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
      const averageBias = MathSqrt(actualMax - totalLength / entries.length);
      const biasedMax = MathMax(actualMax - 3 - averageBias, 1);
      // Dynamically check how many columns seem possible.
      const columns = MathMin(
        // Ideally a square should be drawn. We expect a character to be about 2.5
        // times as high as wide. This is the area formula to calculate a square
        // which contains n rectangles of size `actualMax * approxCharHeights`.
        // Divide that by `actualMax` to receive the correct number of columns.
        // The added bias increases the columns for short entries.
        MathRound(
          MathSqrt(approxCharHeights * biasedMax * entriesLength) / biasedMax,
        ),
        // Do not exceed the breakLength.
        MathFloor((LINE_BREAKING_LENGTH - (level + 1)) / actualMax),
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
        const max = MathMin(i + columns, entriesLength);
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
          str += StringPrototypePadStart(entries[j], padding, " ");
        } else {
          str += entries[j];
        }
        ArrayPrototypePush(tmp, str);
      }
      if (iterableLimit < entries.length) {
        ArrayPrototypePush(tmp, entries[entriesLength]);
      }
      entries = tmp;
    }
    return entries;
  }

  let circular;
  function handleCircular(value, cyan) {
    let index = 1;
    if (circular === undefined) {
      circular = new Map();
      MapPrototypeSet(circular, value, index);
    } else {
      index = MapPrototypeGet(circular, value);
      if (index === undefined) {
        index = circular.size + 1;
        MapPrototypeSet(circular, value, index);
      }
    }
    // Circular string is cyan
    return cyan(`[Circular *${index}]`);
  }

  function _inspectValue(
    value,
    inspectOptions,
  ) {
    const proxyDetails = core.getProxyDetails(value);
    if (proxyDetails != null && inspectOptions.showProxy) {
      return inspectProxy(proxyDetails, inspectOptions);
    }

    const green = maybeColor(colors.green, inspectOptions);
    const yellow = maybeColor(colors.yellow, inspectOptions);
    const gray = maybeColor(colors.gray, inspectOptions);
    const cyan = maybeColor(colors.cyan, inspectOptions);
    const bold = maybeColor(colors.bold, inspectOptions);
    const red = maybeColor(colors.red, inspectOptions);

    switch (typeof value) {
      case "string":
        return green(quoteString(value));
      case "number": // Numbers are yellow
        // Special handling of -0
        return yellow(ObjectIs(value, -0) ? "-0" : `${value}`);
      case "boolean": // booleans are yellow
        return yellow(String(value));
      case "undefined": // undefined is gray
        return gray(String(value));
      case "symbol": // Symbols are green
        return green(maybeQuoteSymbol(value));
      case "bigint": // Bigints are yellow
        return yellow(`${value}n`);
      case "function": // Function string is cyan
        if (ctxHas(value)) {
          // Circular string is cyan
          return handleCircular(value, cyan);
        }

        return inspectFunction(value, inspectOptions);
      case "object": // null is bold
        if (value === null) {
          return bold("null");
        }

        if (ctxHas(value)) {
          return handleCircular(value, cyan);
        }

        return inspectObject(value, inspectOptions, proxyDetails);
      default:
        // Not implemented is red
        return red("[Not Implemented]");
    }
  }

  function inspectValue(
    value,
    inspectOptions,
  ) {
    ArrayPrototypePush(CTX_STACK, value);
    let x;
    try {
      x = _inspectValue(value, inspectOptions);
    } finally {
      ArrayPrototypePop(CTX_STACK);
    }
    return x;
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
    const quote =
      ArrayPrototypeFind(QUOTES, (c) => !StringPrototypeIncludes(string, c)) ??
        QUOTES[0];
    const escapePattern = new RegExp(`(?=[${quote}\\\\])`, "g");
    string = StringPrototypeReplace(string, escapePattern, "\\");
    string = replaceEscapeSequences(string);
    return `${quote}${string}${quote}`;
  }

  // Replace escape sequences that can modify output.
  function replaceEscapeSequences(string) {
    return StringPrototypeReplace(
      StringPrototypeReplace(
        StringPrototypeReplace(
          StringPrototypeReplace(
            StringPrototypeReplace(
              StringPrototypeReplace(
                StringPrototypeReplace(string, /[\b]/g, "\\b"),
                /\f/g,
                "\\f",
              ),
              /\n/g,
              "\\n",
            ),
            /\r/g,
            "\\r",
          ),
          /\t/g,
          "\\t",
        ),
        /\v/g,
        "\\v",
      ),
      // deno-lint-ignore no-control-regex
      /[\x00-\x1f\x7f-\x9f]/g,
      (c) =>
        "\\x" +
        StringPrototypePadStart(
          NumberPrototypeToString(StringPrototypeCharCodeAt(c, 0), 16),
          2,
          "0",
        ),
    );
  }

  // Surround a string with quotes when it is required (e.g the string not a valid identifier).
  function maybeQuoteString(string) {
    if (RegExpPrototypeTest(/^[a-zA-Z_][a-zA-Z_0-9]*$/, string)) {
      return replaceEscapeSequences(string);
    }

    return quoteString(string);
  }

  // Surround a symbol's description in quotes when it is required (e.g the description has non printable characters).
  function maybeQuoteSymbol(symbol) {
    if (symbol.description === undefined) {
      return SymbolPrototypeToString(symbol);
    }

    if (RegExpPrototypeTest(/^[a-zA-Z_][a-zA-Z_.0-9]*$/, symbol.description)) {
      return SymbolPrototypeToString(symbol);
    }

    return `Symbol(${quoteString(symbol.description)})`;
  }

  const CTX_STACK = [];
  function ctxHas(x) {
    // Only check parent contexts
    return ArrayPrototypeIncludes(
      ArrayPrototypeSlice(CTX_STACK, 0, CTX_STACK.length - 1),
      x,
    );
  }

  // Print strings when they are inside of arrays or objects with quotes
  function inspectValueWithQuotes(
    value,
    inspectOptions,
  ) {
    const abbreviateSize =
      typeof inspectOptions.strAbbreviateSize === "undefined"
        ? STR_ABBREVIATE_SIZE
        : inspectOptions.strAbbreviateSize;
    const green = maybeColor(colors.green, inspectOptions);
    switch (typeof value) {
      case "string": {
        const trunc = value.length > abbreviateSize
          ? StringPrototypeSlice(value, 0, abbreviateSize) + "..."
          : value;
        return green(quoteString(trunc)); // Quoted strings are green
      }
      default:
        return inspectValue(value, inspectOptions);
    }
  }

  function inspectArray(
    value,
    inspectOptions,
  ) {
    const gray = maybeColor(colors.gray, inspectOptions);
    const options = {
      typeName: "Array",
      displayName: "",
      delims: ["[", "]"],
      entryHandler: (entry, inspectOptions, next) => {
        const [index, val] = entry;
        let i = index;
        if (!ObjectPrototypeHasOwnProperty(value, i)) {
          i++;
          while (!ObjectPrototypeHasOwnProperty(value, i) && i < value.length) {
            next();
            i++;
          }
          const emptyItems = i - index;
          const ending = emptyItems > 1 ? "s" : "";
          return gray(`<${emptyItems} empty item${ending}>`);
        } else {
          return inspectValueWithQuotes(val, inspectOptions);
        }
      },
      group: inspectOptions.compact,
      sort: false,
    };
    return inspectIterable(value, options, inspectOptions);
  }

  function inspectTypedArray(
    typedArrayName,
    value,
    inspectOptions,
  ) {
    const valueLength = value.length;
    const options = {
      typeName: typedArrayName,
      displayName: `${typedArrayName}(${valueLength})`,
      delims: ["[", "]"],
      entryHandler: (entry, inspectOptions) => {
        const val = entry[1];
        inspectOptions.indentLevel++;
        const inspectedValue = inspectValueWithQuotes(val, inspectOptions);
        inspectOptions.indentLevel--;
        return inspectedValue;
      },
      group: inspectOptions.compact,
      sort: false,
    };
    return inspectIterable(value, options, inspectOptions);
  }

  function inspectSet(
    value,
    inspectOptions,
  ) {
    const options = {
      typeName: "Set",
      displayName: "Set",
      delims: ["{", "}"],
      entryHandler: (entry, inspectOptions) => {
        const val = entry[1];
        inspectOptions.indentLevel++;
        const inspectedValue = inspectValueWithQuotes(val, inspectOptions);
        inspectOptions.indentLevel--;
        return inspectedValue;
      },
      group: false,
      sort: inspectOptions.sorted,
    };
    return inspectIterable(value, options, inspectOptions);
  }

  function inspectMap(
    value,
    inspectOptions,
  ) {
    const options = {
      typeName: "Map",
      displayName: "Map",
      delims: ["{", "}"],
      entryHandler: (entry, inspectOptions) => {
        const [key, val] = entry;
        inspectOptions.indentLevel++;
        const inspectedValue = `${
          inspectValueWithQuotes(key, inspectOptions)
        } => ${inspectValueWithQuotes(val, inspectOptions)}`;
        inspectOptions.indentLevel--;
        return inspectedValue;
      },
      group: false,
      sort: inspectOptions.sorted,
    };
    return inspectIterable(
      value,
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
    return magenta(
      isInvalidDate(value) ? "Invalid Date" : DatePrototypeToISOString(value),
    );
  }

  function inspectRegExp(value, inspectOptions) {
    const red = maybeColor(colors.red, inspectOptions);
    return red(RegExpPrototypeToString(value)); // RegExps are red
  }

  function inspectError(value, cyan) {
    const causes = [value];

    let err = value;
    while (err.cause) {
      if (ArrayPrototypeIncludes(causes, err.cause)) {
        ArrayPrototypePush(causes, handleCircular(err.cause, cyan));
        break;
      } else {
        ArrayPrototypePush(causes, err.cause);
        err = err.cause;
      }
    }

    const refMap = new Map();
    for (const cause of causes) {
      if (circular !== undefined) {
        const index = MapPrototypeGet(circular, cause);
        if (index !== undefined) {
          MapPrototypeSet(refMap, cause, cyan(`<ref *${index}> `));
        }
      }
    }
    ArrayPrototypeShift(causes);

    let finalMessage = (MapPrototypeGet(refMap, value) ?? "");

    if (ObjectPrototypeIsPrototypeOf(AggregateErrorPrototype, value)) {
      const stackLines = StringPrototypeSplit(value.stack, "\n");
      while (true) {
        const line = ArrayPrototypeShift(stackLines);
        if (RegExpPrototypeTest(/\s+at/, line)) {
          ArrayPrototypeUnshift(stackLines, line);
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
              /^(?!\s*$)/gm,
              StringPrototypeRepeat(" ", 4),
            ),
        ),
        "\n",
      );
      finalMessage += aggregateMessage;
      finalMessage += "\n";
      finalMessage += ArrayPrototypeJoin(stackLines, "\n");
    } else {
      finalMessage += value.stack;
    }

    finalMessage += ArrayPrototypeJoin(
      ArrayPrototypeMap(
        causes,
        (cause) =>
          "\nCaused by " + (MapPrototypeGet(refMap, cause) ?? "") +
          (cause?.stack ?? cause),
      ),
      "",
    );

    return finalMessage;
  }

  function inspectStringObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[String: "${StringPrototypeToString(value)}"]`); // wrappers are in cyan
  }

  function inspectBooleanObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[Boolean: ${BooleanPrototypeToString(value)}]`); // wrappers are in cyan
  }

  function inspectNumberObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    // Special handling of -0
    return cyan(
      `[Number: ${
        ObjectIs(NumberPrototypeValueOf(value), -0)
          ? "-0"
          : NumberPrototypeToString(value)
      }]`,
    ); // wrappers are in cyan
  }

  function inspectBigIntObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[BigInt: ${BigIntPrototypeToString(value)}n]`); // wrappers are in cyan
  }

  function inspectSymbolObject(value, inspectOptions) {
    const cyan = maybeColor(colors.cyan, inspectOptions);
    return cyan(`[Symbol: ${maybeQuoteSymbol(SymbolPrototypeValueOf(value))}]`); // wrappers are in cyan
  }

  const PromiseState = {
    Pending: 0,
    Fulfilled: 1,
    Rejected: 2,
  };

  function inspectPromise(
    value,
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

    inspectOptions.indentLevel++;
    const str = `${prefix}${inspectValueWithQuotes(result, inspectOptions)}`;
    inspectOptions.indentLevel--;

    if (str.length + PROMISE_STRING_BASE_LENGTH > LINE_BREAKING_LENGTH) {
      return `Promise {\n${
        StringPrototypeRepeat(DEFAULT_INDENT, inspectOptions.indentLevel + 1)
      }${str}\n}`;
    }

    return `Promise { ${str} }`;
  }

  function inspectProxy(
    targetAndHandler,
    inspectOptions,
  ) {
    return `Proxy ${inspectArray(targetAndHandler, inspectOptions)}`;
  }

  function inspectRawObject(
    value,
    inspectOptions,
  ) {
    const cyan = maybeColor(colors.cyan, inspectOptions);

    if (inspectOptions.indentLevel >= inspectOptions.depth) {
      return [cyan("[Object]"), ""]; // wrappers are in cyan
    }

    let baseString;

    let shouldShowDisplayName = false;
    let displayName = value[
      SymbolToStringTag
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
    const stringKeys = ObjectKeys(value);
    const symbolKeys = ObjectGetOwnPropertySymbols(value);
    if (inspectOptions.sorted) {
      ArrayPrototypeSort(stringKeys);
      ArrayPrototypeSort(
        symbolKeys,
        (s1, s2) =>
          StringPrototypeLocaleCompare(
            s1.description ?? "",
            s2.description ?? "",
          ),
      );
    }

    const red = maybeColor(colors.red, inspectOptions);

    inspectOptions.indentLevel++;

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
          ? inspectValueWithQuotes(propertyValue, inspectOptions)
          : red(`[Thrown ${error.name}: ${error.message}]`);
        ArrayPrototypePush(
          entries,
          `${maybeQuoteString(key)}: ${inspectedValue}`,
        );
      } else {
        const descriptor = ObjectGetOwnPropertyDescriptor(value, key);
        if (descriptor.get !== undefined && descriptor.set !== undefined) {
          ArrayPrototypePush(
            entries,
            `${maybeQuoteString(key)}: [Getter/Setter]`,
          );
        } else if (descriptor.get !== undefined) {
          ArrayPrototypePush(entries, `${maybeQuoteString(key)}: [Getter]`);
        } else {
          ArrayPrototypePush(
            entries,
            `${maybeQuoteString(key)}: ${
              inspectValueWithQuotes(value[key], inspectOptions)
            }`,
          );
        }
      }
    }

    for (const key of symbolKeys) {
      if (
        !inspectOptions.showHidden &&
        !propertyIsEnumerable(value, key)
      ) {
        continue;
      }

      if (inspectOptions.getters) {
        let propertyValue;
        let error;
        try {
          propertyValue = value[key];
        } catch (error_) {
          error = error_;
        }
        const inspectedValue = error == null
          ? inspectValueWithQuotes(propertyValue, inspectOptions)
          : red(`Thrown ${error.name}: ${error.message}`);
        ArrayPrototypePush(
          entries,
          `[${maybeQuoteSymbol(key)}]: ${inspectedValue}`,
        );
      } else {
        const descriptor = ObjectGetOwnPropertyDescriptor(value, key);
        if (descriptor.get !== undefined && descriptor.set !== undefined) {
          ArrayPrototypePush(
            entries,
            `[${maybeQuoteSymbol(key)}]: [Getter/Setter]`,
          );
        } else if (descriptor.get !== undefined) {
          ArrayPrototypePush(entries, `[${maybeQuoteSymbol(key)}]: [Getter]`);
        } else {
          ArrayPrototypePush(
            entries,
            `[${maybeQuoteSymbol(key)}]: ${
              inspectValueWithQuotes(value[key], inspectOptions)
            }`,
          );
        }
      }
    }

    inspectOptions.indentLevel--;

    // Making sure color codes are ignored when calculating the total length
    const totalLength = entries.length + inspectOptions.indentLevel +
      colors.stripColor(ArrayPrototypeJoin(entries, "")).length;

    if (entries.length === 0) {
      baseString = "{}";
    } else if (totalLength > LINE_BREAKING_LENGTH || !inspectOptions.compact) {
      const entryIndent = StringPrototypeRepeat(
        DEFAULT_INDENT,
        inspectOptions.indentLevel + 1,
      );
      const closingIndent = StringPrototypeRepeat(
        DEFAULT_INDENT,
        inspectOptions.indentLevel,
      );
      baseString = `{\n${entryIndent}${
        ArrayPrototypeJoin(entries, `,\n${entryIndent}`)
      }${inspectOptions.trailingComma ? "," : ""}\n${closingIndent}}`;
    } else {
      baseString = `{ ${ArrayPrototypeJoin(entries, ", ")} }`;
    }

    if (shouldShowDisplayName) {
      baseString = `${displayName} ${baseString}`;
    }

    let refIndex = "";
    if (circular !== undefined) {
      const index = MapPrototypeGet(circular, value);
      if (index !== undefined) {
        refIndex = cyan(`<ref *${index}> `);
      }
    }

    return [baseString, refIndex];
  }

  function inspectObject(
    value,
    inspectOptions,
    proxyDetails,
  ) {
    if (
      ReflectHas(value, customInspect) &&
      typeof value[customInspect] === "function"
    ) {
      return String(value[customInspect](inspect, inspectOptions));
    }
    // This non-unique symbol is used to support op_crates, ie.
    // in extensions/web we don't want to depend on public
    // Symbol.for("Deno.customInspect") symbol defined in the public API.
    // Internal only, shouldn't be used by users.
    const privateCustomInspect = SymbolFor("Deno.privateCustomInspect");
    if (
      ReflectHas(value, privateCustomInspect) &&
      typeof value[privateCustomInspect] === "function"
    ) {
      // TODO(nayeemrmn): `inspect` is passed as an argument because custom
      // inspect implementations in `extensions` need it, but may not have access
      // to the `Deno` namespace in web workers. Remove when the `Deno`
      // namespace is always enabled.
      return String(
        value[privateCustomInspect](inspect, inspectOptions),
      );
    }
    if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, value)) {
      return inspectError(value, maybeColor(colors.cyan, inspectOptions));
    } else if (ArrayIsArray(value)) {
      return inspectArray(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(NumberPrototype, value)) {
      return inspectNumberObject(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(BigIntPrototype, value)) {
      return inspectBigIntObject(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(BooleanPrototype, value)) {
      return inspectBooleanObject(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(StringPrototype, value)) {
      return inspectStringObject(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(SymbolPrototype, value)) {
      return inspectSymbolObject(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(PromisePrototype, value)) {
      return inspectPromise(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(RegExpPrototype, value)) {
      return inspectRegExp(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(DatePrototype, value)) {
      return inspectDate(value, inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(SetPrototype, value)) {
      return inspectSet(
        proxyDetails ? proxyDetails[0] : value,
        inspectOptions,
      );
    } else if (ObjectPrototypeIsPrototypeOf(MapPrototype, value)) {
      return inspectMap(
        proxyDetails ? proxyDetails[0] : value,
        inspectOptions,
      );
    } else if (ObjectPrototypeIsPrototypeOf(WeakSetPrototype, value)) {
      return inspectWeakSet(inspectOptions);
    } else if (ObjectPrototypeIsPrototypeOf(WeakMapPrototype, value)) {
      return inspectWeakMap(inspectOptions);
    } else if (isTypedArray(value)) {
      return inspectTypedArray(
        ObjectGetPrototypeOf(value).constructor.name,
        value,
        inspectOptions,
      );
    } else {
      // Otherwise, default object formatting
      let [insp, refIndex] = inspectRawObject(value, inspectOptions);
      insp = refIndex + insp;
      return insp;
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
    if (MapPrototypeHas(colorKeywords, colorString)) {
      colorString = MapPrototypeGet(colorKeywords, colorString);
    }
    // deno-fmt-ignore
    const hashMatch = StringPrototypeMatch(colorString, /^#([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})([\dA-Fa-f]{2})?$/);
    if (hashMatch != null) {
      return [
        Number(`0x${hashMatch[1]}`),
        Number(`0x${hashMatch[2]}`),
        Number(`0x${hashMatch[3]}`),
      ];
    }
    // deno-fmt-ignore
    const smallHashMatch = StringPrototypeMatch(colorString, /^#([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])([\dA-Fa-f])?$/);
    if (smallHashMatch != null) {
      return [
        Number(`0x${smallHashMatch[1]}0`),
        Number(`0x${smallHashMatch[2]}0`),
        Number(`0x${smallHashMatch[3]}0`),
      ];
    }
    // deno-fmt-ignore
    const rgbMatch = StringPrototypeMatch(colorString, /^rgba?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/);
    if (rgbMatch != null) {
      return [
        MathRound(MathMax(0, MathMin(255, Number(rgbMatch[1])))),
        MathRound(MathMax(0, MathMin(255, Number(rgbMatch[2])))),
        MathRound(MathMax(0, MathMin(255, Number(rgbMatch[3])))),
      ];
    }
    // deno-fmt-ignore
    const hslMatch = StringPrototypeMatch(colorString, /^hsla?\(\s*([+\-]?\d*\.?\d+)\s*,\s*([+\-]?\d*\.?\d+)%\s*,\s*([+\-]?\d*\.?\d+)%\s*(,\s*([+\-]?\d*\.?\d+)\s*)?\)$/);
    if (hslMatch != null) {
      // https://www.rapidtables.com/convert/color/hsl-to-rgb.html
      let h = Number(hslMatch[1]) % 360;
      if (h < 0) {
        h += 360;
      }
      const s = MathMax(0, MathMin(100, Number(hslMatch[2]))) / 100;
      const l = MathMax(0, MathMin(100, Number(hslMatch[3]))) / 100;
      const c = (1 - MathAbs(2 * l - 1)) * s;
      const x = c * (1 - MathAbs((h / 60) % 2 - 1));
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

    for (const [key, value] of rawEntries) {
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
        for (const lineType of StringPrototypeSplit(value, /\s+/g)) {
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
        for (const arg of StringPrototypeSplit(value, /\s+/g)) {
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
          const [r, g, b] = css.backgroundColor;
          ansi += `\x1b[48;2;${r};${g};${b}m`;
        } else {
          const parsed = parseCssColor(css.backgroundColor);
          if (parsed !== null) {
            const [r, g, b] = parsed;
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
          const [r, g, b] = css.color;
          ansi += `\x1b[38;2;${r};${g};${b}m`;
        } else {
          const parsed = parseCssColor(css.color);
          if (parsed !== null) {
            const [r, g, b] = parsed;
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
        const [r, g, b] = css.textDecorationColor;
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

  function inspectArgs(args, inspectOptions = {}) {
    circular = undefined;

    const noColor = colors.getNoColor();
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
              formattedArg = inspectValue(args[a++], rInspectOptions);
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
        string += inspectValue(args[a], rInspectOptions);
      }
    }

    if (rInspectOptions.indentLevel > 0) {
      const groupIndent = StringPrototypeRepeat(
        DEFAULT_INDENT,
        rInspectOptions.indentLevel,
      );
      string = groupIndent +
        StringPrototypeReplaceAll(string, "\n", `\n${groupIndent}`);
    }

    return string;
  }

  const countMap = new Map();
  const timerMap = new Map();
  const isConsoleInstance = Symbol("isConsoleInstance");

  function getConsoleInspectOptions() {
    return {
      ...DEFAULT_INSPECT_OPTIONS,
      colors: !colors.getNoColor(),
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
          ...getConsoleInspectOptions(),
          indentLevel: this.indentLevel,
        }) + "\n",
        1,
      );
    };

    debug = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
          indentLevel: this.indentLevel,
        }) + "\n",
        0,
      );
    };

    info = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
          indentLevel: this.indentLevel,
        }) + "\n",
        1,
      );
    };

    dir = (obj = undefined, options = {}) => {
      this.#printFunc(
        inspectArgs([obj], { ...getConsoleInspectOptions(), ...options }) +
          "\n",
        1,
      );
    };

    dirxml = this.dir;

    warn = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
          indentLevel: this.indentLevel,
        }) + "\n",
        2,
      );
    };

    error = (...args) => {
      this.#printFunc(
        inspectArgs(args, {
          ...getConsoleInspectOptions(),
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

      const [first, ...rest] = args;

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
          "The 'properties' argument must be of type Array. " +
            "Received type string",
        );
      }

      if (data === null || typeof data !== "object") {
        return this.log(data);
      }

      const stringifyValue = (value) =>
        inspectValueWithQuotes(value, {
          ...DEFAULT_INSPECT_OPTIONS,
          depth: 1,
        });
      const toTable = (header, body) => this.log(cliTable(header, body));

      let resultData;
      const isSet = ObjectPrototypeIsPrototypeOf(SetPrototype, data);
      const isMap = ObjectPrototypeIsPrototypeOf(MapPrototype, data);
      const valuesKey = "Values";
      const indexKey = isSet || isMap ? "(iter idx)" : "(idx)";

      if (isSet) {
        resultData = [...new SafeSet(data)];
      } else if (isMap) {
        let idx = 0;
        resultData = {};

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
      keys.forEach((k, idx) => {
        const value = resultData[k];
        const primitive = value === null ||
          (typeof value !== "function" && typeof value !== "object");
        if (properties === undefined && primitive) {
          hasPrimitives = true;
          ArrayPrototypePush(values, stringifyValue(value));
        } else {
          const valueObj = value || {};
          const keys = properties || ObjectKeys(valueObj);
          for (const k of keys) {
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
          !isMap && hasPrimitives && valuesKey,
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

      MapPrototypeSet(timerMap, label, DateNow());
    };

    timeLog = (label = "default", ...args) => {
      label = String(label);

      if (!MapPrototypeHas(timerMap, label)) {
        this.warn(`Timer '${label}' does not exists`);
        return;
      }

      const startTime = MapPrototypeGet(timerMap, label);
      const duration = DateNow() - startTime;

      this.info(`${label}: ${duration}ms`, ...new SafeArrayIterator(args));
    };

    timeEnd = (label = "default") => {
      label = String(label);

      if (!MapPrototypeHas(timerMap, label)) {
        this.warn(`Timer '${label}' does not exists`);
        return;
      }

      const startTime = MapPrototypeGet(timerMap, label);
      MapPrototypeDelete(timerMap, label);
      const duration = DateNow() - startTime;

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
        { ...getConsoleInspectOptions(), indentLevel: 0 },
      );
      const err = {
        name: "Trace",
        message,
      };
      ErrorCaptureStackTrace(err, this.trace);
      this.error(err.stack);
    };

    static [SymbolHasInstance](instance) {
      return instance[isConsoleInstance];
    }
  }

  const customInspect = SymbolFor("Deno.customInspect");

  function inspect(
    value,
    inspectOptions = {},
  ) {
    circular = undefined;
    return inspectValue(value, {
      ...DEFAULT_INSPECT_OPTIONS,
      ...inspectOptions,
    });
  }

  /** Creates a proxy that represents a subset of the properties
   * of the original object optionally without evaluating the properties
   * in order to get the values. */
  function createFilteredInspectProxy({ object, keys, evaluate }) {
    return new Proxy({}, {
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

  // A helper function that will bind our own console implementation
  // with default implementation of Console from V8. This will cause
  // console messages to be piped to inspector console.
  //
  // We are using `Deno.core.callConsole` binding to preserve proper stack
  // frames in inspector console. This has to be done because V8 considers
  // the last JS stack frame as gospel for the inspector. In our case we
  // specifically want the latest user stack frame to be the one that matters
  // though.
  //
  // Inspired by:
  // https://github.com/nodejs/node/blob/1317252dfe8824fd9cfee125d2aaa94004db2f3b/lib/internal/util/inspector.js#L39-L61
  function wrapConsole(consoleFromDeno, consoleFromV8) {
    const callConsole = core.callConsole;

    for (const key of ObjectKeys(consoleFromV8)) {
      if (ObjectPrototypeHasOwnProperty(consoleFromDeno, key)) {
        consoleFromDeno[key] = FunctionPrototypeBind(
          callConsole,
          consoleFromDeno,
          consoleFromV8[key],
          consoleFromDeno[key],
        );
      }
    }
  }

  // Expose these fields to internalObject for tests.
  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    Console,
    cssToAnsi,
    inspectArgs,
    parseCss,
    parseCssColor,
  };

  window.__bootstrap.console = {
    CSI,
    inspectArgs,
    Console,
    customInspect,
    inspect,
    wrapConsole,
    createFilteredInspectProxy,
    quoteString,
  };
})(this);
