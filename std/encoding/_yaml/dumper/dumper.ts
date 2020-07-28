// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable max-len */

import { YAMLError } from "../error.ts";
import type { RepresentFn, StyleVariant, Type } from "../type.ts";
import * as common from "../utils.ts";
import { DumperState, DumperStateOptions } from "./dumper_state.ts";

type Any = common.Any;
type ArrayObject<T = Any> = common.ArrayObject<T>;

const _toString = Object.prototype.toString;
const _hasOwnProperty = Object.prototype.hasOwnProperty;

const CHAR_TAB = 0x09; /* Tab */
const CHAR_LINE_FEED = 0x0a; /* LF */
const CHAR_SPACE = 0x20; /* Space */
const CHAR_EXCLAMATION = 0x21; /* ! */
const CHAR_DOUBLE_QUOTE = 0x22; /* " */
const CHAR_SHARP = 0x23; /* # */
const CHAR_PERCENT = 0x25; /* % */
const CHAR_AMPERSAND = 0x26; /* & */
const CHAR_SINGLE_QUOTE = 0x27; /* ' */
const CHAR_ASTERISK = 0x2a; /* * */
const CHAR_COMMA = 0x2c; /* , */
const CHAR_MINUS = 0x2d; /* - */
const CHAR_COLON = 0x3a; /* : */
const CHAR_GREATER_THAN = 0x3e; /* > */
const CHAR_QUESTION = 0x3f; /* ? */
const CHAR_COMMERCIAL_AT = 0x40; /* @ */
const CHAR_LEFT_SQUARE_BRACKET = 0x5b; /* [ */
const CHAR_RIGHT_SQUARE_BRACKET = 0x5d; /* ] */
const CHAR_GRAVE_ACCENT = 0x60; /* ` */
const CHAR_LEFT_CURLY_BRACKET = 0x7b; /* { */
const CHAR_VERTICAL_LINE = 0x7c; /* | */
const CHAR_RIGHT_CURLY_BRACKET = 0x7d; /* } */

const ESCAPE_SEQUENCES: { [char: number]: string } = {};

ESCAPE_SEQUENCES[0x00] = "\\0";
ESCAPE_SEQUENCES[0x07] = "\\a";
ESCAPE_SEQUENCES[0x08] = "\\b";
ESCAPE_SEQUENCES[0x09] = "\\t";
ESCAPE_SEQUENCES[0x0a] = "\\n";
ESCAPE_SEQUENCES[0x0b] = "\\v";
ESCAPE_SEQUENCES[0x0c] = "\\f";
ESCAPE_SEQUENCES[0x0d] = "\\r";
ESCAPE_SEQUENCES[0x1b] = "\\e";
ESCAPE_SEQUENCES[0x22] = '\\"';
ESCAPE_SEQUENCES[0x5c] = "\\\\";
ESCAPE_SEQUENCES[0x85] = "\\N";
ESCAPE_SEQUENCES[0xa0] = "\\_";
ESCAPE_SEQUENCES[0x2028] = "\\L";
ESCAPE_SEQUENCES[0x2029] = "\\P";

const DEPRECATED_BOOLEANS_SYNTAX = [
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
];

function encodeHex(character: number): string {
  const string = character.toString(16).toUpperCase();

  let handle: string;
  let length: number;
  if (character <= 0xff) {
    handle = "x";
    length = 2;
  } else if (character <= 0xffff) {
    handle = "u";
    length = 4;
  } else if (character <= 0xffffffff) {
    handle = "U";
    length = 8;
  } else {
    throw new YAMLError(
      "code point within a string may not be greater than 0xFFFFFFFF",
    );
  }

  return `\\${handle}${common.repeat("0", length - string.length)}${string}`;
}

// Indents every line in a string. Empty lines (\n only) are not indented.
function indentString(string: string, spaces: number): string {
  const ind = common.repeat(" ", spaces),
    length = string.length;
  let position = 0,
    next = -1,
    result = "",
    line: string;

  while (position < length) {
    next = string.indexOf("\n", position);
    if (next === -1) {
      line = string.slice(position);
      position = length;
    } else {
      line = string.slice(position, next + 1);
      position = next + 1;
    }

    if (line.length && line !== "\n") result += ind;

    result += line;
  }

  return result;
}

function generateNextLine(state: DumperState, level: number): string {
  return `\n${common.repeat(" ", state.indent * level)}`;
}

function testImplicitResolving(state: DumperState, str: string): boolean {
  let type: Type;
  for (
    let index = 0, length = state.implicitTypes.length;
    index < length;
    index += 1
  ) {
    type = state.implicitTypes[index];

    if (type.resolve(str)) {
      return true;
    }
  }

  return false;
}

// [33] s-white ::= s-space | s-tab
function isWhitespace(c: number): boolean {
  return c === CHAR_SPACE || c === CHAR_TAB;
}

// Returns true if the character can be printed without escaping.
// From YAML 1.2: "any allowed characters known to be non-printable
// should also be escaped. [However,] This isn’t mandatory"
// Derived from nb-char - \t - #x85 - #xA0 - #x2028 - #x2029.
function isPrintable(c: number): boolean {
  return (
    (0x00020 <= c && c <= 0x00007e) ||
    (0x000a1 <= c && c <= 0x00d7ff && c !== 0x2028 && c !== 0x2029) ||
    (0x0e000 <= c && c <= 0x00fffd && c !== 0xfeff) /* BOM */ ||
    (0x10000 <= c && c <= 0x10ffff)
  );
}

// Simplified test for values allowed after the first character in plain style.
function isPlainSafe(c: number): boolean {
  // Uses a subset of nb-char - c-flow-indicator - ":" - "#"
  // where nb-char ::= c-printable - b-char - c-byte-order-mark.
  return (
    isPrintable(c) &&
    c !== 0xfeff &&
    // - c-flow-indicator
    c !== CHAR_COMMA &&
    c !== CHAR_LEFT_SQUARE_BRACKET &&
    c !== CHAR_RIGHT_SQUARE_BRACKET &&
    c !== CHAR_LEFT_CURLY_BRACKET &&
    c !== CHAR_RIGHT_CURLY_BRACKET &&
    // - ":" - "#"
    c !== CHAR_COLON &&
    c !== CHAR_SHARP
  );
}

// Simplified test for values allowed as the first character in plain style.
function isPlainSafeFirst(c: number): boolean {
  // Uses a subset of ns-char - c-indicator
  // where ns-char = nb-char - s-white.
  return (
    isPrintable(c) &&
    c !== 0xfeff &&
    !isWhitespace(c) && // - s-white
    // - (c-indicator ::=
    // “-” | “?” | “:” | “,” | “[” | “]” | “{” | “}”
    c !== CHAR_MINUS &&
    c !== CHAR_QUESTION &&
    c !== CHAR_COLON &&
    c !== CHAR_COMMA &&
    c !== CHAR_LEFT_SQUARE_BRACKET &&
    c !== CHAR_RIGHT_SQUARE_BRACKET &&
    c !== CHAR_LEFT_CURLY_BRACKET &&
    c !== CHAR_RIGHT_CURLY_BRACKET &&
    // | “#” | “&” | “*” | “!” | “|” | “>” | “'” | “"”
    c !== CHAR_SHARP &&
    c !== CHAR_AMPERSAND &&
    c !== CHAR_ASTERISK &&
    c !== CHAR_EXCLAMATION &&
    c !== CHAR_VERTICAL_LINE &&
    c !== CHAR_GREATER_THAN &&
    c !== CHAR_SINGLE_QUOTE &&
    c !== CHAR_DOUBLE_QUOTE &&
    // | “%” | “@” | “`”)
    c !== CHAR_PERCENT &&
    c !== CHAR_COMMERCIAL_AT &&
    c !== CHAR_GRAVE_ACCENT
  );
}

// Determines whether block indentation indicator is required.
function needIndentIndicator(string: string): boolean {
  const leadingSpaceRe = /^\n* /;
  return leadingSpaceRe.test(string);
}

const STYLE_PLAIN = 1,
  STYLE_SINGLE = 2,
  STYLE_LITERAL = 3,
  STYLE_FOLDED = 4,
  STYLE_DOUBLE = 5;

// Determines which scalar styles are possible and returns the preferred style.
// lineWidth = -1 => no limit.
// Pre-conditions: str.length > 0.
// Post-conditions:
//  STYLE_PLAIN or STYLE_SINGLE => no \n are in the string.
//  STYLE_LITERAL => no lines are suitable for folding (or lineWidth is -1).
//  STYLE_FOLDED => a line > lineWidth and can be folded (and lineWidth != -1).
function chooseScalarStyle(
  string: string,
  singleLineOnly: boolean,
  indentPerLevel: number,
  lineWidth: number,
  testAmbiguousType: (...args: Any[]) => Any,
): number {
  const shouldTrackWidth = lineWidth !== -1;
  let hasLineBreak = false,
    hasFoldableLine = false, // only checked if shouldTrackWidth
    previousLineBreak = -1, // count the first line correctly
    plain = isPlainSafeFirst(string.charCodeAt(0)) &&
      !isWhitespace(string.charCodeAt(string.length - 1));

  let char: number, i: number;
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
      if (char === CHAR_LINE_FEED) {
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
    return plain && !testAmbiguousType(string) ? STYLE_PLAIN : STYLE_SINGLE;
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
  const breakRe = / [^ ]/g; // note: the match index will always be <= length-2.
  let match;
  // start is an inclusive index. end, curr, and next are exclusive.
  let start = 0,
    end,
    curr = 0,
    next = 0;
  let result = "";

  // Invariants: 0 <= start <= length-1.
  //   0 <= curr <= next <= max(0, length-2). curr - start <= width.
  // Inside the loop:
  //   A match implies length >= 2, so curr and next are <= length-2.
  // tslint:disable-next-line:no-conditional-assignment
  while ((match = breakRe.exec(line))) {
    next = match.index;
    // maintain invariant: curr - start <= width
    if (next - start > width) {
      end = curr > start ? curr : next; // derive end <= length-2
      result += `\n${line.slice(start, end)}`;
      // skip the space that was output as \n
      start = end + 1; // derive start <= length-1
    }
    curr = next;
  }

  // By the invariants, start <= length-1, so there is something left over.
  // It is either the whole string or a part starting from non-whitespace.
  result += "\n";
  // Insert a break if the remainder is too long and there is a break available.
  if (line.length - start > width && curr > start) {
    result += `${line.slice(start, curr)}\n${line.slice(curr + 1)}`;
  } else {
    result += line.slice(start);
  }

  return result.slice(1); // drop extra \n joiner
}

// (See the note for writeScalar.)
function dropEndingNewline(string: string): string {
  return string[string.length - 1] === "\n" ? string.slice(0, -1) : string;
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
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    return foldLine(string.slice(0, nextLF), width);
  })();
  // If we haven't reached the first content line yet, don't add an extra \n.
  let prevMoreIndented = string[0] === "\n" || string[0] === " ";
  let moreIndented;

  // rest of the lines
  let match;
  // tslint:disable-next-line:no-conditional-assignment
  while ((match = lineRe.exec(string))) {
    const prefix = match[1],
      line = match[2];
    moreIndented = line[0] === " ";
    result += prefix +
      (!prevMoreIndented && !moreIndented && line !== "" ? "\n" : "") +
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      foldLine(line, width);
    prevMoreIndented = moreIndented;
  }

  return result;
}

// Escapes a double-quoted string.
function escapeString(string: string): string {
  let result = "";
  let char, nextChar;
  let escapeSeq;

  for (let i = 0; i < string.length; i++) {
    char = string.charCodeAt(i);
    // Check for surrogate pairs (reference Unicode 3.0 section "3.7 Surrogates").
    if (char >= 0xd800 && char <= 0xdbff /* high surrogate */) {
      nextChar = string.charCodeAt(i + 1);
      if (nextChar >= 0xdc00 && nextChar <= 0xdfff /* low surrogate */) {
        // Combine the surrogate pair and store it escaped.
        result += encodeHex(
          (char - 0xd800) * 0x400 + nextChar - 0xdc00 + 0x10000,
        );
        // Advance index one extra since we already used that char here.
        i++;
        continue;
      }
    }
    escapeSeq = ESCAPE_SEQUENCES[char];
    result += !escapeSeq && isPrintable(char)
      ? string[i]
      : escapeSeq || encodeHex(char);
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

// Note: line breaking/folding is implemented for only the folded style.
// NB. We drop the last trailing newline (if any) of a returned block scalar
//  since the dumper adds its own newline. This always works:
//    • No ending newline => unaffected; already using strip "-" chomping.
//    • Ending newline    => removed then restored.
//  Importantly, this keeps the "+" chomp indicator from gaining an extra line.
function writeScalar(
  state: DumperState,
  string: string,
  level: number,
  iskey: boolean,
): void {
  state.dump = ((): string => {
    if (string.length === 0) {
      return "''";
    }
    if (
      !state.noCompatMode &&
      DEPRECATED_BOOLEANS_SYNTAX.indexOf(string) !== -1
    ) {
      return `'${string}'`;
    }

    const indent = state.indent * Math.max(1, level); // no 0-indent scalars
    // As indentation gets deeper, let the width decrease monotonically
    // to the lower bound min(state.lineWidth, 40).
    // Note that this implies
    //  state.lineWidth ≤ 40 + state.indent: width is fixed at the lower bound.
    //  state.lineWidth > 40 + state.indent: width decreases until the lower
    //  bound.
    // This behaves better than a constant minimum width which disallows
    // narrower options, or an indent threshold which causes the width
    // to suddenly increase.
    const lineWidth = state.lineWidth === -1
      ? -1
      : Math.max(Math.min(state.lineWidth, 40), state.lineWidth - indent);

    // Without knowing if keys are implicit/explicit,
    // assume implicit for safety.
    const singleLineOnly = iskey ||
      // No block styles in flow mode.
      (state.flowLevel > -1 && level >= state.flowLevel);
    function testAmbiguity(str: string): boolean {
      return testImplicitResolving(state, str);
    }

    switch (
      chooseScalarStyle(
        string,
        singleLineOnly,
        state.indent,
        lineWidth,
        testAmbiguity,
      )
    ) {
      case STYLE_PLAIN:
        return string;
      case STYLE_SINGLE:
        return `'${string.replace(/'/g, "''")}'`;
      case STYLE_LITERAL:
        return `|${blockHeader(string, state.indent)}${
          dropEndingNewline(indentString(string, indent))
        }`;
      case STYLE_FOLDED:
        return `>${blockHeader(string, state.indent)}${
          dropEndingNewline(
            indentString(foldString(string, lineWidth), indent),
          )
        }`;
      case STYLE_DOUBLE:
        return `"${escapeString(string)}"`;
      default:
        throw new YAMLError("impossible error: invalid scalar style");
    }
  })();
}

function writeFlowSequence(
  state: DumperState,
  level: number,
  object: Any,
): void {
  let _result = "";
  const _tag = state.tag;

  for (let index = 0, length = object.length; index < length; index += 1) {
    // Write only valid elements.
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (writeNode(state, level, object[index], false, false)) {
      if (index !== 0) _result += `,${!state.condenseFlow ? " " : ""}`;
      _result += state.dump;
    }
  }

  state.tag = _tag;
  state.dump = `[${_result}]`;
}

function writeBlockSequence(
  state: DumperState,
  level: number,
  object: Any,
  compact = false,
): void {
  let _result = "";
  const _tag = state.tag;

  for (let index = 0, length = object.length; index < length; index += 1) {
    // Write only valid elements.
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (writeNode(state, level + 1, object[index], true, true)) {
      if (!compact || index !== 0) {
        _result += generateNextLine(state, level);
      }

      if (state.dump && CHAR_LINE_FEED === state.dump.charCodeAt(0)) {
        _result += "-";
      } else {
        _result += "- ";
      }

      _result += state.dump;
    }
  }

  state.tag = _tag;
  state.dump = _result || "[]"; // Empty sequence if no valid values.
}

function writeFlowMapping(
  state: DumperState,
  level: number,
  object: Any,
): void {
  let _result = "";
  const _tag = state.tag,
    objectKeyList = Object.keys(object);

  let pairBuffer: string, objectKey: string, objectValue: Any;
  for (
    let index = 0, length = objectKeyList.length;
    index < length;
    index += 1
  ) {
    pairBuffer = state.condenseFlow ? '"' : "";

    if (index !== 0) pairBuffer += ", ";

    objectKey = objectKeyList[index];
    objectValue = object[objectKey];

    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (!writeNode(state, level, objectKey, false, false)) {
      continue; // Skip this pair because of invalid key;
    }

    if (state.dump.length > 1024) pairBuffer += "? ";

    pairBuffer += `${state.dump}${state.condenseFlow ? '"' : ""}:${
      state.condenseFlow ? "" : " "
    }`;

    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (!writeNode(state, level, objectValue, false, false)) {
      continue; // Skip this pair because of invalid value.
    }

    pairBuffer += state.dump;

    // Both key and value are valid.
    _result += pairBuffer;
  }

  state.tag = _tag;
  state.dump = `{${_result}}`;
}

function writeBlockMapping(
  state: DumperState,
  level: number,
  object: Any,
  compact = false,
): void {
  const _tag = state.tag,
    objectKeyList = Object.keys(object);
  let _result = "";

  // Allow sorting keys so that the output file is deterministic
  if (state.sortKeys === true) {
    // Default sorting
    objectKeyList.sort();
  } else if (typeof state.sortKeys === "function") {
    // Custom sort function
    objectKeyList.sort(state.sortKeys);
  } else if (state.sortKeys) {
    // Something is wrong
    throw new YAMLError("sortKeys must be a boolean or a function");
  }

  let pairBuffer = "",
    objectKey: string,
    objectValue: Any,
    explicitPair: boolean;
  for (
    let index = 0, length = objectKeyList.length;
    index < length;
    index += 1
  ) {
    pairBuffer = "";

    if (!compact || index !== 0) {
      pairBuffer += generateNextLine(state, level);
    }

    objectKey = objectKeyList[index];
    objectValue = object[objectKey];

    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (!writeNode(state, level + 1, objectKey, true, true, true)) {
      continue; // Skip this pair because of invalid key.
    }

    explicitPair = (state.tag !== null && state.tag !== "?") ||
      (state.dump && state.dump.length > 1024);

    if (explicitPair) {
      if (state.dump && CHAR_LINE_FEED === state.dump.charCodeAt(0)) {
        pairBuffer += "?";
      } else {
        pairBuffer += "? ";
      }
    }

    pairBuffer += state.dump;

    if (explicitPair) {
      pairBuffer += generateNextLine(state, level);
    }

    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    if (!writeNode(state, level + 1, objectValue, true, explicitPair)) {
      continue; // Skip this pair because of invalid value.
    }

    if (state.dump && CHAR_LINE_FEED === state.dump.charCodeAt(0)) {
      pairBuffer += ":";
    } else {
      pairBuffer += ": ";
    }

    pairBuffer += state.dump;

    // Both key and value are valid.
    _result += pairBuffer;
  }

  state.tag = _tag;
  state.dump = _result || "{}"; // Empty mapping if no valid pairs.
}

function detectType(
  state: DumperState,
  object: Any,
  explicit = false,
): boolean {
  const typeList = explicit ? state.explicitTypes : state.implicitTypes;

  let type: Type;
  let style: StyleVariant;
  let _result: string;
  for (let index = 0, length = typeList.length; index < length; index += 1) {
    type = typeList[index];

    if (
      (type.instanceOf || type.predicate) &&
      (!type.instanceOf ||
        (typeof object === "object" && object instanceof type.instanceOf)) &&
      (!type.predicate || type.predicate(object))
    ) {
      state.tag = explicit ? type.tag : "?";

      if (type.represent) {
        style = state.styleMap[type.tag] || type.defaultStyle;

        if (_toString.call(type.represent) === "[object Function]") {
          _result = (type.represent as RepresentFn)(object, style);
        } else if (_hasOwnProperty.call(type.represent, style)) {
          _result = (type.represent as ArrayObject<RepresentFn>)[style](
            object,
            style,
          );
        } else {
          throw new YAMLError(
            `!<${type.tag}> tag resolver accepts not "${style}" style`,
          );
        }

        state.dump = _result;
      }

      return true;
    }
  }

  return false;
}

// Serializes `object` and writes it to global `result`.
// Returns true on success, or false on invalid object.
//
function writeNode(
  state: DumperState,
  level: number,
  object: Any,
  block: boolean,
  compact: boolean,
  iskey = false,
): boolean {
  state.tag = null;
  state.dump = object;

  if (!detectType(state, object, false)) {
    detectType(state, object, true);
  }

  const type = _toString.call(state.dump);

  if (block) {
    block = state.flowLevel < 0 || state.flowLevel > level;
  }

  const objectOrArray = type === "[object Object]" || type === "[object Array]";

  let duplicateIndex = -1;
  let duplicate = false;
  if (objectOrArray) {
    duplicateIndex = state.duplicates.indexOf(object);
    duplicate = duplicateIndex !== -1;
  }

  if (
    (state.tag !== null && state.tag !== "?") ||
    duplicate ||
    (state.indent !== 2 && level > 0)
  ) {
    compact = false;
  }

  if (duplicate && state.usedDuplicates[duplicateIndex]) {
    state.dump = `*ref_${duplicateIndex}`;
  } else {
    if (objectOrArray && duplicate && !state.usedDuplicates[duplicateIndex]) {
      state.usedDuplicates[duplicateIndex] = true;
    }
    if (type === "[object Object]") {
      if (block && Object.keys(state.dump).length !== 0) {
        writeBlockMapping(state, level, state.dump, compact);
        if (duplicate) {
          state.dump = `&ref_${duplicateIndex}${state.dump}`;
        }
      } else {
        writeFlowMapping(state, level, state.dump);
        if (duplicate) {
          state.dump = `&ref_${duplicateIndex} ${state.dump}`;
        }
      }
    } else if (type === "[object Array]") {
      const arrayLevel = state.noArrayIndent && level > 0 ? level - 1 : level;
      if (block && state.dump.length !== 0) {
        writeBlockSequence(state, arrayLevel, state.dump, compact);
        if (duplicate) {
          state.dump = `&ref_${duplicateIndex}${state.dump}`;
        }
      } else {
        writeFlowSequence(state, arrayLevel, state.dump);
        if (duplicate) {
          state.dump = `&ref_${duplicateIndex} ${state.dump}`;
        }
      }
    } else if (type === "[object String]") {
      if (state.tag !== "?") {
        writeScalar(state, state.dump, level, iskey);
      }
    } else {
      if (state.skipInvalid) return false;
      throw new YAMLError(`unacceptable kind of an object to dump ${type}`);
    }

    if (state.tag !== null && state.tag !== "?") {
      state.dump = `!<${state.tag}> ${state.dump}`;
    }
  }

  return true;
}

function inspectNode(
  object: Any,
  objects: Any[],
  duplicatesIndexes: number[],
): void {
  if (object !== null && typeof object === "object") {
    const index = objects.indexOf(object);
    if (index !== -1) {
      if (duplicatesIndexes.indexOf(index) === -1) {
        duplicatesIndexes.push(index);
      }
    } else {
      objects.push(object);

      if (Array.isArray(object)) {
        for (let idx = 0, length = object.length; idx < length; idx += 1) {
          inspectNode(object[idx], objects, duplicatesIndexes);
        }
      } else {
        const objectKeyList = Object.keys(object);

        for (
          let idx = 0, length = objectKeyList.length;
          idx < length;
          idx += 1
        ) {
          inspectNode(object[objectKeyList[idx]], objects, duplicatesIndexes);
        }
      }
    }
  }
}

function getDuplicateReferences(object: object, state: DumperState): void {
  const objects: Any[] = [],
    duplicatesIndexes: number[] = [];

  inspectNode(object, objects, duplicatesIndexes);

  const length = duplicatesIndexes.length;
  for (let index = 0; index < length; index += 1) {
    state.duplicates.push(objects[duplicatesIndexes[index]]);
  }
  state.usedDuplicates = new Array(length);
}

export function dump(input: Any, options?: DumperStateOptions): string {
  options = options || {};

  const state = new DumperState(options);

  if (!state.noRefs) getDuplicateReferences(input, state);

  if (writeNode(state, 0, input, true, true)) return `${state.dump}\n`;

  return "";
}
