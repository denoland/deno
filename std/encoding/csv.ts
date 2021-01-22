// Ported from Go:
// https://github.com/golang/go/blob/go1.12.5/src/encoding/csv/
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { StringReader } from "../io/readers.ts";
import { assert } from "../_util/assert.ts";

export { NEWLINE, stringify, StringifyError } from "./csv_stringify.ts";

export type {
  Column,
  ColumnDetails,
  DataItem,
  StringifyOptions,
} from "./csv_stringify.ts";

const INVALID_RUNE = ["\r", "\n", '"'];

export const ERR_BARE_QUOTE = 'bare " in non-quoted-field';
export const ERR_QUOTE = 'extraneous or missing " in quoted-field';
export const ERR_INVALID_DELIM = "Invalid Delimiter";
export const ERR_FIELD_COUNT = "wrong number of fields";

/**
 * A ParseError is returned for parsing errors.
 * Line numbers are 1-indexed and columns are 0-indexed.
 */
export class ParseError extends Error {
  /** Line where the record starts*/
  startLine: number;
  /** Line where the error occurred */
  line: number;
  /** Column (rune index) where the error occurred */
  column: number | null;

  constructor(
    start: number,
    line: number,
    column: number | null,
    message: string,
  ) {
    super();
    this.startLine = start;
    this.column = column;
    this.line = line;

    if (message === ERR_FIELD_COUNT) {
      this.message = `record on line ${line}: ${message}`;
    } else if (start !== line) {
      this.message =
        `record on line ${start}; parse error on line ${line}, column ${column}: ${message}`;
    } else {
      this.message =
        `parse error on line ${line}, column ${column}: ${message}`;
    }
  }
}

/**
 * @property separator - Character which separates values. Default: ','
 * @property comment - Character to start a comment. Default: '#'
 * @property trimLeadingSpace - Flag to trim the leading space of the value.
 *           Default: 'false'
 * @property lazyQuotes - Allow unquoted quote in a quoted field or non double
 *           quoted quotes in quoted field. Default: 'false'
 * @property fieldsPerRecord - Enabling the check of fields for each row.
 *           If == 0, first row is used as referral for the number of fields.
 */
export interface ReadOptions {
  separator?: string;
  comment?: string;
  trimLeadingSpace?: boolean;
  lazyQuotes?: boolean;
  fieldsPerRecord?: number;
}

function chkOptions(opt: ReadOptions): void {
  if (!opt.separator) {
    opt.separator = ",";
  }
  if (!opt.trimLeadingSpace) {
    opt.trimLeadingSpace = false;
  }
  if (
    INVALID_RUNE.includes(opt.separator) ||
    (typeof opt.comment === "string" && INVALID_RUNE.includes(opt.comment)) ||
    opt.separator === opt.comment
  ) {
    throw new Error(ERR_INVALID_DELIM);
  }
}

async function readRecord(
  startLine: number,
  reader: BufReader,
  opt: ReadOptions = { separator: ",", trimLeadingSpace: false },
): Promise<string[] | null> {
  const tp = new TextProtoReader(reader);
  let line = await readLine(tp);
  let lineIndex = startLine + 1;

  if (line === null) return null;
  if (line.length === 0) {
    return [];
  }
  // line starting with comment character is ignored
  if (opt.comment && line[0] === opt.comment) {
    return [];
  }

  assert(opt.separator != null);

  let fullLine = line;
  let quoteError: ParseError | null = null;
  const quote = '"';
  const quoteLen = quote.length;
  const separatorLen = opt.separator.length;
  let recordBuffer = "";
  const fieldIndexes = [] as number[];
  parseField:
  for (;;) {
    if (opt.trimLeadingSpace) {
      line = line.trimLeft();
    }

    if (line.length === 0 || !line.startsWith(quote)) {
      // Non-quoted string field
      const i = line.indexOf(opt.separator);
      let field = line;
      if (i >= 0) {
        field = field.substring(0, i);
      }
      // Check to make sure a quote does not appear in field.
      if (!opt.lazyQuotes) {
        const j = field.indexOf(quote);
        if (j >= 0) {
          const col = runeCount(
            fullLine.slice(0, fullLine.length - line.slice(j).length),
          );
          quoteError = new ParseError(
            startLine + 1,
            lineIndex,
            col,
            ERR_BARE_QUOTE,
          );
          break parseField;
        }
      }
      recordBuffer += field;
      fieldIndexes.push(recordBuffer.length);
      if (i >= 0) {
        line = line.substring(i + separatorLen);
        continue parseField;
      }
      break parseField;
    } else {
      // Quoted string field
      line = line.substring(quoteLen);
      for (;;) {
        const i = line.indexOf(quote);
        if (i >= 0) {
          // Hit next quote.
          recordBuffer += line.substring(0, i);
          line = line.substring(i + quoteLen);
          if (line.startsWith(quote)) {
            // `""` sequence (append quote).
            recordBuffer += quote;
            line = line.substring(quoteLen);
          } else if (line.startsWith(opt.separator)) {
            // `","` sequence (end of field).
            line = line.substring(separatorLen);
            fieldIndexes.push(recordBuffer.length);
            continue parseField;
          } else if (0 === line.length) {
            // `"\n` sequence (end of line).
            fieldIndexes.push(recordBuffer.length);
            break parseField;
          } else if (opt.lazyQuotes) {
            // `"` sequence (bare quote).
            recordBuffer += quote;
          } else {
            // `"*` sequence (invalid non-escaped quote).
            const col = runeCount(
              fullLine.slice(0, fullLine.length - line.length - quoteLen),
            );
            quoteError = new ParseError(
              startLine + 1,
              lineIndex,
              col,
              ERR_QUOTE,
            );
            break parseField;
          }
        } else if (line.length > 0 || !(await isEOF(tp))) {
          // Hit end of line (copy all data so far).
          recordBuffer += line;
          const r = await readLine(tp);
          lineIndex++;
          line = r ?? ""; // This is a workaround for making this module behave similarly to the encoding/csv/reader.go.
          fullLine = line;
          if (r === null) {
            // Abrupt end of file (EOF or error).
            if (!opt.lazyQuotes) {
              const col = runeCount(fullLine);
              quoteError = new ParseError(
                startLine + 1,
                lineIndex,
                col,
                ERR_QUOTE,
              );
              break parseField;
            }
            fieldIndexes.push(recordBuffer.length);
            break parseField;
          }
          recordBuffer += "\n"; // preserve line feed (This is because TextProtoReader removes it.)
        } else {
          // Abrupt end of file (EOF on error).
          if (!opt.lazyQuotes) {
            const col = runeCount(fullLine);
            quoteError = new ParseError(
              startLine + 1,
              lineIndex,
              col,
              ERR_QUOTE,
            );
            break parseField;
          }
          fieldIndexes.push(recordBuffer.length);
          break parseField;
        }
      }
    }
  }
  if (quoteError) {
    throw quoteError;
  }
  const result = [] as string[];
  let preIdx = 0;
  for (const i of fieldIndexes) {
    result.push(recordBuffer.slice(preIdx, i));
    preIdx = i;
  }
  return result;
}

async function isEOF(tp: TextProtoReader): Promise<boolean> {
  return (await tp.r.peek(0)) === null;
}

function runeCount(s: string): number {
  // Array.from considers the surrogate pair.
  return Array.from(s).length;
}

async function readLine(tp: TextProtoReader): Promise<string | null> {
  let line: string;
  const r = await tp.readLine();
  if (r === null) return null;
  line = r;

  // For backwards compatibility, drop trailing \r before EOF.
  if ((await isEOF(tp)) && line.length > 0 && line[line.length - 1] === "\r") {
    line = line.substring(0, line.length - 1);
  }

  // Normalize \r\n to \n on all input lines.
  if (
    line.length >= 2 &&
    line[line.length - 2] === "\r" &&
    line[line.length - 1] === "\n"
  ) {
    line = line.substring(0, line.length - 2);
    line = line + "\n";
  }

  return line;
}

/**
 * Parse the CSV from the `reader` with the options provided and return `string[][]`.
 *
 * @param reader provides the CSV data to parse
 * @param opt controls the parsing behavior
 */
export async function readMatrix(
  reader: BufReader,
  opt: ReadOptions = {
    separator: ",",
    trimLeadingSpace: false,
    lazyQuotes: false,
  },
): Promise<string[][]> {
  const result: string[][] = [];
  let _nbFields: number | undefined;
  let lineResult: string[];
  let first = true;
  let lineIndex = 0;
  chkOptions(opt);

  for (;;) {
    const r = await readRecord(lineIndex, reader, opt);
    if (r === null) break;
    lineResult = r;
    lineIndex++;
    // If fieldsPerRecord is 0, Read sets it to
    // the number of fields in the first record
    if (first) {
      first = false;
      if (opt.fieldsPerRecord !== undefined) {
        if (opt.fieldsPerRecord === 0) {
          _nbFields = lineResult.length;
        } else {
          _nbFields = opt.fieldsPerRecord;
        }
      }
    }

    if (lineResult.length > 0) {
      if (_nbFields && _nbFields !== lineResult.length) {
        throw new ParseError(lineIndex, lineIndex, null, ERR_FIELD_COUNT);
      }
      result.push(lineResult);
    }
  }
  return result;
}

/**
 * Parse the CSV string/buffer with the options provided.
 *
 * ColumnOptions provides the column definition
 * and the parse function for each entry of the
 * column.
 */
export interface ColumnOptions {
  /**
   * Name of the column to be used as property
   */
  name: string;
  /**
   * Parse function for the column.
   * This is executed on each entry of the header.
   * This can be combined with the Parse function of the rows.
   */
  parse?: (input: string) => unknown;
}

export interface ParseOptions extends ReadOptions {
  /**
   * If you provide `skipFirstRow: true` and `columns`, the first line will be skipped.
   * If you provide `skipFirstRow: true` but not `columns`, the first line will be skipped and used as header definitions.
   */
  skipFirstRow?: boolean;

  /**
   * If you provide `string[]` or `ColumnOptions[]`, those names will be used for header definition.
   */
  columns?: string[] | ColumnOptions[];

  /** Parse function for rows.
   * Example:
   *     const r = await parseFile('a,b,c\ne,f,g\n', {
   *      columns: ["this", "is", "sparta"],
   *       parse: (e: Record<string, unknown>) => {
   *         return { super: e.this, street: e.is, fighter: e.sparta };
   *       }
   *     });
   * // output
   * [
   *   { super: "a", street: "b", fighter: "c" },
   *   { super: "e", street: "f", fighter: "g" }
   * ]
   */
  parse?: (input: unknown) => unknown;
}

/**
 * Csv parse helper to manipulate data.
 * Provides an auto/custom mapper for columns and parse function
 * for columns and rows.
 * @param input Input to parse. Can be a string or BufReader.
 * @param opt options of the parser.
 * @returns If you don't provide `opt.skipFirstRow`, `opt.parse`, and `opt.columns`, it returns `string[][]`.
 *   If you provide `opt.skipFirstRow` or `opt.columns` but not `opt.parse`, it returns `object[]`.
 *   If you provide `opt.parse`, it returns an array where each element is the value returned from `opt.parse`.
 */
export async function parse(
  input: string | BufReader,
  opt: ParseOptions = {
    skipFirstRow: false,
  },
): Promise<unknown[]> {
  let r: string[][];
  if (input instanceof BufReader) {
    r = await readMatrix(input, opt);
  } else {
    r = await readMatrix(new BufReader(new StringReader(input)), opt);
  }
  if (opt.skipFirstRow || opt.columns) {
    let headers: ColumnOptions[] = [];
    let i = 0;

    if (opt.skipFirstRow) {
      const head = r.shift();
      assert(head != null);
      headers = head.map(
        (e): ColumnOptions => {
          return {
            name: e,
          };
        },
      );
      i++;
    }

    if (opt.columns) {
      if (typeof opt.columns[0] !== "string") {
        headers = opt.columns as ColumnOptions[];
      } else {
        const h = opt.columns as string[];
        headers = h.map(
          (e): ColumnOptions => {
            return {
              name: e,
            };
          },
        );
      }
    }
    return r.map((e): unknown => {
      if (e.length !== headers.length) {
        throw `Error number of fields line:${i}`;
      }
      i++;
      const out: Record<string, unknown> = {};
      for (let j = 0; j < e.length; j++) {
        const h = headers[j];
        if (h.parse) {
          out[h.name] = h.parse(e[j]);
        } else {
          out[h.name] = e[j];
        }
      }
      if (opt.parse) {
        return opt.parse(out);
      }
      return out;
    });
  }
  if (opt.parse) {
    return r.map((e: string[]): unknown => {
      assert(opt.parse, "opt.parse must be set");
      return opt.parse(e);
    });
  }
  return r;
}
