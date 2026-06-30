// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import {
  AMPERSAND,
  ASTERISK,
  BOM,
  COLON,
  COMMA,
  COMMERCIAL_AT,
  DOUBLE_QUOTE,
  EXCLAMATION,
  GRAVE_ACCENT,
  GREATER_THAN,
  isWhiteSpace,
  LEFT_CURLY_BRACKET,
  LEFT_SQUARE_BRACKET,
  LINE_FEED,
  MINUS,
  PERCENT,
  QUESTION,
  RIGHT_CURLY_BRACKET,
  RIGHT_SQUARE_BRACKET,
  SHARP,
  SINGLE_QUOTE,
  VERTICAL_LINE,
} from "./_chars.ts";
import { DEFAULT_SCHEMA, type Schema } from "./_schema.ts";
import type { KindType, StyleVariant, Type } from "./_type.ts";
import { isObject } from "./_utils.ts";

const STYLE_PLAIN = 1;
const STYLE_SINGLE = 2;
const STYLE_LITERAL = 3;
const STYLE_FOLDED = 4;
const STYLE_DOUBLE = 5;

const LEADING_SPACE_REGEXP = /^\n* /;

const ESCAPE_SEQUENCES = new Map<number, string>([
  [0x00, "\\0"],
  [0x07, "\\a"],
  [0x08, "\\b"],
  [0x09, "\\t"],
  [0x0a, "\\n"],
  [0x0b, "\\v"],
  [0x0c, "\\f"],
  [0x0d, "\\r"],
  [0x1b, "\\e"],
  [0x22, '\\"'],
  [0x5c, "\\\\"],
  [0x85, "\\N"],
  [0xa0, "\\_"],
  [0x2028, "\\L"],
  [0x2029, "\\P"],
]);

const DEPRECATED_BOOLEANS_SYNTAX = new Set([
  "y",
  "Y",
  "yes",
  "Yes",
  "YES",
  "on",
  "On",
  "ON",
  "n",
  "N",
  "no",
  "No",
  "NO",
  "off",
  "Off",
  "OFF",
]);

/**
 * Encodes a Unicode character code point as a hexadecimal escape sequence.
 */
function charCodeToHexString(charCode: number): string {
  const hexString = charCode.toString(16).toUpperCase();
  if (charCode <= 0xff) return `\\x${hexString.padStart(2, "0")}`;
  if (charCode <= 0xffff) return `\\u${hexString.padStart(4, "0")}`;
  if (charCode <= 0xffffffff) return `\\U${hexString.padStart(8, "0")}`;
  throw new Error(
    "Code point within a string may not be greater than 0xFFFFFFFF",
  );
}

function createStyleMap(
  map: Record<string, StyleVariant>,
): Map<string, StyleVariant> {
  const result = new Map();
  for (let tag of Object.keys(map)) {
    const style = String(map[tag]) as StyleVariant;
    if (tag.slice(0, 2) === "!!") {
      tag = `tag:yaml.org,2002:${tag.slice(2)}`;
    }
    result.set(tag, style);
  }
  return result;
}

// Indents every line in a string. Empty lines (\n only) are not indented.
function indentString(string: string, spaces: number): string {
  const indent = " ".repeat(spaces);
  return string
    .split("\n")
    .map((line) => line.length ? indent + line : line)
    .join("\n");
}

function generateNextLine(indent: number, level: number): string {
  return `\n${" ".repeat(indent * level)}`;
}

/**
 * @link https://yaml.org/spec/1.2.2/ 5.1. Character Set
 * @return `true` if the character is printable without escaping, `false` otherwise.
 */
function isPrintable(c: number): boolean {
  return (
    (0x00020 <= c && c <= 0x00007e) ||
    (0x000a1 <= c && c <= 0x00d7ff && c !== 0x2028 && c !== 0x2029) ||
    (0x0e000 <= c && c <= 0x00fffd && c !== BOM) ||
    (0x10000 <= c && c <= 0x10ffff)
  );
}

/**
 * @return `true` if value is allowed after the first character in plain style, `false` otherwise.
 */
function isPlainSafe(c: number): boolean {
  return (
    isPrintable(c) &&
    c !== BOM &&
    c !== COMMA &&
    c !== LEFT_SQUARE_BRACKET &&
    c !== RIGHT_SQUARE_BRACKET &&
    c !== LEFT_CURLY_BRACKET &&
    c !== RIGHT_CURLY_BRACKET &&
    c !== COLON &&
    c !== SHARP
  );
}

/**
 * @return `true` if value is allowed as the first character in plain style, `false` otherwise.
 */
function isPlainSafeFirst(c: number): boolean {
  return (
    isPlainSafe(c) &&
    !isWhiteSpace(c) &&
    c !== MINUS &&
    c !== QUESTION &&
    c !== AMPERSAND &&
    c !== ASTERISK &&
    c !== EXCLAMATION &&
    c !== VERTICAL_LINE &&
    c !== GREATER_THAN &&
    c !== SINGLE_QUOTE &&
    c !== DOUBLE_QUOTE &&
    c !== PERCENT &&
    c !== COMMERCIAL_AT &&
    c !== GRAVE_ACCENT
  );
}

// Determines whether block indentation indicator is required.
function needIndentIndicator(string: string): boolean {
  return LEADING_SPACE_REGEXP.test(string);
}

// Determines which scalar styles are possible and returns the preferred style.
// lineWidth = -1 => no limit.
// Pre-conditions: str.length > 0.
// Post-conditions:
//  STYLE_PLAIN or STYLE_SINGLE => no \n are in the string.
//  STYLE_LITERAL => no lines are suitable for folding (or lineWidth is -1).
//  STYLE_FOLDED => a line > lineWidth and can be folded (and lineWidth !== -1).
function chooseScalarStyle(
  string: string,
  singleLineOnly: boolean,
  indentPerLevel: number,
  lineWidth: number,
  implicitTypes: Type<"scalar", unknown>[],
  quoteStyle: "'" | '"',
): number {
  const shouldTrackWidth = lineWidth !== -1;
  let hasLineBreak = false;
  let hasFoldableLine = false; // only checked if shouldTrackWidth
  let previousLineBreak = -1; // count the first line correctly
  let plain = isPlainSafeFirst(string.charCodeAt(0)) &&
    !isWhiteSpace(string.charCodeAt(string.length - 1));

  let char: number;
  let i: number;
  if (singleLineOnly) {
    // Case: no block styles.
    // Check for disallowed characters to rule out plain and single.
    for (i = 0; i < string.length; i++) {
      char = string.charCodeAt(i);
      if (!isPrintable(char)) {
        return STYLE_DOUBLE;
      }
      plain = plain && isPlainSafe(char);
    }
  } else {
    // Case: block styles permitted.
    for (i = 0; i < string.length; i++) {
      char = string.charCodeAt(i);
      if (char === LINE_FEED) {
        hasLineBreak = true;
        // Check if any line can be folded.
        if (shouldTrackWidth) {
          hasFoldableLine = hasFoldableLine ||
            // Foldable line = too long, and not more-indented.
            (i - previousLineBreak - 1 > lineWidth &&
              string[previousLineBreak + 1] !== " ");
          previousLineBreak = i;
        }
      } else if (!isPrintable(char)) {
        return STYLE_DOUBLE;
      }
      plain = plain && isPlainSafe(char);
    }
    // in case the end is missing a \n
    hasFoldableLine = hasFoldableLine ||
      (shouldTrackWidth &&
        i - previousLineBreak - 1 > lineWidth &&
        string[previousLineBreak + 1] !== " ");
  }
  // Although every style can represent \n without escaping, prefer block styles
  // for multiline, since they're more readable and they don't add empty lines.
  // Also prefer folding a super-long line.
  if (!hasLineBreak && !hasFoldableLine) {
    // Strings interpretable as another type have to be quoted;
    // e.g. the string 'true' vs. the boolean true.
    return plain && !implicitTypes.some((type) => type.resolve(string))
      ? STYLE_PLAIN
      : quoteStyle === "'"
      ? STYLE_SINGLE
      : STYLE_DOUBLE;
  }
  // Edge case: block indentation indicator can only have one digit.
  if (indentPerLevel > 9 && needIndentIndicator(string)) {
    return STYLE_DOUBLE;
  }
  // At this point we know block styles are valid.
  // Prefer literal style unless we want to fold.
  return hasFoldableLine ? STYLE_FOLDED : STYLE_LITERAL;
}

// Greedy line breaking.
// Picks the longest line under the limit each time,
// otherwise settles for the shortest line over the limit.
// NB. More-indented lines *cannot* be folded, as that would add an extra \n.
function foldLine(line: string, width: number): string {
  if (line === "" || line[0] === " ") return line;

  // Since a more-indented line adds a \n, breaks can't be followed by a space.
  const breakRegExp = / [^ ]/g; // note: the match index will always be <= length-2.
  // start is an inclusive index. end, curr, and next are exclusive.
  let start = 0;
  let end;
  let curr = 0;
  let next = 0;
  const lines = [];

  // Invariants: 0 <= start <= length-1.
  //   0 <= curr <= next <= max(0, length-2). curr - start <= width.
  // Inside the loop:
  //   A match implies length >= 2, so curr and next are <= length-2.
  for (const match of line.matchAll(breakRegExp)) {
    next = match.index;
    // maintain invariant: curr - start <= width
    if (next - start > width) {
      end = curr > start ? curr : next; // derive end <= length-2
      lines.push(line.slice(start, end));
      // skip the space that was output as \n
      start = end + 1; // derive start <= length-1
    }
    curr = next;
  }

  // By the invariants, start <= length-1, so there is something left over.
  // It is either the whole string or a part starting from non-whitespace.
  // Insert a break if the remainder is too long and there is a break available.
  if (line.length - start > width && curr > start) {
    lines.push(line.slice(start, curr));
    lines.push(line.slice(curr + 1));
  } else {
    lines.push(line.slice(start));
  }

  return lines.join("\n");
}

function trimTrailingNewline(string: string) {
  return string.at(-1) === "\n" ? string.slice(0, -1) : string;
}

// Note: a long line without a suitable break point will exceed the width limit.
// Pre-conditions: every char in str isPrintable, str.length > 0, width > 0.
function foldString(string: string, width: number): string {
  // In folded style, $k$ consecutive newlines output as $k+1$ newlines—
  // unless they're before or after a more-indented line, or at the very
  // beginning or end, in which case $k$ maps to $k$.
  // Therefore, parse each chunk as newline(s) followed by a content line.
  const lineRe = /(\n+)([^\n]*)/g;

  // first line (possibly an empty line)
  let result = ((): string => {
    let nextLF = string.indexOf("\n");
    nextLF = nextLF !== -1 ? nextLF : string.length;
    lineRe.lastIndex = nextLF;
    return foldLine(string.slice(0, nextLF), width);
  })();
  // If we haven't reached the first content line yet, don't add an extra \n.
  let prevMoreIndented = string[0] === "\n" || string[0] === " ";
  let moreIndented;

  // rest of the lines
  let match;
  // tslint:disable-next-line:no-conditional-assignment
  while ((match = lineRe.exec(string))) {
    const prefix = match[1];
    const line = match[2] || "";
    moreIndented = line[0] === " ";
    result += prefix +
      (!prevMoreIndented && !moreIndented && line !== "" ? "\n" : "") +
      foldLine(line, width);
    prevMoreIndented = moreIndented;
  }

  return result;
}

// Escapes a double-quoted string.
function escapeString(string: string): string {
  let result = "";
  let char;
  let nextChar;
  let escapeSeq;

  for (let i = 0; i < string.length; i++) {
    char = string.charCodeAt(i);
    // Check for surrogate pairs (reference Unicode 3.0 section "3.7 Surrogates").
    if (char >= 0xd800 && char <= 0xdbff /* high surrogate */) {
      nextChar = string.charCodeAt(i + 1);
      if (nextChar >= 0xdc00 && nextChar <= 0xdfff /* low surrogate */) {
        // Combine the surrogate pair and store it escaped.
        result += charCodeToHexString(
          (char - 0xd800) * 0x400 + nextChar - 0xdc00 + 0x10000,
        );
        // Advance index one extra since we already used that char here.
        i++;
        continue;
      }
    }
    escapeSeq = ESCAPE_SEQUENCES.get(char);
    result += !escapeSeq && isPrintable(char)
      ? string[i]
      : escapeSeq || charCodeToHexString(char);
  }

  return result;
}

// Pre-conditions: string is valid for a block scalar, 1 <= indentPerLevel <= 9.
function blockHeader(string: string, indentPerLevel: number): string {
  const indentIndicator = needIndentIndicator(string)
    ? String(indentPerLevel)
    : "";

  // note the special case: the string '\n' counts as a "trailing" empty line.
  const clip = string[string.length - 1] === "\n";
  const keep = clip && (string[string.length - 2] === "\n" || string === "\n");
  const chomp = keep ? "+" : clip ? "" : "-";

  return `${indentIndicator}${chomp}\n`;
}

function getDuplicateObjects(root: unknown): unknown[] {
  const seenObjects = new Set();
  const duplicateObjects = new Set();
  const queue = [root];

  for (let i = 0; i < queue.length; i++) {
    const value = queue[i];
    if (!isObject(value)) continue;
    if (seenObjects.has(value)) {
      duplicateObjects.add(value);
      continue;
    }
    seenObjects.add(value);
    const children = Array.isArray(value) ? value : Object.values(value);
    queue.push(...children);
  }

  return [...duplicateObjects];
}
function stringifyValue(value: unknown, tag: string | null) {
  if (tag !== null && tag !== "?") return `!<${tag}> ${value}`;
  return value as string;
}

export interface DumperStateOptions {
  /** indentation width to use (in spaces). */
  indent?: number;
  /** when true, adds an indentation level to array elements */
  arrayIndent?: boolean;
  /**
   * do not throw on invalid types (like function in the safe schema)
   * and skip pairs and single values with such types.
   */
  skipInvalid?: boolean;
  /**
   * specifies level of nesting, when to switch from
   * block to flow style for collections. -1 means block style everywhere
   */
  flowLevel?: number;
  /** Each tag may have own set of styles.	- "tag" => "style" map. */
  styles?: Record<string, StyleVariant>;
  /** specifies a schema to use. */
  schema?: Schema;
  /**
   * If true, sort keys when dumping YAML in ascending, ASCII character order.
   * If a function, use the function to sort the keys. (default: false)
   * If a function is specified, the function must return a negative value
   * if first argument is less than second argument, zero if they're equal
   * and a positive value otherwise.
   */
  sortKeys?: boolean | ((a: string, b: string, depth: number) => number);
  /** set max line width. (default: 80) */
  lineWidth?: number;
  /**
   * if false, don't convert duplicate objects
   * into references (default: true)
   */
  useAnchors?: boolean;
  /**
   * if false don't try to be compatible with older yaml versions.
   * Currently: don't quote "yes", "no" and so on,
   * as required for YAML 1.1 (default: true)
   */
  compatMode?: boolean;
  /**
   * if true flow sequences will be condensed, omitting the
   * space between `key: value` or `a, b`. Eg. `'[a,b]'` or `{a:{b:c}}`.
   * Can be useful when using yaml for pretty URL query params
   * as spaces are %-encoded. (default: false).
   */
  condenseFlow?: boolean;
  /**
   * Strings will be quoted using this quoting style.
   * If you specify single quotes, double quotes will still be used
   * for non-printable characters. (default: "'")
   */
  quoteStyle?: "'" | '"';
}

export class DumperState {
  indent: number;
  arrayIndent: boolean;
  skipInvalid: boolean;
  flowLevel: number;
  sortKeys: boolean | ((a: string, b: string, depth: number) => number);
  lineWidth: number;
  useAnchors: boolean;
  compatMode: boolean;
  condenseFlow: boolean;
  implicitTypes: Type<"scalar">[];
  explicitTypes: Type<KindType>[];
  duplicates: unknown[] = [];
  usedDuplicates: Set<unknown> = new Set();
  styleMap: Map<string, StyleVariant> = new Map();
  quoteStyle: "'" | '"';

  constructor({
    schema = DEFAULT_SCHEMA,
    indent = 2,
    arrayIndent = true,
    skipInvalid = false,
    flowLevel = -1,
    styles = undefined,
    sortKeys = false,
    lineWidth = 80,
    useAnchors = true,
    compatMode = true,
    condenseFlow = false,
    quoteStyle = "'",
  }: DumperStateOptions) {
    this.indent = Math.max(1, indent);
    this.arrayIndent = arrayIndent;
    this.skipInvalid = skipInvalid;
    this.flowLevel = flowLevel;
    if (styles) this.styleMap = createStyleMap(styles);
    this.sortKeys = sortKeys;
    this.lineWidth = lineWidth;
    this.useAnchors = useAnchors;
    this.compatMode = compatMode;
    this.condenseFlow = condenseFlow;
    this.implicitTypes = schema.implicitTypes;
    this.explicitTypes = schema.explicitTypes;
    this.quoteStyle = quoteStyle;
  }

  // Note: line breaking/folding is implemented for only the folded style.
  // NB. We drop the last trailing newline (if any) of a returned block scalar
  //  since the dumper adds its own newline. This always works:
  //    • No ending newline => unaffected; already using strip "-" chomping.
  //    • Ending newline    => removed then restored.
  //  Importantly, this keeps the "+" chomp indicator from gaining an extra line.
  stringifyScalar(
    string: string,
    { level, isKey }: { level: number; isKey: boolean },
  ): string {
    if (string.length === 0) {
      return "''";
    }
    if (this.compatMode && DEPRECATED_BOOLEANS_SYNTAX.has(string)) {
      return `'${string}'`;
    }

    const indent = this.indent * Math.max(1, level); // no 0-indent scalars
    // As indentation gets deeper, let the width decrease monotonically
    // to the lower bound min(this.lineWidth, 40).
    // Note that this implies
    //  this.lineWidth ≤ 40 + this.indent: width is fixed at the lower bound.
    //  this.lineWidth > 40 + this.indent: width decreases until the lower
    //  bound.
    // This behaves better than a constant minimum width which disallows
    // narrower options, or an indent threshold which causes the width
    // to suddenly increase.
    const lineWidth = this.lineWidth === -1
      ? -1
      : Math.max(Math.min(this.lineWidth, 40), this.lineWidth - indent);

    // Without knowing if keys are implicit/explicit,
    // assume implicit for safety.
    const singleLineOnly = isKey ||
      // No block styles in flow mode.
      (this.flowLevel > -1 && level >= this.flowLevel);

    const scalarStyle = chooseScalarStyle(
      string,
      singleLineOnly,
      this.indent,
      lineWidth,
      this.implicitTypes,
      this.quoteStyle,
    );
    switch (scalarStyle) {
      case STYLE_PLAIN:
        return string;
      case STYLE_SINGLE:
        return `'${string.replace(/'/g, "''")}'`;
      case STYLE_LITERAL:
        return `|${blockHeader(string, this.indent)}${
          trimTrailingNewline(indentString(string, indent))
        }`;
      case STYLE_FOLDED:
        return `>${blockHeader(string, this.indent)}${
          trimTrailingNewline(
            indentString(foldString(string, lineWidth), indent),
          )
        }`;
      case STYLE_DOUBLE:
        return `"${escapeString(string)}"`;
      default:
        throw new TypeError(
          "Invalid scalar style should be unreachable: please file a bug report against Deno at https://github.com/denoland/std/issues",
        );
    }
  }

  stringifyFlowSequence(
    array: unknown[],
    { level }: { level: number },
  ): string {
    const results = [];
    for (const value of array) {
      const string = this.stringifyNode(value, {
        level,
        block: false,
        compact: false,
        isKey: false,
      });
      if (string === null) continue;
      results.push(string);
    }
    const separator = this.condenseFlow ? "," : ", ";
    return `[${results.join(separator)}]`;
  }

  stringifyBlockSequence(
    array: unknown[],
    { level, compact }: { level: number; compact: boolean },
  ): string {
    const whitespace = generateNextLine(this.indent, level);
    const prefix = compact ? "" : whitespace;
    const results = [];
    for (const value of array) {
      const string = this.stringifyNode(value, {
        level: level + 1,
        block: true,
        compact: true,
        isKey: false,
      });
      if (string === null) continue;
      const linePrefix = LINE_FEED === string.charCodeAt(0) ? "-" : "- ";
      results.push(`${linePrefix}${string}`);
    }
    return results.length ? prefix + results.join(whitespace) : "[]";
  }

  stringifyFlowMapping(
    object: Record<string, unknown>,
    { level }: { level: number },
  ): string {
    const quote = this.condenseFlow ? '"' : "";
    const separator = this.condenseFlow ? ":" : ": ";

    const results = [];
    for (const [key, value] of Object.entries(object)) {
      const keyString = this.stringifyNode(key, {
        level,
        block: false,
        compact: false,
        isKey: false,
      });
      if (keyString === null) continue; // Skip this pair because of invalid key;

      const valueString = this.stringifyNode(value, {
        level,
        block: false,
        compact: false,
        isKey: false,
      });
      if (valueString === null) continue; // Skip this pair because of invalid value.

      const keyPrefix = keyString.length > 1024 ? "? " : "";
      results.push(
        quote + keyPrefix + keyString + quote + separator + valueString,
      );
    }

    return `{${results.join(", ")}}`;
  }

  stringifyBlockMapping(
    object: Record<string, unknown>,
    { tag, level, compact }: {
      tag: string | null;
      level: number;
      compact: boolean;
    },
  ): string {
    const keys = Object.keys(object);

    // Allow sorting keys so that the output file is deterministic
    if (this.sortKeys === true) {
      // Default sorting
      keys.sort();
    } else if (typeof this.sortKeys === "function") {
      // Custom sort function
      const sortKeys = this.sortKeys;
      keys.sort((a, b) => sortKeys(a, b, level));
    } else if (this.sortKeys) {
      // Something is wrong
      throw new TypeError(
        `"sortKeys" must be a boolean or a function: received ${typeof this
          .sortKeys}`,
      );
    }

    const separator = generateNextLine(this.indent, level);

    const results = [];

    for (const key of keys) {
      const value = object[key];

      const keyString = this.stringifyNode(key, {
        level: level + 1,
        block: true,
        compact: true,
        isKey: true,
      });
      if (keyString === null) continue; // Skip this pair because of invalid key.

      const explicitPair = (tag !== null && tag !== "?") ||
        (keyString.length > 1024);

      const valueString = this.stringifyNode(value, {
        level: level + 1,
        block: true,
        compact: explicitPair,
        isKey: false,
      });
      if (valueString === null) continue; // Skip this pair because of invalid value.

      let pairBuffer = "";
      if (explicitPair) {
        pairBuffer += keyString.charCodeAt(0) === LINE_FEED ? "?" : "? ";
      }
      pairBuffer += keyString;
      if (explicitPair) pairBuffer += separator;
      pairBuffer += valueString.charCodeAt(0) === LINE_FEED ? ":" : ": ";
      pairBuffer += valueString;
      results.push(pairBuffer);
    }

    const prefix = compact ? "" : separator;
    return results.length ? prefix + results.join(separator) : "{}"; // Empty mapping if no valid pairs.
  }

  getTypeRepresentation(type: Type<KindType, unknown>, value: unknown) {
    if (!type.represent) return value;
    const style = this.styleMap.get(type.tag) ??
      type.defaultStyle as StyleVariant;
    if (typeof type.represent === "function") {
      return type.represent(value, style);
    }
    const represent = type.represent[style];
    if (!represent) {
      throw new TypeError(
        `!<${type.tag}> tag resolver accepts not "${style}" style`,
      );
    }
    return represent(value, style);
  }

  detectType(value: unknown): { tag: string | null; value: unknown } {
    for (const type of this.implicitTypes) {
      if (type.predicate?.(value)) {
        value = this.getTypeRepresentation(type, value);
        return { tag: "?", value };
      }
    }
    for (const type of this.explicitTypes) {
      if (type.predicate?.(value)) {
        value = this.getTypeRepresentation(type, value);
        return { tag: type.tag, value };
      }
    }
    return { tag: null, value };
  }

  // Serializes `object` and writes it to global `result`.
  // Returns true on success, or false on invalid object.
  stringifyNode(value: unknown, { level, block, compact, isKey }: {
    level: number;
    block: boolean;
    compact: boolean;
    isKey: boolean;
  }): string | null {
    const result = this.detectType(value);
    const tag = result.tag;
    value = result.value;

    if (block) {
      block = this.flowLevel < 0 || this.flowLevel > level;
    }

    if (typeof value === "string" || value instanceof String) {
      value = value instanceof String ? value.valueOf() : value;
      if (tag !== "?") {
        value = this.stringifyScalar(value as string, { level, isKey });
      }
      return stringifyValue(value, tag);
    }

    if (isObject(value)) {
      const duplicateIndex = this.duplicates.indexOf(value);
      const duplicate = duplicateIndex !== -1;

      if (duplicate) {
        if (this.usedDuplicates.has(value)) return `*ref_${duplicateIndex}`;
        this.usedDuplicates.add(value);
      }

      if (
        (tag !== null && tag !== "?") ||
        duplicate ||
        (this.indent !== 2 && level > 0)
      ) {
        compact = false;
      }

      if (Array.isArray(value)) {
        const arrayLevel = !this.arrayIndent && level > 0 ? level - 1 : level;
        if (block && value.length !== 0) {
          value = this.stringifyBlockSequence(value, {
            level: arrayLevel,
            compact,
          });
          if (duplicate) value = `&ref_${duplicateIndex}${value}`;
          return stringifyValue(value, tag);
        }

        value = this.stringifyFlowSequence(value, { level: arrayLevel });
        if (duplicate) value = `&ref_${duplicateIndex} ${value}`;
        return stringifyValue(value, tag);
      }

      if (block && Object.keys(value).length !== 0) {
        value = this.stringifyBlockMapping(value, { tag, level, compact });
        if (duplicate) value = `&ref_${duplicateIndex}${value}`;
        return stringifyValue(value, tag);
      }

      value = this.stringifyFlowMapping(value, { level });
      if (duplicate) value = `&ref_${duplicateIndex} ${value}`;
      return stringifyValue(value, tag);
    }

    if (this.skipInvalid) return null;
    throw new TypeError(`Cannot stringify ${typeof value}`);
  }

  stringify(value: unknown): string {
    if (this.useAnchors) {
      this.duplicates = getDuplicateObjects(value);
      this.usedDuplicates = new Set();
    }

    const string = this.stringifyNode(value, {
      level: 0,
      block: true,
      compact: true,
      isKey: false,
    });
    if (string !== null) {
      return `${string}\n`;
    }
    return "";
  }
}
