// Originally ported from Go:
// https://github.com/golang/go/blob/go1.12.5/src/encoding/csv/
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/assert.ts";

export interface ReadOptions {
  /** Character which separates values.
   *
   * @default {","}
   */
  separator?: string;
  /** Character to start a comment.
   *
   * Lines beginning with the comment character without preceding whitespace
   * are ignored. With leading whitespace the comment character becomes part of
   * the field, even you provide `trimLeadingSpace: true`.
   *
   * @default {"#"}
   */
  comment?: string;
  /** Flag to trim the leading space of the value.
   *
   * This is done even if the field delimiter, `separator`, is white space.
   *
   * @default {false}
   */
  trimLeadingSpace?: boolean;
  /**
   * Allow unquoted quote in a quoted field or non-double-quoted quotes in
   * quoted field.
   *
   * @default {false}
   */
  lazyQuotes?: boolean;
  /**
   * Enabling checking number of expected fields for each row.
   *
   * If positive, each record is required to have the given number of fields.
   * If === 0, it will be set to the number of fields in the first row, so that
   * future rows must have the same field count.
   * If negative, no check is made and records may have a variable number of
   * fields.
   *
   * If the wrong number of fields is in a row, a `ParseError` is thrown.
   */
  fieldsPerRecord?: number;
}

export const defaultReadOptions: ReadOptions = {
  separator: ",",
  trimLeadingSpace: false,
};

export interface LineReader {
  readLine(): Promise<string | null>;
  isEOF(): boolean;
}

export async function parseRecord(
  line: string,
  reader: LineReader,
  opt: ReadOptions,
  startLine: number,
  lineIndex: number = startLine,
): Promise<Array<string> | null> {
  // line starting with comment character is ignored
  if (opt.comment && line[0] === opt.comment) {
    return [];
  }

  assert(opt.separator !== undefined);

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
      line = line.trimStart();
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
        } else if (line.length > 0 || !reader.isEOF()) {
          // Hit end of line (copy all data so far).
          recordBuffer += line;
          const r = await reader.readLine();
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

function runeCount(s: string): number {
  // Array.from considers the surrogate pair.
  return Array.from(s).length;
}

/**
 * A ParseError is returned for parsing errors.
 * Line numbers are 1-indexed and columns are 0-indexed.
 */
export class ParseError extends SyntaxError {
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

export const ERR_BARE_QUOTE = 'bare " in non-quoted-field';
export const ERR_QUOTE = 'extraneous or missing " in quoted-field';
export const ERR_INVALID_DELIM = "Invalid Delimiter";
export const ERR_FIELD_COUNT = "wrong number of fields";

export function convertRowToObject(
  row: string[],
  headers: readonly string[],
  index: number,
) {
  if (row.length !== headers.length) {
    throw new Error(
      `Error number of fields line: ${index}\nNumber of fields found: ${headers.length}\nExpected number of fields: ${row.length}`,
    );
  }
  const out: Record<string, unknown> = {};
  for (let i = 0; i < row.length; i++) {
    out[headers[i]] = row[i];
  }
  return out;
}

// deno-fmt-ignore
export type ParseResult<ParseOptions, T> =
  // If `columns` option is specified, the return type is Record type.
  T extends ParseOptions & { columns: readonly (infer C extends string)[] }
    ? RecordWithColumn<C>[]
  // If `skipFirstRow` option is specified, the return type is Record type.
  : T extends ParseOptions & { skipFirstRow: true }
    ? Record<string, string | undefined>[]
  // If `columns` and `skipFirstRow` option is _not_ specified, the return type is string[][].
  : T extends ParseOptions & { columns?: undefined; skipFirstRow?: false | undefined }
    ? string[][]
  // else, the return type is Record type or string[][].
  : Record<string, string | undefined>[] | string[][];

// RecordWithColumn<"aaa"|"bbb"> => Record<"aaa"|"bbb", string>
// RecordWithColumn<string> => Record<string, string | undefined>
type RecordWithColumn<C extends string> = string extends C
  ? Record<string, string | undefined>
  : Record<C, string>;
