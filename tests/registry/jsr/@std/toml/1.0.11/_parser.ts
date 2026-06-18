// Copyright 2018-2025 the Deno authors. MIT license.
// This module is browser compatible.

import { deepMerge } from "jsr:@std/collections@^1.1.3/deep-merge";

/**
 * Copy of `import { isLeap } from "@std/datetime";` because it cannot be impoted as long as it is unstable.
 */
function isLeap(yearNumber: number): boolean {
  return (
    (yearNumber % 4 === 0 && yearNumber % 100 !== 0) || yearNumber % 400 === 0
  );
}

// ---------------------------
// Interfaces and base classes
// ---------------------------

interface Success<T> {
  ok: true;
  body: T;
}
interface Failure {
  ok: false;
}
type ParseResult<T> = Success<T> | Failure;

type ParserComponent<T = unknown> = (scanner: Scanner) => ParseResult<T>;

type Block = {
  type: "Block";
  value: Record<string, unknown>;
};
type Table = {
  type: "Table";
  keys: string[];
  value: Record<string, unknown>;
};
type TableArray = {
  type: "TableArray";
  keys: string[];
  value: Record<string, unknown>;
};

export class Scanner {
  #whitespace = /[ \t]/;
  #position = 0;
  #source: string;

  constructor(source: string) {
    this.#source = source;
  }

  get position() {
    return this.#position;
  }
  get source() {
    return this.#source;
  }

  /**
   * Get current character
   * @param index - relative index from current position
   */
  char(index = 0) {
    return this.#source[this.#position + index] ?? "";
  }

  /**
   * Get sliced string
   * @param start - start position relative from current position
   * @param end - end position relative from current position
   */
  slice(start: number, end: number): string {
    return this.#source.slice(this.#position + start, this.#position + end);
  }

  /**
   * Move position to next
   */
  next(count: number = 1) {
    this.#position += count;
  }

  skipWhitespaces() {
    while (this.#whitespace.test(this.char()) && !this.eof()) {
      this.next();
    }
    // Invalid if current char is other kinds of whitespace
    if (!this.isCurrentCharEOL() && /\s/.test(this.char())) {
      const escaped = "\\u" + this.char().charCodeAt(0).toString(16);
      const position = this.#position;
      throw new SyntaxError(
        `Cannot parse the TOML: It contains invalid whitespace at position '${position}': \`${escaped}\``,
      );
    }
  }

  nextUntilChar(options: { skipComments?: boolean } = { skipComments: true }) {
    while (!this.eof()) {
      const char = this.char();
      if (this.#whitespace.test(char) || this.isCurrentCharEOL()) {
        this.next();
      } else if (options.skipComments && this.char() === "#") {
        // entering comment
        while (!this.isCurrentCharEOL() && !this.eof()) {
          this.next();
        }
      } else {
        break;
      }
    }
  }

  /**
   * Position reached EOF or not
   */
  eof() {
    return this.#position >= this.#source.length;
  }

  isCurrentCharEOL() {
    return this.char() === "\n" || this.startsWith("\r\n");
  }

  startsWith(searchString: string) {
    return this.#source.startsWith(searchString, this.#position);
  }

  match(regExp: RegExp) {
    if (!regExp.sticky) {
      throw new Error(`RegExp ${regExp} does not have a sticky 'y' flag`);
    }
    regExp.lastIndex = this.#position;
    return this.#source.match(regExp);
  }
}

// -----------------------
// Utilities
// -----------------------

function success<T>(body: T): Success<T> {
  return { ok: true, body };
}
function failure(): Failure {
  return { ok: false };
}

/**
 * Creates a nested object from the keys and values.
 *
 * e.g. `unflat(["a", "b", "c"], 1)` returns `{ a: { b: { c: 1 } } }`
 */
export function unflat(
  keys: string[],
  values: unknown = { __proto__: null },
): Record<string, unknown> {
  return keys.reduceRight(
    (acc, key) => ({ [key]: acc }),
    values,
  ) as Record<string, unknown>;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function getTargetValue(target: Record<string, unknown>, keys: string[]) {
  const key = keys[0];
  if (!key) {
    throw new Error(
      "Cannot parse the TOML: key length is not a positive number",
    );
  }
  return target[key];
}

function deepAssignTable(
  target: Record<string, unknown>,
  table: Table,
) {
  const { keys, type, value } = table;
  const currentValue = getTargetValue(target, keys);

  if (currentValue === undefined) {
    return Object.assign(target, unflat(keys, value));
  }
  if (Array.isArray(currentValue)) {
    const last = currentValue.at(-1);
    deepAssign(last, { type, keys: keys.slice(1), value });
    return target;
  }
  if (isObject(currentValue)) {
    deepAssign(currentValue, { type, keys: keys.slice(1), value });
    return target;
  }
  throw new Error("Unexpected assign");
}

function deepAssignTableArray(
  target: Record<string, unknown>,
  table: TableArray,
) {
  const { type, keys, value } = table;
  const currentValue = getTargetValue(target, keys);

  if (currentValue === undefined) {
    return Object.assign(target, unflat(keys, [value]));
  }
  if (Array.isArray(currentValue)) {
    if (table.keys.length === 1) {
      currentValue.push(value);
    } else {
      const last = currentValue.at(-1);
      deepAssign(last, {
        type: table.type,
        keys: table.keys.slice(1),
        value: table.value,
      });
    }
    return target;
  }
  if (isObject(currentValue)) {
    deepAssign(currentValue, { type, keys: keys.slice(1), value });
    return target;
  }
  throw new Error("Unexpected assign");
}

export function deepAssign(
  target: Record<string, unknown>,
  body: Block | Table | TableArray,
) {
  switch (body.type) {
    case "Block":
      return deepMerge(target, body.value);
    case "Table":
      return deepAssignTable(target, body);
    case "TableArray":
      return deepAssignTableArray(target, body);
  }
}

// ---------------------------------
// Parser combinators and generators
// ---------------------------------

// deno-lint-ignore no-explicit-any
function or<T extends readonly ParserComponent<any>[]>(
  parsers: T,
): ParserComponent<
  ReturnType<T[number]> extends ParseResult<infer R> ? R : Failure
> {
  return (scanner: Scanner) => {
    for (const parse of parsers) {
      const result = parse(scanner);
      if (result.ok) return result;
    }
    return failure();
  };
}

/** Join the parse results of the given parser into an array.
 *
 * If the parser fails at the first attempt, it will return an empty array.
 */
function join<T>(
  parser: ParserComponent<T>,
  separator: string,
): ParserComponent<T[]> {
  const Separator = character(separator);
  return (scanner: Scanner): ParseResult<T[]> => {
    const out: T[] = [];
    const first = parser(scanner);
    if (!first.ok) return success(out);
    out.push(first.body);
    while (!scanner.eof()) {
      if (!Separator(scanner).ok) break;
      const result = parser(scanner);
      if (!result.ok) {
        throw new SyntaxError(`Invalid token after "${separator}"`);
      }
      out.push(result.body);
    }
    return success(out);
  };
}

/** Join the parse results of the given parser into an array.
 *
 * This requires the parser to succeed at least once.
 */
function join1<T>(
  parser: ParserComponent<T>,
  separator: string,
): ParserComponent<T[]> {
  const Separator = character(separator);
  return (scanner: Scanner): ParseResult<T[]> => {
    const first = parser(scanner);
    if (!first.ok) return failure();
    const out: T[] = [first.body];
    while (!scanner.eof()) {
      if (!Separator(scanner).ok) break;
      const result = parser(scanner);
      if (!result.ok) {
        throw new SyntaxError(`Invalid token after "${separator}"`);
      }
      out.push(result.body);
    }
    return success(out);
  };
}

function kv<T>(
  keyParser: ParserComponent<string[]>,
  separator: string,
  valueParser: ParserComponent<T>,
): ParserComponent<{ [key: string]: unknown }> {
  const Separator = character(separator);
  return (scanner: Scanner): ParseResult<{ [key: string]: unknown }> => {
    const position = scanner.position;
    const key = keyParser(scanner);
    if (!key.ok) return failure();
    const sep = Separator(scanner);
    if (!sep.ok) {
      throw new SyntaxError(`key/value pair doesn't have "${separator}"`);
    }
    const value = valueParser(scanner);
    if (!value.ok) {
      const lineEndIndex = scanner.source.indexOf("\n", scanner.position);
      const endPosition = lineEndIndex > 0
        ? lineEndIndex
        : scanner.source.length;
      const line = scanner.source.slice(position, endPosition);
      throw new SyntaxError(`Cannot parse value on line '${line}'`);
    }
    return success(unflat(key.body, value.body));
  };
}

function merge(
  parser: ParserComponent<unknown[]>,
): ParserComponent<Record<string, unknown>> {
  return (scanner: Scanner): ParseResult<Record<string, unknown>> => {
    const result = parser(scanner);
    if (!result.ok) return failure();
    let body = { __proto__: null };
    for (const record of result.body) {
      if (typeof record === "object" && record !== null) {
        body = deepMerge(body, record);
      }
    }
    return success(body);
  };
}

function repeat<T>(
  parser: ParserComponent<T>,
): ParserComponent<T[]> {
  return (scanner: Scanner) => {
    const body: T[] = [];
    while (!scanner.eof()) {
      const result = parser(scanner);
      if (!result.ok) break;
      body.push(result.body);
      scanner.nextUntilChar();
    }
    if (body.length === 0) return failure();
    return success(body);
  };
}

function surround<T>(
  left: string,
  parser: ParserComponent<T>,
  right: string,
): ParserComponent<T> {
  const Left = character(left);
  const Right = character(right);
  return (scanner: Scanner) => {
    if (!Left(scanner).ok) {
      return failure();
    }
    const result = parser(scanner);
    if (!result.ok) {
      throw new SyntaxError(`Invalid token after "${left}"`);
    }
    if (!Right(scanner).ok) {
      throw new SyntaxError(
        `Not closed by "${right}" after started with "${left}"`,
      );
    }
    return success(result.body);
  };
}

function character(str: string) {
  return (scanner: Scanner): ParseResult<void> => {
    scanner.skipWhitespaces();
    if (!scanner.startsWith(str)) return failure();
    scanner.next(str.length);
    scanner.skipWhitespaces();
    return success(undefined);
  };
}

// -----------------------
// Parser components
// -----------------------

const BARE_KEY_REGEXP = /[A-Za-z0-9_-]+/y;
export function bareKey(scanner: Scanner): ParseResult<string> {
  scanner.skipWhitespaces();
  const key = scanner.match(BARE_KEY_REGEXP)?.[0];
  if (!key) return failure();
  scanner.next(key.length);
  return success(key);
}

function escapeSequence(scanner: Scanner): ParseResult<string> {
  if (scanner.char() !== "\\") return failure();
  scanner.next();
  // See https://toml.io/en/v1.0.0-rc.3#string
  switch (scanner.char()) {
    case "b":
      scanner.next();
      return success("\b");
    case "t":
      scanner.next();
      return success("\t");
    case "n":
      scanner.next();
      return success("\n");
    case "f":
      scanner.next();
      return success("\f");
    case "r":
      scanner.next();
      return success("\r");
    case "u":
    case "U": {
      // Unicode character
      const codePointLen = scanner.char() === "u" ? 4 : 6;
      const codePoint = parseInt("0x" + scanner.slice(1, 1 + codePointLen), 16);
      const str = String.fromCodePoint(codePoint);
      scanner.next(codePointLen + 1);
      return success(str);
    }
    case '"':
      scanner.next();
      return success('"');
    case "\\":
      scanner.next();
      return success("\\");
    default:
      throw new SyntaxError(
        `Invalid escape sequence: \\${scanner.char()}`,
      );
  }
}

export function basicString(scanner: Scanner): ParseResult<string> {
  scanner.skipWhitespaces();
  if (scanner.char() !== '"') return failure();
  scanner.next();
  const acc = [];
  while (scanner.char() !== '"' && !scanner.eof()) {
    if (scanner.char() === "\n") {
      throw new SyntaxError("Single-line string cannot contain EOL");
    }
    const escapedChar = escapeSequence(scanner);
    if (escapedChar.ok) {
      acc.push(escapedChar.body);
    } else {
      acc.push(scanner.char());
      scanner.next();
    }
  }
  if (scanner.eof()) {
    throw new SyntaxError(
      `Single-line string is not closed:\n${acc.join("")}`,
    );
  }
  scanner.next(); // skip last '""
  return success(acc.join(""));
}

export function literalString(scanner: Scanner): ParseResult<string> {
  scanner.skipWhitespaces();
  if (scanner.char() !== "'") return failure();
  scanner.next();
  const acc: string[] = [];
  while (scanner.char() !== "'" && !scanner.eof()) {
    if (scanner.char() === "\n") {
      throw new SyntaxError("Single-line string cannot contain EOL");
    }
    acc.push(scanner.char());
    scanner.next();
  }
  if (scanner.eof()) {
    throw new SyntaxError(
      `Single-line string is not closed:\n${acc.join("")}`,
    );
  }
  scanner.next(); // skip last "'"
  return success(acc.join(""));
}

export function multilineBasicString(
  scanner: Scanner,
): ParseResult<string> {
  scanner.skipWhitespaces();
  if (!scanner.startsWith('"""')) return failure();
  scanner.next(3);
  if (scanner.char() === "\n") {
    // The first newline (LF) is trimmed
    scanner.next();
  } else if (scanner.startsWith("\r\n")) {
    // The first newline (CRLF) is trimmed
    scanner.next(2);
  }
  const acc: string[] = [];
  while (!scanner.startsWith('"""') && !scanner.eof()) {
    // line ending backslash
    if (scanner.startsWith("\\\n")) {
      scanner.next();
      scanner.nextUntilChar({ skipComments: false });
      continue;
    } else if (scanner.startsWith("\\\r\n")) {
      scanner.next();
      scanner.nextUntilChar({ skipComments: false });
      continue;
    }
    const escapedChar = escapeSequence(scanner);
    if (escapedChar.ok) {
      acc.push(escapedChar.body);
    } else {
      acc.push(scanner.char());
      scanner.next();
    }
  }

  if (scanner.eof()) {
    throw new SyntaxError(
      `Multi-line string is not closed:\n${acc.join("")}`,
    );
  }
  // if ends with 4 `"`, push the fist `"` to string
  if (scanner.char(3) === '"') {
    acc.push('"');
    scanner.next();
  }
  scanner.next(3); // skip last '""""
  return success(acc.join(""));
}

export function multilineLiteralString(
  scanner: Scanner,
): ParseResult<string> {
  scanner.skipWhitespaces();
  if (!scanner.startsWith("'''")) return failure();
  scanner.next(3);
  if (scanner.char() === "\n") {
    // The first newline (LF) is trimmed
    scanner.next();
  } else if (scanner.startsWith("\r\n")) {
    // The first newline (CRLF) is trimmed
    scanner.next(2);
  }
  const acc: string[] = [];
  while (!scanner.startsWith("'''") && !scanner.eof()) {
    acc.push(scanner.char());
    scanner.next();
  }
  if (scanner.eof()) {
    throw new SyntaxError(
      `Multi-line string is not closed:\n${acc.join("")}`,
    );
  }
  // if ends with 4 `'`, push the fist `'` to string
  if (scanner.char(3) === "'") {
    acc.push("'");
    scanner.next();
  }
  scanner.next(3); // skip last "'''"
  return success(acc.join(""));
}

const BOOLEAN_REGEXP = /(?:true|false)\b/y;
export function boolean(scanner: Scanner): ParseResult<boolean> {
  scanner.skipWhitespaces();
  const match = scanner.match(BOOLEAN_REGEXP);
  if (!match) return failure();
  const string = match[0];
  scanner.next(string.length);
  const value = string === "true";
  return success(value);
}

const INFINITY_MAP = new Map<string, number>([
  ["inf", Infinity],
  ["+inf", Infinity],
  ["-inf", -Infinity],
]);
const INFINITY_REGEXP = /[+-]?inf\b/y;
export function infinity(scanner: Scanner): ParseResult<number> {
  scanner.skipWhitespaces();
  const match = scanner.match(INFINITY_REGEXP);
  if (!match) return failure();
  const string = match[0];
  scanner.next(string.length);
  const value = INFINITY_MAP.get(string)!;
  return success(value);
}

const NAN_REGEXP = /[+-]?nan\b/y;
export function nan(scanner: Scanner): ParseResult<number> {
  scanner.skipWhitespaces();
  const match = scanner.match(NAN_REGEXP);
  if (!match) return failure();
  const string = match[0];
  scanner.next(string.length);
  const value = NaN;
  return success(value);
}

export const dottedKey = join1(or([bareKey, basicString, literalString]), ".");

const BINARY_REGEXP = /0b[01]+(?:_[01]+)*\b/y;
export function binary(scanner: Scanner): ParseResult<number | string> {
  scanner.skipWhitespaces();
  const match = scanner.match(BINARY_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  const value = match.slice(2).replaceAll("_", "");
  const number = parseInt(value, 2);
  return isNaN(number) ? failure() : success(number);
}

const OCTAL_REGEXP = /0o[0-7]+(?:_[0-7]+)*\b/y;
export function octal(scanner: Scanner): ParseResult<number | string> {
  scanner.skipWhitespaces();
  const match = scanner.match(OCTAL_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  const value = match.slice(2).replaceAll("_", "");
  const number = parseInt(value, 8);
  return isNaN(number) ? failure() : success(number);
}

const HEX_REGEXP = /0x[0-9a-f]+(?:_[0-9a-f]+)*\b/yi;
export function hex(scanner: Scanner): ParseResult<number | string> {
  scanner.skipWhitespaces();
  const match = scanner.match(HEX_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  const value = match.slice(2).replaceAll("_", "");
  const number = parseInt(value, 16);
  return isNaN(number) ? failure() : success(number);
}

const INTEGER_REGEXP = /[+-]?(?:0|[1-9][0-9]*(?:_[0-9]+)*)\b/y;
export function integer(scanner: Scanner): ParseResult<number | string> {
  scanner.skipWhitespaces();
  const match = scanner.match(INTEGER_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  const value = match.replaceAll("_", "");
  const int = parseInt(value, 10);
  return success(int);
}

const FLOAT_REGEXP =
  /[+-]?(?:0|[1-9][0-9]*(?:_[0-9]+)*)(?:\.[0-9]+(?:_[0-9]+)*)?(?:e[+-]?[0-9]+(?:_[0-9]+)*)?\b/yi;
export function float(scanner: Scanner): ParseResult<number> {
  scanner.skipWhitespaces();
  const match = scanner.match(FLOAT_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  const value = match.replaceAll("_", "");
  const float = parseFloat(value);
  if (isNaN(float)) return failure();
  return success(float);
}

const DATE_TIME_REGEXP =
  /(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})(?:[ 0-9TZ.:+-]+)?\b/y;
export function dateTime(scanner: Scanner): ParseResult<Date> {
  scanner.skipWhitespaces();
  const match = scanner.match(DATE_TIME_REGEXP);
  if (!match) return failure();
  const string = match[0];
  scanner.next(string.length);
  const groups = match.groups as { year: string; month: string; day: string };
  // special case if month is February
  if (groups.month == "02") {
    const days = parseInt(groups.day);
    if (days > 29) {
      throw new SyntaxError(`Invalid date string "${match}"`);
    }
    const year = parseInt(groups.year);
    if (days > 28 && !isLeap(year)) {
      throw new SyntaxError(`Invalid date string "${match}"`);
    }
  }
  const date = new Date(string.trim());
  // invalid date
  if (isNaN(date.getTime())) {
    throw new SyntaxError(`Invalid date string "${match}"`);
  }
  return success(date);
}

const LOCAL_TIME_REGEXP = /(\d{2}):(\d{2}):(\d{2})(?:\.[0-9]+)?\b/y;
export function localTime(scanner: Scanner): ParseResult<string> {
  scanner.skipWhitespaces();

  const match = scanner.match(LOCAL_TIME_REGEXP)?.[0];
  if (!match) return failure();
  scanner.next(match.length);
  return success(match);
}

export function arrayValue(scanner: Scanner): ParseResult<unknown[]> {
  scanner.skipWhitespaces();

  if (scanner.char() !== "[") return failure();
  scanner.next();

  const array: unknown[] = [];
  while (!scanner.eof()) {
    scanner.nextUntilChar();
    const result = value(scanner);
    if (!result.ok) break;
    array.push(result.body);
    scanner.skipWhitespaces();
    // may have a next item, but trailing comma is allowed at array
    if (scanner.char() !== ",") break;
    scanner.next();
  }
  scanner.nextUntilChar();

  if (scanner.char() !== "]") throw new SyntaxError("Array is not closed");
  scanner.next();

  return success(array);
}

export function inlineTable(
  scanner: Scanner,
): ParseResult<Record<string, unknown>> {
  scanner.nextUntilChar();
  if (scanner.char(1) === "}") {
    scanner.next(2);
    return success({ __proto__: null });
  }
  const pairs = surround("{", join(pair, ","), "}")(scanner);
  if (!pairs.ok) return failure();
  let table = { __proto__: null } as Record<string, unknown>;
  for (const pair of pairs.body) {
    table = deepMerge(table, pair);
  }
  return success(table);
}

export const value = or([
  multilineBasicString,
  multilineLiteralString,
  basicString,
  literalString,
  boolean,
  infinity,
  nan,
  dateTime,
  localTime,
  binary,
  octal,
  hex,
  float,
  integer,
  arrayValue,
  inlineTable,
]);

export const pair = kv(dottedKey, "=", value);

export function block(
  scanner: Scanner,
): ParseResult<Block> {
  scanner.nextUntilChar();
  const result = merge(repeat(pair))(scanner);
  if (result.ok) return success({ type: "Block", value: result.body });
  return failure();
}

export const tableHeader = surround("[", dottedKey, "]");

export function table(scanner: Scanner): ParseResult<Table> {
  scanner.nextUntilChar();
  const header = tableHeader(scanner);
  if (!header.ok) return failure();
  scanner.nextUntilChar();
  const b = block(scanner);
  return success({
    type: "Table",
    keys: header.body,
    value: b.ok ? b.body.value : { __proto__: null },
  });
}

export const tableArrayHeader = surround("[[", dottedKey, "]]");

export function tableArray(
  scanner: Scanner,
): ParseResult<TableArray> {
  scanner.nextUntilChar();
  const header = tableArrayHeader(scanner);
  if (!header.ok) return failure();
  scanner.nextUntilChar();
  const b = block(scanner);
  return success({
    type: "TableArray",
    keys: header.body,
    value: b.ok ? b.body.value : { __proto__: null },
  });
}

export function toml(
  scanner: Scanner,
): ParseResult<Record<string, unknown>> {
  const blocks = repeat(or([block, tableArray, table]))(scanner);
  if (!blocks.ok) return success({ __proto__: null });
  const body = blocks.body.reduce(deepAssign, { __proto__: null });
  return success(body);
}

function createParseErrorMessage(scanner: Scanner, message: string) {
  const string = scanner.source.slice(0, scanner.position);
  const lines = string.split("\n");
  const row = lines.length;
  const column = lines.at(-1)?.length ?? 0;
  return `Parse error on line ${row}, column ${column}: ${message}`;
}

export function parserFactory<T>(parser: ParserComponent<T>) {
  return (tomlString: string): T => {
    const scanner = new Scanner(tomlString);
    try {
      const result = parser(scanner);
      if (result.ok && scanner.eof()) return result.body;
      const message = `Unexpected character: "${scanner.char()}"`;
      throw new SyntaxError(createParseErrorMessage(scanner, message));
    } catch (error) {
      if (error instanceof Error) {
        throw new SyntaxError(createParseErrorMessage(scanner, error.message));
      }
      const message = "Invalid error type caught";
      throw new SyntaxError(createParseErrorMessage(scanner, message));
    }
  };
}
