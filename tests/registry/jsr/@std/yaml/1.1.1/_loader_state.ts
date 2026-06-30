// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import {
  AMPERSAND,
  ASTERISK,
  BACKSLASH,
  CARRIAGE_RETURN,
  COLON,
  COMMA,
  COMMERCIAL_AT,
  DOT,
  DOUBLE_QUOTE,
  EXCLAMATION,
  GRAVE_ACCENT,
  GREATER_THAN,
  isEOL,
  isFlowIndicator,
  isWhiteSpace,
  isWhiteSpaceOrEOL,
  LEFT_CURLY_BRACKET,
  LEFT_SQUARE_BRACKET,
  LINE_FEED,
  MINUS,
  PERCENT,
  PLUS,
  QUESTION,
  RIGHT_CURLY_BRACKET,
  RIGHT_SQUARE_BRACKET,
  SHARP,
  SINGLE_QUOTE,
  SMALLER_THAN,
  SPACE,
  VERTICAL_LINE,
} from "./_chars.ts";

import { DEFAULT_SCHEMA, type Schema, type TypeMap } from "./_schema.ts";
import type { KindType, Type } from "./_type.ts";
import { isObject, isPlainObject } from "./_utils.ts";

const CONTEXT_FLOW_IN = 1;
const CONTEXT_FLOW_OUT = 2;
const CONTEXT_BLOCK_IN = 3;
const CONTEXT_BLOCK_OUT = 4;

const CHOMPING_CLIP = 1;
const CHOMPING_STRIP = 2;
const CHOMPING_KEEP = 3;

const PATTERN_NON_PRINTABLE_REGEXP =
  // deno-lint-ignore no-control-regex
  /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F-\x84\x86-\x9F\uFFFE\uFFFF]|[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?:[^\uD800-\uDBFF]|^)[\uDC00-\uDFFF]/;
const PATTERN_NON_ASCII_LINE_BREAKS_REGEXP = /[\x85\u2028\u2029]/;
const PATTERN_FLOW_INDICATORS_REGEXP = /[,\[\]\{\}]/;
const PATTERN_TAG_HANDLE_REGEXP = /^(?:!|!!|![a-z\-]+!)$/i;
const PATTERN_TAG_URI_REGEXP =
  /^(?:!|[^,\[\]\{\}])(?:%[0-9a-f]{2}|[0-9a-z\-#;\/\?:@&=\+\$,_\.!~\*'\(\)\[\]])*$/i;

export interface LoaderStateOptions {
  /** specifies a schema to use. */
  schema?: Schema;
  /** compatibility with JSON.parse behaviour. */
  allowDuplicateKeys?: boolean;
  /** function to call on warning messages. */
  onWarning?(error: SyntaxError): void;
}

const ESCAPED_HEX_LENGTHS = new Map<number, number>([
  [0x78, 2], // x
  [0x75, 4], // u
  [0x55, 8], // U
]);

const SIMPLE_ESCAPE_SEQUENCES = new Map<number, string>([
  [0x30, "\x00"], // 0
  [0x61, "\x07"], // a
  [0x62, "\x08"], // b
  [0x74, "\x09"], // t
  [0x09, "\x09"], // Tab
  [0x6e, "\x0A"], // n
  [0x76, "\x0B"], // v
  [0x66, "\x0C"], // f
  [0x72, "\x0D"], // r
  [0x65, "\x1B"], // e
  [0x20, " "], // Space
  [0x22, '"'], // "
  [0x2f, "/"], // /
  [0x5c, "\\"], // \
  [0x4e, "\x85"], // N
  [0x5f, "\xA0"], // _
  [0x4c, "\u2028"], // L
  [0x50, "\u2029"], // P
]);

/**
 * Converts a hexadecimal character code to its decimal value.
 */
function hexCharCodeToNumber(charCode: number) {
  // Check if the character code is in the range for '0' to '9'
  if (0x30 <= charCode && charCode <= 0x39) return charCode - 0x30; // Convert '0'-'9' to 0-9

  // Normalize the character code to lowercase if it's a letter
  const lc = charCode | 0x20;

  // Check if the character code is in the range for 'a' to 'f'
  if (0x61 <= lc && lc <= 0x66) return lc - 0x61 + 10; // Convert 'a'-'f' to 10-15

  return -1;
}

/**
 * Converts a decimal character code to its decimal value.
 */
function decimalCharCodeToNumber(charCode: number): number {
  // Check if the character code is in the range for '0' to '9'
  if (0x30 <= charCode && charCode <= 0x39) return charCode - 0x30; // Convert '0'-'9' to 0-9
  return -1;
}

/**
 * Converts a Unicode code point to a string.
 */
function codepointToChar(codepoint: number): string {
  // Check if the code point is within the Basic Multilingual Plane (BMP)
  if (codepoint <= 0xffff) return String.fromCharCode(codepoint); // Convert BMP code point to character

  // Encode UTF-16 surrogate pair for code points beyond BMP
  // Reference: https://en.wikipedia.org/wiki/UTF-16#Code_points_U.2B010000_to_U.2B10FFFF
  return String.fromCharCode(
    ((codepoint - 0x010000) >> 10) + 0xd800, // High surrogate
    ((codepoint - 0x010000) & 0x03ff) + 0xdc00, // Low surrogate
  );
}

const INDENT = 4;
const MAX_LENGTH = 75;
const DELIMITERS = "\x00\r\n\x85\u2028\u2029";

function getSnippet(buffer: string, position: number): string | null {
  if (!buffer) return null;
  let start = position;
  let end = position;
  let head = "";
  let tail = "";

  while (start > 0 && !DELIMITERS.includes(buffer.charAt(start - 1))) {
    start--;
    if (position - start > MAX_LENGTH / 2 - 1) {
      head = " ... ";
      start += 5;
      break;
    }
  }

  while (end < buffer.length && !DELIMITERS.includes(buffer.charAt(end))) {
    end++;
    if (end - position > MAX_LENGTH / 2 - 1) {
      tail = " ... ";
      end -= 5;
      break;
    }
  }

  const snippet = buffer.slice(start, end);
  const indent = " ".repeat(INDENT);
  const caretIndent = " ".repeat(INDENT + position - start + head.length);
  return `${indent + head + snippet + tail}\n${caretIndent}^`;
}

function markToString(
  buffer: string,
  position: number,
  line: number,
  column: number,
): string {
  let where = `at line ${line + 1}, column ${column + 1}`;
  const snippet = getSnippet(buffer, position);
  if (snippet) where += `:\n${snippet}`;
  return where;
}

function getIndentStatus(lineIndent: number, parentIndent: number) {
  if (lineIndent > parentIndent) return 1;
  if (lineIndent < parentIndent) return -1;
  return 0;
}

function writeFoldedLines(count: number) {
  if (count === 1) return " ";
  if (count > 1) return "\n".repeat(count - 1);
  return "";
}

class Scanner {
  source: string;
  #length: number;
  position = 0;

  constructor(source: string) {
    // Use 0 as string terminator. That significantly simplifies bounds check.
    source += "\0";
    this.source = source;
    this.#length = source.length;
  }

  peek(offset = 0) {
    return this.source.charCodeAt(this.position + offset);
  }

  next() {
    this.position += 1;
  }

  eof() {
    return this.position >= this.#length - 1;
  }
}

interface State {
  tag: string | null;
  anchor: string | null;
  kind: KindType | null;
  result: unknown[] | Record<string, unknown> | string | null;
}
export class LoaderState {
  #scanner: Scanner;
  lineIndent = 0;
  lineStart = 0;
  line = 0;
  onWarning: ((error: SyntaxError) => void) | undefined;
  allowDuplicateKeys: boolean;
  implicitTypes: Type<"scalar">[];
  typeMap: TypeMap;

  checkLineBreaks = false;
  tagMap = new Map();
  anchorMap = new Map();

  constructor(
    input: string,
    {
      schema = DEFAULT_SCHEMA,
      onWarning,
      allowDuplicateKeys = false,
    }: LoaderStateOptions,
  ) {
    this.#scanner = new Scanner(input);
    this.onWarning = onWarning;
    this.allowDuplicateKeys = allowDuplicateKeys;
    this.implicitTypes = schema.implicitTypes;
    this.typeMap = schema.typeMap;

    this.readIndent();
  }

  skipWhitespaces() {
    let ch = this.#scanner.peek();
    while (isWhiteSpace(ch)) {
      this.#scanner.next();
      ch = this.#scanner.peek();
    }
  }

  skipComment() {
    let ch = this.#scanner.peek();
    if (ch !== SHARP) return;
    this.#scanner.next();
    ch = this.#scanner.peek();
    while (ch !== 0 && !isEOL(ch)) {
      this.#scanner.next();
      ch = this.#scanner.peek();
    }
  }

  readIndent() {
    let ch = this.#scanner.peek();
    while (ch === SPACE) {
      this.lineIndent += 1;
      this.#scanner.next();
      ch = this.#scanner.peek();
    }
  }

  #createError(message: string): SyntaxError {
    const mark = markToString(
      this.#scanner.source,
      this.#scanner.position,
      this.line,
      this.#scanner.position - this.lineStart,
    );
    return new SyntaxError(`${message} ${mark}`);
  }

  dispatchWarning(message: string) {
    const error = this.#createError(message);
    this.onWarning?.(error);
  }

  yamlDirectiveHandler(args: string[]): string | null {
    if (args.length !== 1) {
      throw this.#createError(
        "Cannot handle YAML directive: YAML directive accepts exactly one argument",
      );
    }

    const match = /^([0-9]+)\.([0-9]+)$/.exec(args[0]!);
    if (match === null) {
      throw this.#createError(
        "Cannot handle YAML directive: ill-formed argument",
      );
    }

    const major = parseInt(match[1]!, 10);
    const minor = parseInt(match[2]!, 10);
    if (major !== 1) {
      throw this.#createError(
        "Cannot handle YAML directive: unacceptable YAML version",
      );
    }
    this.checkLineBreaks = minor < 2;
    if (minor !== 1 && minor !== 2) {
      this.dispatchWarning(
        "Cannot handle YAML directive: unsupported YAML version",
      );
    }
    return args[0] ?? null;
  }
  tagDirectiveHandler(args: string[]) {
    if (args.length !== 2) {
      throw this.#createError(
        `Cannot handle tag directive: directive accepts exactly two arguments, received ${args.length}`,
      );
    }

    const handle = args[0]!;
    const prefix = args[1]!;

    if (!PATTERN_TAG_HANDLE_REGEXP.test(handle)) {
      throw this.#createError(
        `Cannot handle tag directive: ill-formed handle (first argument) in "${handle}"`,
      );
    }

    if (this.tagMap.has(handle)) {
      throw this.#createError(
        `Cannot handle tag directive: previously declared suffix for "${handle}" tag handle`,
      );
    }

    if (!PATTERN_TAG_URI_REGEXP.test(prefix)) {
      throw this.#createError(
        "Cannot handle tag directive: ill-formed tag prefix (second argument) of the TAG directive",
      );
    }

    this.tagMap.set(handle, prefix);
  }
  captureSegment(start: number, end: number, checkJson: boolean) {
    if (start < end) {
      const result = this.#scanner.source.slice(start, end);

      if (checkJson) {
        for (let position = 0; position < result.length; position++) {
          const character = result.charCodeAt(position);
          if (
            !(character === 0x09 ||
              (0x20 <= character && character <= 0x10ffff))
          ) {
            throw this.#createError(
              `Expected valid JSON character: received "${character}"`,
            );
          }
        }
      } else if (PATTERN_NON_PRINTABLE_REGEXP.test(result)) {
        throw this.#createError("Stream contains non-printable characters");
      }

      return result;
    }
  }
  readBlockSequence(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
  ): State | void {
    let detected = false;

    const result: unknown[] = [];

    if (anchor !== null) this.anchorMap.set(anchor, result);

    let ch = this.#scanner.peek();

    while (ch !== 0) {
      if (ch !== MINUS) {
        break;
      }

      const following = this.#scanner.peek(1);

      if (!isWhiteSpaceOrEOL(following)) {
        break;
      }

      detected = true;
      this.#scanner.next();

      if (this.skipSeparationSpace(true, -1)) {
        if (this.lineIndent <= nodeIndent) {
          result.push(null);
          ch = this.#scanner.peek();
          continue;
        }
      }

      const line = this.line;
      const newState = this.composeNode({
        parentIndent: nodeIndent,
        nodeContext: CONTEXT_BLOCK_IN,
        allowToSeek: false,
        allowCompact: true,
      });
      if (newState) result.push(newState.result);
      this.skipSeparationSpace(true, -1);

      ch = this.#scanner.peek();

      if ((this.line === line || this.lineIndent > nodeIndent) && ch !== 0) {
        throw this.#createError(
          "Cannot read block sequence: bad indentation of a sequence entry",
        );
      } else if (this.lineIndent < nodeIndent) {
        break;
      }
    }

    if (detected) return { tag, anchor, kind: "sequence", result };
  }
  mergeMappings(
    destination: Record<string, unknown>,
    source: Record<string, unknown>,
    overridableKeys: Set<string>,
  ) {
    if (!isObject(source)) {
      throw this.#createError(
        "Cannot merge mappings: the provided source object is unacceptable",
      );
    }

    for (const [key, value] of Object.entries(source)) {
      if (Object.hasOwn(destination, key)) continue;
      // `Object.defineProperty` is significantly slower than direct
      // assignment in V8. Direct assignment produces an identical descriptor
      // (writable/enumerable/configurable) for ordinary keys; the only
      // sensitive case is `__proto__`, where direct assignment would mutate
      // the prototype chain instead of creating an own property.
      if (key === "__proto__") {
        Object.defineProperty(destination, key, {
          value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        destination[key] = value;
      }
      overridableKeys.add(key);
    }
  }
  storeMappingPair(
    result: Record<string, unknown>,
    overridableKeys: Set<string>,
    keyTag: string | null,
    keyNode: Record<PropertyKey, unknown> | unknown[] | string | null,
    valueNode: unknown,
    startLine?: number,
    startPos?: number,
  ): Record<string, unknown> {
    // The output is a plain object here, so keys can only be strings.
    // We need to convert keyNode to a string, but doing so can hang the process
    // (deeply nested arrays that explode exponentially using aliases).
    if (Array.isArray(keyNode)) {
      keyNode = Array.prototype.slice.call(keyNode);

      for (let index = 0; index < keyNode.length; index++) {
        if (Array.isArray(keyNode[index])) {
          throw this.#createError(
            "Cannot store mapping pair: nested arrays are not supported inside keys",
          );
        }

        if (typeof keyNode === "object" && isPlainObject(keyNode[index])) {
          keyNode[index] = "[object Object]";
        }
      }
    }

    // Avoid code execution in load() via toString property
    // (still use its own toString for arrays, timestamps,
    // and whatever user schema extensions happen to have @@toStringTag)
    if (typeof keyNode === "object" && isPlainObject(keyNode)) {
      keyNode = "[object Object]";
    }

    keyNode = String(keyNode);

    if (keyTag === "tag:yaml.org,2002:merge") {
      if (Array.isArray(valueNode)) {
        for (
          let index = 0;
          index < valueNode.length;
          index++
        ) {
          this.mergeMappings(result, valueNode[index], overridableKeys);
        }
      } else {
        this.mergeMappings(
          result,
          valueNode as Record<string, unknown>,
          overridableKeys,
        );
      }
    } else {
      if (
        !this.allowDuplicateKeys &&
        !overridableKeys.has(keyNode) &&
        Object.hasOwn(result, keyNode)
      ) {
        this.line = startLine || this.line;
        this.#scanner.position = startPos || this.#scanner.position;
        throw this.#createError("Cannot store mapping pair: duplicated key");
      }
      // See `mergeMappings` above for why `Object.defineProperty` is kept
      // only for the `__proto__` key.
      if (keyNode === "__proto__") {
        Object.defineProperty(result, keyNode, {
          value: valueNode,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        result[keyNode] = valueNode;
      }
      overridableKeys.delete(keyNode);
    }

    return result;
  }
  readLineBreak() {
    const ch = this.#scanner.peek();

    if (ch === LINE_FEED) {
      this.#scanner.next();
    } else if (ch === CARRIAGE_RETURN) {
      this.#scanner.next();
      if (this.#scanner.peek() === LINE_FEED) {
        this.#scanner.next();
      }
    } else {
      throw this.#createError("Cannot read line: line break not found");
    }

    this.line += 1;
    this.lineStart = this.#scanner.position;
  }
  skipSeparationSpace(allowComments: boolean, checkIndent: number): number {
    let lineBreaks = 0;
    let ch = this.#scanner.peek();

    while (ch !== 0) {
      this.skipWhitespaces();
      ch = this.#scanner.peek();

      if (allowComments) {
        this.skipComment();
        ch = this.#scanner.peek();
      }

      if (isEOL(ch)) {
        this.readLineBreak();

        ch = this.#scanner.peek();
        lineBreaks++;
        this.lineIndent = 0;

        this.readIndent();
        ch = this.#scanner.peek();
      } else {
        break;
      }
    }

    if (
      checkIndent !== -1 &&
      lineBreaks !== 0 &&
      this.lineIndent < checkIndent
    ) {
      this.dispatchWarning("deficient indentation");
    }

    return lineBreaks;
  }
  testDocumentSeparator(): boolean {
    let ch = this.#scanner.peek();

    // Condition this.#scanner.position === this.lineStart is tested
    // in parent on each call, for efficiency. No needs to test here again.
    if (
      (ch === MINUS || ch === DOT) &&
      ch === this.#scanner.peek(1) &&
      ch === this.#scanner.peek(2)
    ) {
      ch = this.#scanner.peek(3);

      if (ch === 0 || isWhiteSpaceOrEOL(ch)) {
        return true;
      }
    }

    return false;
  }

  readPlainScalar(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
    withinFlowCollection: boolean,
  ): State | void {
    let ch = this.#scanner.peek();

    if (
      isWhiteSpaceOrEOL(ch) ||
      isFlowIndicator(ch) ||
      ch === SHARP ||
      ch === AMPERSAND ||
      ch === ASTERISK ||
      ch === EXCLAMATION ||
      ch === VERTICAL_LINE ||
      ch === GREATER_THAN ||
      ch === SINGLE_QUOTE ||
      ch === DOUBLE_QUOTE ||
      ch === PERCENT ||
      ch === COMMERCIAL_AT ||
      ch === GRAVE_ACCENT
    ) {
      return;
    }

    let following: number;
    if (ch === QUESTION || ch === MINUS) {
      following = this.#scanner.peek(1);

      if (
        isWhiteSpaceOrEOL(following) ||
        (withinFlowCollection && isFlowIndicator(following))
      ) {
        return;
      }
    }

    let result = "";

    let captureEnd = this.#scanner.position;
    let captureStart = this.#scanner.position;
    let hasPendingContent = false;
    let line = 0;
    while (ch !== 0) {
      if (ch === COLON) {
        following = this.#scanner.peek(1);

        if (
          isWhiteSpaceOrEOL(following) ||
          (withinFlowCollection && isFlowIndicator(following))
        ) {
          break;
        }
      } else if (ch === SHARP) {
        const preceding = this.#scanner.peek(-1);

        if (isWhiteSpaceOrEOL(preceding)) {
          break;
        }
      } else if (
        (this.#scanner.position === this.lineStart &&
          this.testDocumentSeparator()) ||
        (withinFlowCollection && isFlowIndicator(ch))
      ) {
        break;
      } else if (isEOL(ch)) {
        line = this.line;
        const lineStart = this.lineStart;
        const lineIndent = this.lineIndent;
        this.skipSeparationSpace(false, -1);

        if (this.lineIndent >= nodeIndent) {
          hasPendingContent = true;
          ch = this.#scanner.peek();
          continue;
        } else {
          this.#scanner.position = captureEnd;
          this.line = line;
          this.lineStart = lineStart;
          this.lineIndent = lineIndent;
          break;
        }
      }

      if (hasPendingContent) {
        const segment = this.captureSegment(captureStart, captureEnd, false);
        if (segment) result += segment;
        result += writeFoldedLines(this.line - line);
        captureStart = captureEnd = this.#scanner.position;
        hasPendingContent = false;
      }

      if (!isWhiteSpace(ch)) {
        captureEnd = this.#scanner.position + 1;
      }

      this.#scanner.next();
      ch = this.#scanner.peek();
    }

    const segment = this.captureSegment(captureStart, captureEnd, false);
    if (segment) result += segment;
    if (anchor !== null) this.anchorMap.set(anchor, result);
    if (result) return { tag, anchor, kind: "scalar", result };
  }
  readSingleQuotedScalar(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
  ): State | void {
    let ch = this.#scanner.peek();

    if (ch !== SINGLE_QUOTE) return;

    let result = "";
    this.#scanner.next();
    let captureStart = this.#scanner.position;
    let captureEnd = this.#scanner.position;

    ch = this.#scanner.peek();
    while (ch !== 0) {
      if (ch === SINGLE_QUOTE) {
        const segment = this.captureSegment(
          captureStart,
          this.#scanner.position,
          true,
        );
        if (segment) result += segment;
        this.#scanner.next();
        ch = this.#scanner.peek();

        if (ch === SINGLE_QUOTE) {
          captureStart = this.#scanner.position;
          this.#scanner.next();
          captureEnd = this.#scanner.position;
        } else {
          if (anchor !== null) this.anchorMap.set(anchor, result);
          return { tag, anchor, kind: "scalar", result };
        }
      } else if (isEOL(ch)) {
        const segment = this.captureSegment(captureStart, captureEnd, true);
        if (segment) result += segment;
        result += writeFoldedLines(
          this.skipSeparationSpace(false, nodeIndent),
        );
        captureStart = captureEnd = this.#scanner.position;
      } else if (
        this.#scanner.position === this.lineStart &&
        this.testDocumentSeparator()
      ) {
        throw this.#createError(
          "Unexpected end of the document within a single quoted scalar",
        );
      } else {
        this.#scanner.next();
        captureEnd = this.#scanner.position;
      }
      ch = this.#scanner.peek();
    }

    throw this.#createError(
      "Unexpected end of the stream within a single quoted scalar",
    );
  }
  readDoubleQuotedScalar(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
  ): State | void {
    let ch = this.#scanner.peek();

    if (ch !== DOUBLE_QUOTE) return;

    let result = "";
    this.#scanner.next();
    let captureEnd = this.#scanner.position;
    let captureStart = this.#scanner.position;
    let tmp: number;
    ch = this.#scanner.peek();
    while (ch !== 0) {
      if (ch === DOUBLE_QUOTE) {
        const segment = this.captureSegment(
          captureStart,
          this.#scanner.position,
          true,
        );
        if (segment) result += segment;
        this.#scanner.next();
        if (anchor !== null) this.anchorMap.set(anchor, result);
        return { tag, anchor, kind: "scalar", result };
      }
      if (ch === BACKSLASH) {
        const segment = this.captureSegment(
          captureStart,
          this.#scanner.position,
          true,
        );
        if (segment) result += segment;
        this.#scanner.next();
        ch = this.#scanner.peek();

        if (isEOL(ch)) {
          this.skipSeparationSpace(false, nodeIndent);
        } else if (ch < 256 && SIMPLE_ESCAPE_SEQUENCES.has(ch)) {
          result += SIMPLE_ESCAPE_SEQUENCES.get(ch);
          this.#scanner.next();
        } else if ((tmp = ESCAPED_HEX_LENGTHS.get(ch) ?? 0) > 0) {
          let hexLength = tmp;
          let hexResult = 0;

          for (; hexLength > 0; hexLength--) {
            this.#scanner.next();
            ch = this.#scanner.peek();

            if ((tmp = hexCharCodeToNumber(ch)) >= 0) {
              hexResult = (hexResult << 4) + tmp;
            } else {
              throw this.#createError(
                "Cannot read double quoted scalar: expected hexadecimal character",
              );
            }
          }

          result += codepointToChar(hexResult);

          this.#scanner.next();
        } else {
          throw this.#createError(
            "Cannot read double quoted scalar: unknown escape sequence",
          );
        }

        captureStart = captureEnd = this.#scanner.position;
      } else if (isEOL(ch)) {
        const segment = this.captureSegment(captureStart, captureEnd, true);
        if (segment) result += segment;
        result += writeFoldedLines(
          this.skipSeparationSpace(false, nodeIndent),
        );
        captureStart = captureEnd = this.#scanner.position;
      } else if (
        this.#scanner.position === this.lineStart &&
        this.testDocumentSeparator()
      ) {
        throw this.#createError(
          "Unexpected end of the document within a double quoted scalar",
        );
      } else {
        this.#scanner.next();
        captureEnd = this.#scanner.position;
      }
      ch = this.#scanner.peek();
    }

    throw this.#createError(
      "Unexpected end of the stream within a double quoted scalar",
    );
  }
  readFlowCollection(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
  ): State | void {
    let ch = this.#scanner.peek();
    let terminator: number;
    let isMapping = true;
    let result = {};
    if (ch === LEFT_SQUARE_BRACKET) {
      terminator = RIGHT_SQUARE_BRACKET;
      isMapping = false;
      result = [];
    } else if (ch === LEFT_CURLY_BRACKET) {
      terminator = RIGHT_CURLY_BRACKET;
    } else {
      return;
    }

    if (anchor !== null) this.anchorMap.set(anchor, result);

    this.#scanner.next();
    ch = this.#scanner.peek();

    let readNext = true;
    let valueNode = null;
    let keyNode = null;
    let keyTag: string | null = null;
    let isExplicitPair = false;
    let isPair = false;
    let following = 0;
    let line = 0;
    const overridableKeys = new Set<string>();
    while (ch !== 0) {
      this.skipSeparationSpace(true, nodeIndent);

      ch = this.#scanner.peek();

      if (ch === terminator) {
        this.#scanner.next();
        const kind = isMapping ? "mapping" : "sequence";
        return { tag, anchor, kind, result };
      }
      if (!readNext) {
        throw this.#createError(
          "Cannot read flow collection: missing comma between flow collection entries",
        );
      }

      keyTag = keyNode = valueNode = null;
      isPair = isExplicitPair = false;

      if (ch === QUESTION) {
        following = this.#scanner.peek(1);

        if (isWhiteSpaceOrEOL(following)) {
          isPair = isExplicitPair = true;
          this.#scanner.next();
          this.skipSeparationSpace(true, nodeIndent);
        }
      }

      line = this.line;
      const newState = this.composeNode({
        parentIndent: nodeIndent,
        nodeContext: CONTEXT_FLOW_IN,
        allowToSeek: false,
        allowCompact: true,
      });
      if (newState) {
        keyTag = newState.tag || null;
        keyNode = newState.result;
      }
      this.skipSeparationSpace(true, nodeIndent);

      ch = this.#scanner.peek();

      if ((isExplicitPair || this.line === line) && ch === COLON) {
        isPair = true;
        this.#scanner.next();
        ch = this.#scanner.peek();
        this.skipSeparationSpace(true, nodeIndent);
        const newState = this.composeNode({
          parentIndent: nodeIndent,
          nodeContext: CONTEXT_FLOW_IN,
          allowToSeek: false,
          allowCompact: true,
        });
        if (newState) valueNode = newState.result;
      }

      if (isMapping) {
        this.storeMappingPair(
          result as Record<string, unknown>,
          overridableKeys,
          keyTag,
          keyNode,
          valueNode,
        );
      } else if (isPair) {
        (result as Record<string, unknown>[]).push(
          this.storeMappingPair(
            {},
            overridableKeys,
            keyTag,
            keyNode,
            valueNode,
          ),
        );
      } else {
        (result as unknown[]).push(keyNode);
      }

      this.skipSeparationSpace(true, nodeIndent);

      ch = this.#scanner.peek();

      if (ch === COMMA) {
        readNext = true;
        this.#scanner.next();
        ch = this.#scanner.peek();
      } else {
        readNext = false;
      }
    }

    throw this.#createError(
      "Cannot read flow collection: unexpected end of the stream within a flow collection",
    );
  }
  // Handles block scaler styles: e.g. '|', '>', '|-' and '>-'.
  // https://yaml.org/spec/1.2.2/#81-block-scalar-styles
  readBlockScalar(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
  ): State | void {
    let chomping = CHOMPING_CLIP;
    let didReadContent = false;
    let detectedIndent = false;
    let textIndent = nodeIndent;
    let emptyLines = 0;
    let atMoreIndented = false;

    let ch = this.#scanner.peek();

    let folding = false;
    if (ch === VERTICAL_LINE) {
      folding = false;
    } else if (ch === GREATER_THAN) {
      folding = true;
    } else {
      return;
    }

    let result = "";

    let tmp = 0;
    while (ch !== 0) {
      this.#scanner.next();
      ch = this.#scanner.peek();

      if (ch === PLUS || ch === MINUS) {
        if (CHOMPING_CLIP === chomping) {
          chomping = ch === PLUS ? CHOMPING_KEEP : CHOMPING_STRIP;
        } else {
          throw this.#createError(
            "Cannot read block: chomping mode identifier repeated",
          );
        }
      } else if ((tmp = decimalCharCodeToNumber(ch)) >= 0) {
        if (tmp === 0) {
          throw this.#createError(
            "Cannot read block: indentation width must be greater than 0",
          );
        } else if (!detectedIndent) {
          textIndent = nodeIndent + tmp - 1;
          detectedIndent = true;
        } else {
          throw this.#createError(
            "Cannot read block: indentation width identifier repeated",
          );
        }
      } else {
        break;
      }
    }

    if (isWhiteSpace(ch)) {
      this.skipWhitespaces();
      this.skipComment();
      ch = this.#scanner.peek();
    }

    while (ch !== 0) {
      this.readLineBreak();
      this.lineIndent = 0;

      ch = this.#scanner.peek();

      while (
        (!detectedIndent || this.lineIndent < textIndent) &&
        ch === SPACE
      ) {
        this.lineIndent++;
        this.#scanner.next();
        ch = this.#scanner.peek();
      }

      if (!detectedIndent && this.lineIndent > textIndent) {
        textIndent = this.lineIndent;
      }

      if (isEOL(ch)) {
        emptyLines++;
        continue;
      }

      // End of the scalar.
      if (this.lineIndent < textIndent) {
        // Perform the chomping.
        if (chomping === CHOMPING_KEEP) {
          result += "\n".repeat(
            didReadContent ? 1 + emptyLines : emptyLines,
          );
        } else if (chomping === CHOMPING_CLIP) {
          if (didReadContent) {
            // i.e. only if the scalar is not empty.
            result += "\n";
          }
        }

        // Break this `while` cycle and go to the function's epilogue.
        break;
      }

      // Folded style: use fancy rules to handle line breaks.
      if (folding) {
        // Lines starting with white space characters (more-indented lines) are not folded.
        if (isWhiteSpace(ch)) {
          atMoreIndented = true;
          // except for the first content line (cf. Example 8.1)
          result += "\n".repeat(
            didReadContent ? 1 + emptyLines : emptyLines,
          );

          // End of more-indented block.
        } else if (atMoreIndented) {
          atMoreIndented = false;
          result += "\n".repeat(emptyLines + 1);

          // Just one line break - perceive as the same line.
        } else if (emptyLines === 0) {
          if (didReadContent) {
            // i.e. only if we have already read some scalar content.
            result += " ";
          }

          // Several line breaks - perceive as different lines.
        } else {
          result += "\n".repeat(emptyLines);
        }

        // Literal style: just add exact number of line breaks between content lines.
      } else {
        // Keep all line breaks except the header line break.
        result += "\n".repeat(
          didReadContent ? 1 + emptyLines : emptyLines,
        );
      }

      didReadContent = true;
      detectedIndent = true;
      emptyLines = 0;
      const captureStart = this.#scanner.position;

      while (!isEOL(ch) && ch !== 0) {
        this.#scanner.next();
        ch = this.#scanner.peek();
      }

      const segment = this.captureSegment(
        captureStart,
        this.#scanner.position,
        false,
      );
      if (segment) result += segment;
    }

    if (anchor !== null) this.anchorMap.set(anchor, result);
    return { tag, anchor, kind: "scalar", result };
  }
  readBlockMapping(
    tag: string | null,
    anchor: string | null,
    nodeIndent: number,
    flowIndent: number,
  ): State | void {
    const result = {};
    const overridableKeys = new Set<string>();

    let allowCompact = false;
    let line: number;
    let pos: number;
    let keyTag = null;
    let keyNode = null;
    let valueNode = null;
    let atExplicitKey = false;
    let detected = false;

    if (anchor !== null) this.anchorMap.set(anchor, result);

    let ch = this.#scanner.peek();

    while (ch !== 0) {
      const following = this.#scanner.peek(1);
      line = this.line; // Save the current line.
      pos = this.#scanner.position;

      //
      // Explicit notation case. There are two separate blocks:
      // first for the key (denoted by "?") and second for the value (denoted by ":")
      //
      if ((ch === QUESTION || ch === COLON) && isWhiteSpaceOrEOL(following)) {
        if (ch === QUESTION) {
          if (atExplicitKey) {
            this.storeMappingPair(
              result,
              overridableKeys,
              keyTag as string,
              keyNode,
              null,
            );
            keyTag = null;
            keyNode = null;
            valueNode = null;
          }

          detected = true;
          atExplicitKey = true;
          allowCompact = true;
        } else if (atExplicitKey) {
          // i.e. 0x3A/* : */ === character after the explicit key.
          atExplicitKey = false;
          allowCompact = true;
        } else {
          throw this.#createError(
            "Cannot read block as explicit mapping pair is incomplete: a key node is missed or followed by a non-tabulated empty line",
          );
        }

        this.#scanner.next();
        ch = following;

        //
        // Implicit notation case. Flow-style node as the key first, then ":", and the value.
        //
      } else {
        const newState = this.composeNode({
          parentIndent: flowIndent,
          nodeContext: CONTEXT_FLOW_OUT,
          allowToSeek: false,
          allowCompact: true,
        });
        if (!newState) break; // Reading is done. Go to the epilogue.
        if (this.line === line) {
          ch = this.#scanner.peek();

          this.skipWhitespaces();
          ch = this.#scanner.peek();

          if (ch === COLON) {
            this.#scanner.next();
            ch = this.#scanner.peek();

            if (!isWhiteSpaceOrEOL(ch)) {
              throw this.#createError(
                "Cannot read block: a whitespace character is expected after the key-value separator within a block mapping",
              );
            }

            if (atExplicitKey) {
              this.storeMappingPair(
                result,
                overridableKeys,
                keyTag as string,
                keyNode,
                null,
              );
              keyTag = null;
              keyNode = null;
              valueNode = null;
            }

            detected = true;
            atExplicitKey = false;
            allowCompact = false;
            keyTag = newState.tag;
            keyNode = newState.result;
          } else if (detected) {
            throw this.#createError(
              "Cannot read an implicit mapping pair: missing colon",
            );
          } else {
            const { kind, result } = newState;
            return { tag, anchor, kind, result }; // Keep the result of `composeNode`.
          }
        } else if (detected) {
          throw this.#createError(
            "Cannot read a block mapping entry: a multiline key may not be an implicit key",
          );
        } else {
          const { kind, result } = newState;
          return { tag, anchor, kind, result }; // Keep the result of `composeNode`.
        }
      }

      //
      // Common reading code for both explicit and implicit notations.
      //
      if (this.line === line || this.lineIndent > nodeIndent) {
        const newState = this.composeNode({
          parentIndent: nodeIndent,
          nodeContext: CONTEXT_BLOCK_OUT,
          allowToSeek: true,
          allowCompact,
        });
        if (newState) {
          if (atExplicitKey) {
            keyNode = newState.result;
          } else {
            valueNode = newState.result;
          }
        }

        if (!atExplicitKey) {
          this.storeMappingPair(
            result,
            overridableKeys,
            keyTag as string,
            keyNode,
            valueNode,
            line,
            pos,
          );
          keyTag = keyNode = valueNode = null;
        }

        this.skipSeparationSpace(true, -1);
        ch = this.#scanner.peek();
      }

      if (this.lineIndent > nodeIndent && ch !== 0) {
        throw this.#createError(
          "Cannot read block: bad indentation of a mapping entry",
        );
      } else if (this.lineIndent < nodeIndent) {
        break;
      }
    }

    //
    // Epilogue.
    //

    // Special case: last mapping's node contains only the key in explicit notation.
    if (atExplicitKey) {
      this.storeMappingPair(
        result,
        overridableKeys,
        keyTag as string,
        keyNode,
        null,
      );
    }

    // Expose the resulting mapping.
    if (detected) return { tag, anchor, kind: "mapping", result };
  }
  readTagProperty(tag: string | null): string | void {
    let isVerbatim = false;
    let isNamed = false;
    let tagHandle = "";
    let tagName: string;

    let ch = this.#scanner.peek();

    if (ch !== EXCLAMATION) return;

    if (tag !== null) {
      throw this.#createError(
        "Cannot read tag property: duplication of a tag property",
      );
    }

    this.#scanner.next();
    ch = this.#scanner.peek();

    if (ch === SMALLER_THAN) {
      isVerbatim = true;
      this.#scanner.next();
      ch = this.#scanner.peek();
    } else if (ch === EXCLAMATION) {
      isNamed = true;
      tagHandle = "!!";
      this.#scanner.next();
      ch = this.#scanner.peek();
    } else {
      tagHandle = "!";
    }

    let position = this.#scanner.position;

    if (isVerbatim) {
      do {
        this.#scanner.next();
        ch = this.#scanner.peek();
      } while (ch !== 0 && ch !== GREATER_THAN);

      if (!this.#scanner.eof()) {
        tagName = this.#scanner.source.slice(position, this.#scanner.position);
        this.#scanner.next();
        ch = this.#scanner.peek();
      } else {
        throw this.#createError(
          "Cannot read tag property: unexpected end of stream",
        );
      }
    } else {
      while (ch !== 0 && !isWhiteSpaceOrEOL(ch)) {
        if (ch === EXCLAMATION) {
          if (!isNamed) {
            tagHandle = this.#scanner.source.slice(
              position - 1,
              this.#scanner.position + 1,
            );

            if (!PATTERN_TAG_HANDLE_REGEXP.test(tagHandle)) {
              throw this.#createError(
                "Cannot read tag property: named tag handle contains invalid characters",
              );
            }

            isNamed = true;
            position = this.#scanner.position + 1;
          } else {
            throw this.#createError(
              "Cannot read tag property: tag suffix cannot contain an exclamation mark",
            );
          }
        }

        this.#scanner.next();
        ch = this.#scanner.peek();
      }

      tagName = this.#scanner.source.slice(position, this.#scanner.position);

      if (PATTERN_FLOW_INDICATORS_REGEXP.test(tagName)) {
        throw this.#createError(
          "Cannot read tag property: tag suffix cannot contain flow indicator characters",
        );
      }
    }

    if (tagName && !PATTERN_TAG_URI_REGEXP.test(tagName)) {
      throw this.#createError(
        `Cannot read tag property: invalid characters in tag name "${tagName}"`,
      );
    }

    if (isVerbatim) {
      return tagName;
    } else if (this.tagMap.has(tagHandle)) {
      return this.tagMap.get(tagHandle) + tagName;
    } else if (tagHandle === "!") {
      return `!${tagName}`;
    } else if (tagHandle === "!!") {
      return `tag:yaml.org,2002:${tagName}`;
    }

    throw this.#createError(
      `Cannot read tag property: undeclared tag handle "${tagHandle}"`,
    );
  }
  readAnchorProperty(anchor: string | null): string | void {
    let ch = this.#scanner.peek();
    if (ch !== AMPERSAND) return;

    if (anchor !== null) {
      throw this.#createError(
        "Cannot read anchor property: duplicate anchor property",
      );
    }
    this.#scanner.next();
    ch = this.#scanner.peek();

    const position = this.#scanner.position;
    while (ch !== 0 && !isWhiteSpaceOrEOL(ch) && !isFlowIndicator(ch)) {
      this.#scanner.next();
      ch = this.#scanner.peek();
    }

    if (this.#scanner.position === position) {
      throw this.#createError(
        "Cannot read anchor property: name of an anchor node must contain at least one character",
      );
    }

    return this.#scanner.source.slice(position, this.#scanner.position);
  }
  readAlias(): string | void {
    if (this.#scanner.peek() !== ASTERISK) return;

    this.#scanner.next();
    let ch = this.#scanner.peek();

    const position = this.#scanner.position;

    while (ch !== 0 && !isWhiteSpaceOrEOL(ch) && !isFlowIndicator(ch)) {
      this.#scanner.next();
      ch = this.#scanner.peek();
    }

    if (this.#scanner.position === position) {
      throw this.#createError(
        "Cannot read alias: alias name must contain at least one character",
      );
    }

    const alias = this.#scanner.source.slice(position, this.#scanner.position);
    if (!this.anchorMap.has(alias)) {
      throw this.#createError(
        `Cannot read alias: unidentified alias "${alias}"`,
      );
    }

    this.skipSeparationSpace(true, -1);

    return this.anchorMap.get(alias);
  }
  resolveTag(state: State) {
    switch (state.tag) {
      case null:
      case "!":
        return state;
      case "?": {
        for (const type of this.implicitTypes) {
          // Implicit resolving is not allowed for non-scalar types, and '?'
          // non-specific tag is only assigned to plain scalars. So, it isn't
          // needed to check for 'kind' conformity.

          if (!type.resolve(state.result)) continue;
          // `state.result` updated in resolver if matched
          const result = type.construct(state.result);
          state.result = result;
          state.tag = type.tag;
          const { anchor } = state;
          if (anchor !== null) this.anchorMap.set(anchor, result);
          return state;
        }
        return state;
      }
    }

    const kind = state.kind ?? "fallback";

    const map = this.typeMap[kind];
    const type = map.get(state.tag);

    if (!type) {
      throw this.#createError(
        `Cannot resolve unknown tag !<${state.tag}>`,
      );
    }

    if (state.result !== null && type.kind !== state.kind) {
      throw this.#createError(
        `Unacceptable node kind for !<${state.tag}> tag: it should be "${type.kind}", not "${state.kind}"`,
      );
    }

    if (!type.resolve(state.result)) {
      // `state.result` updated in resolver if matched
      throw this.#createError(
        `Cannot resolve a node with !<${state.tag}> explicit tag`,
      );
    }

    const result = type.construct(state.result);
    state.result = result;
    const { anchor } = state;
    if (anchor !== null) this.anchorMap.set(anchor, result);
    return state;
  }
  composeNode({ parentIndent, nodeContext, allowToSeek, allowCompact }: {
    parentIndent: number;
    nodeContext: number;
    allowToSeek: boolean;
    allowCompact: boolean;
  }): State | void {
    let indentStatus = 1; // 1: this>parent, 0: this=parent, -1: this<parent
    let atNewLine = false;

    const allowBlockScalars = CONTEXT_BLOCK_OUT === nodeContext ||
      CONTEXT_BLOCK_IN === nodeContext;

    let allowBlockCollections = allowBlockScalars;
    const allowBlockStyles = allowBlockScalars;

    if (allowToSeek) {
      if (this.skipSeparationSpace(true, -1)) {
        atNewLine = true;
        indentStatus = getIndentStatus(this.lineIndent, parentIndent);
      }
    }

    let tag: string | null = null;
    let anchor: string | null = null;

    if (indentStatus === 1) {
      while (true) {
        const newTag = this.readTagProperty(tag);
        if (newTag) {
          tag = newTag;
        } else {
          const newAnchor = this.readAnchorProperty(anchor);
          if (!newAnchor) break;
          anchor = newAnchor;
        }
        if (this.skipSeparationSpace(true, -1)) {
          atNewLine = true;
          allowBlockCollections = allowBlockStyles;
          indentStatus = getIndentStatus(this.lineIndent, parentIndent);
        } else {
          allowBlockCollections = false;
        }
      }
    }

    if (allowBlockCollections) {
      allowBlockCollections = atNewLine || allowCompact;
    }

    if (indentStatus === 1) {
      const cond = CONTEXT_FLOW_IN === nodeContext ||
        CONTEXT_FLOW_OUT === nodeContext;
      const flowIndent = cond ? parentIndent : parentIndent + 1;

      if (allowBlockCollections) {
        const blockIndent = this.#scanner.position - this.lineStart;
        const blockSequenceState = this.readBlockSequence(
          tag,
          anchor,
          blockIndent,
        );
        if (blockSequenceState) return this.resolveTag(blockSequenceState);

        const blockMappingState = this.readBlockMapping(
          tag,
          anchor,
          blockIndent,
          flowIndent,
        );
        if (blockMappingState) return this.resolveTag(blockMappingState);
      }
      const flowCollectionState = this.readFlowCollection(
        tag,
        anchor,
        flowIndent,
      );
      if (flowCollectionState) return this.resolveTag(flowCollectionState);

      if (allowBlockScalars) {
        const blockScalarState = this.readBlockScalar(
          tag,
          anchor,
          flowIndent,
        );
        if (blockScalarState) return this.resolveTag(blockScalarState);
      }
      const singleQuoteState = this.readSingleQuotedScalar(
        tag,
        anchor,
        flowIndent,
      );
      if (singleQuoteState) return this.resolveTag(singleQuoteState);

      const doubleQuoteState = this.readDoubleQuotedScalar(
        tag,
        anchor,
        flowIndent,
      );
      if (doubleQuoteState) return this.resolveTag(doubleQuoteState);

      const alias = this.readAlias();
      if (alias) {
        if (tag !== null || anchor !== null) {
          throw this.#createError(
            "Cannot compose node: alias node should not have any properties",
          );
        }
        return this.resolveTag({ tag, anchor, kind: null, result: alias });
      }
      const plainScalarState = this.readPlainScalar(
        tag,
        anchor,
        flowIndent,
        CONTEXT_FLOW_IN === nodeContext,
      );
      if (plainScalarState) {
        plainScalarState.tag ??= "?";
        return this.resolveTag(plainScalarState);
      }
    } else if (
      indentStatus === 0 &&
      CONTEXT_BLOCK_OUT === nodeContext &&
      allowBlockCollections
    ) {
      // Special case: block sequences are allowed to have same indentation level as the parent.
      // http://www.yaml.org/spec/1.2/spec.html#id2799784
      const blockIndent = this.#scanner.position - this.lineStart;
      const newState = this.readBlockSequence(tag, anchor, blockIndent);
      if (newState) return this.resolveTag(newState);
    }

    const newState = this.resolveTag({ tag, anchor, kind: null, result: null });
    if (newState.tag !== null || newState.anchor !== null) return newState;
  }

  readDirectives() {
    let hasDirectives = false;
    let version = null;

    let ch = this.#scanner.peek();
    while (ch !== 0) {
      this.skipSeparationSpace(true, -1);

      ch = this.#scanner.peek();

      if (this.lineIndent > 0 || ch !== PERCENT) {
        break;
      }

      hasDirectives = true;
      this.#scanner.next();
      ch = this.#scanner.peek();
      let position = this.#scanner.position;

      while (ch !== 0 && !isWhiteSpaceOrEOL(ch)) {
        this.#scanner.next();
        ch = this.#scanner.peek();
      }

      const directiveName = this.#scanner.source.slice(
        position,
        this.#scanner.position,
      );
      const directiveArgs = [];

      if (directiveName.length < 1) {
        throw this.#createError(
          "Cannot read document: directive name length must be greater than zero",
        );
      }

      while (ch !== 0) {
        this.skipWhitespaces();
        this.skipComment();
        ch = this.#scanner.peek();

        if (isEOL(ch)) break;

        position = this.#scanner.position;

        while (ch !== 0 && !isWhiteSpaceOrEOL(ch)) {
          this.#scanner.next();
          ch = this.#scanner.peek();
        }

        directiveArgs.push(
          this.#scanner.source.slice(position, this.#scanner.position),
        );
      }

      if (ch !== 0) this.readLineBreak();

      switch (directiveName) {
        case "YAML":
          if (version !== null) {
            throw this.#createError(
              "Cannot handle YAML directive: duplication of %YAML directive",
            );
          }
          version = this.yamlDirectiveHandler(directiveArgs);
          break;
        case "TAG":
          this.tagDirectiveHandler(directiveArgs);
          break;
        default:
          this.dispatchWarning(`unknown document directive "${directiveName}"`);
          break;
      }

      ch = this.#scanner.peek();
    }
    return hasDirectives;
  }

  readDocument() {
    const documentStart = this.#scanner.position;

    this.checkLineBreaks = false;
    this.tagMap = new Map();
    this.anchorMap = new Map();

    const hasDirectives = this.readDirectives();

    this.skipSeparationSpace(true, -1);

    let result = null;

    if (
      this.lineIndent === 0 &&
      this.#scanner.peek() === MINUS &&
      this.#scanner.peek(1) === MINUS &&
      this.#scanner.peek(2) === MINUS
    ) {
      this.#scanner.position += 3;
      this.skipSeparationSpace(true, -1);
    } else if (hasDirectives) {
      throw this.#createError(
        "Cannot read document: directives end mark is expected",
      );
    }

    const newState = this.composeNode({
      parentIndent: this.lineIndent - 1,
      nodeContext: CONTEXT_BLOCK_OUT,
      allowToSeek: false,
      allowCompact: true,
    });
    if (newState) result = newState.result;
    this.skipSeparationSpace(true, -1);

    if (
      this.checkLineBreaks &&
      PATTERN_NON_ASCII_LINE_BREAKS_REGEXP.test(
        this.#scanner.source.slice(documentStart, this.#scanner.position),
      )
    ) {
      this.dispatchWarning("non-ASCII line breaks are interpreted as content");
    }

    if (
      this.#scanner.position === this.lineStart && this.testDocumentSeparator()
    ) {
      if (this.#scanner.peek() === DOT) {
        this.#scanner.position += 3;
        this.skipSeparationSpace(true, -1);
      }
    } else if (!this.#scanner.eof()) {
      throw this.#createError(
        "Cannot read document: end of the stream or a document separator is expected",
      );
    }

    return result;
  }

  *readDocuments() {
    while (!this.#scanner.eof()) {
      yield this.readDocument();
    }
  }
}
