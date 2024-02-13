// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import {
  convertRowToObject,
  ERR_BARE_QUOTE,
  ERR_FIELD_COUNT,
  ERR_INVALID_DELIM,
  ERR_QUOTE,
  ParseError,
  type ParseResult,
  type ReadOptions,
} from "./_io.ts";
import { assert } from "../assert/assert.ts";

export { ParseError, ReadOptions };

const BYTE_ORDER_MARK = "\ufeff";

class Parser {
  #input = "";
  #cursor = 0;
  #options: {
    separator: string;
    trimLeadingSpace: boolean;
    comment?: string;
    lazyQuotes?: boolean;
    fieldsPerRecord?: number;
  };
  constructor({
    separator = ",",
    trimLeadingSpace = false,
    comment,
    lazyQuotes,
    fieldsPerRecord,
  }: ReadOptions = {}) {
    this.#options = {
      separator,
      trimLeadingSpace,
      comment,
      lazyQuotes,
      fieldsPerRecord,
    };
  }
  #readLine(): string | null {
    if (this.#isEOF()) return null;

    if (
      !this.#input.startsWith("\r\n", this.#cursor) ||
      !this.#input.startsWith("\n", this.#cursor)
    ) {
      let buffer = "";
      let hadNewline = false;
      while (this.#cursor < this.#input.length) {
        if (this.#input.startsWith("\r\n", this.#cursor)) {
          hadNewline = true;
          this.#cursor += 2;
          break;
        }
        if (
          this.#input.startsWith("\n", this.#cursor)
        ) {
          hadNewline = true;
          this.#cursor += 1;
          break;
        }
        buffer += this.#input[this.#cursor];
        this.#cursor += 1;
      }
      if (!hadNewline && buffer.endsWith("\r")) {
        buffer = buffer.slice(0, -1);
      }

      return buffer;
    }
    return null;
  }
  #isEOF(): boolean {
    return this.#cursor >= this.#input.length;
  }
  #parseRecord(startLine: number): string[] | null {
    let line = this.#readLine();
    if (line === null) return null;
    if (line.length === 0) {
      return [];
    }

    function runeCount(s: string): number {
      // Array.from considers the surrogate pair.
      return Array.from(s).length;
    }

    let lineIndex = startLine + 1;

    // line starting with comment character is ignored
    if (this.#options.comment && line[0] === this.#options.comment) {
      return [];
    }

    let fullLine = line;
    let quoteError: ParseError | null = null;
    const quote = '"';
    const quoteLen = quote.length;
    const separatorLen = this.#options.separator.length;
    let recordBuffer = "";
    const fieldIndexes = [] as number[];
    parseField:
    for (;;) {
      if (this.#options.trimLeadingSpace) {
        line = line.trimStart();
      }

      if (line.length === 0 || !line.startsWith(quote)) {
        // Non-quoted string field
        const i = line.indexOf(this.#options.separator);
        let field = line;
        if (i >= 0) {
          field = field.substring(0, i);
        }
        // Check to make sure a quote does not appear in field.
        if (!this.#options.lazyQuotes) {
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
            } else if (line.startsWith(this.#options.separator)) {
              // `","` sequence (end of field).
              line = line.substring(separatorLen);
              fieldIndexes.push(recordBuffer.length);
              continue parseField;
            } else if (0 === line.length) {
              // `"\n` sequence (end of line).
              fieldIndexes.push(recordBuffer.length);
              break parseField;
            } else if (this.#options.lazyQuotes) {
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
          } else if (line.length > 0 || !(this.#isEOF())) {
            // Hit end of line (copy all data so far).
            recordBuffer += line;
            const r = this.#readLine();
            lineIndex++;
            line = r ?? ""; // This is a workaround for making this module behave similarly to the encoding/csv/reader.go.
            fullLine = line;
            if (r === null) {
              // Abrupt end of file (EOF or error).
              if (!this.#options.lazyQuotes) {
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
            if (!this.#options.lazyQuotes) {
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
  parse(input: string): string[][] {
    this.#input = input.startsWith(BYTE_ORDER_MARK) ? input.slice(1) : input;
    this.#cursor = 0;
    const result: string[][] = [];
    let _nbFields: number | undefined;
    let lineResult: string[];
    let first = true;
    let lineIndex = 0;

    const INVALID_RUNE = ["\r", "\n", '"'];

    const options = this.#options;
    if (
      INVALID_RUNE.includes(options.separator) ||
      (typeof options.comment === "string" &&
        INVALID_RUNE.includes(options.comment)) ||
      options.separator === options.comment
    ) {
      throw new Error(ERR_INVALID_DELIM);
    }

    for (;;) {
      const r = this.#parseRecord(lineIndex);
      if (r === null) break;
      lineResult = r;
      lineIndex++;
      // If fieldsPerRecord is 0, Read sets it to
      // the number of fields in the first record
      if (first) {
        first = false;
        if (options.fieldsPerRecord !== undefined) {
          if (options.fieldsPerRecord === 0) {
            _nbFields = lineResult.length;
          } else {
            _nbFields = options.fieldsPerRecord;
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
}

export interface ParseOptions extends ReadOptions {
  /**
   * If you provide `skipFirstRow: true` and `columns`, the first line will be
   * skipped.
   * If you provide `skipFirstRow: true` but not `columns`, the first line will
   * be skipped and used as header definitions.
   */
  skipFirstRow?: boolean;

  /** List of names used for header definition. */
  columns?: readonly string[];
}

/**
 * Csv parse helper to manipulate data.
 * Provides an auto/custom mapper for columns.
 *
 * @example
 * ```ts
 * import { parse } from "https://deno.land/std@$STD_VERSION/csv/parse.ts";
 * const string = "a,b,c\nd,e,f";
 *
 * console.log(
 *   await parse(string, {
 *     skipFirstRow: false,
 *   }),
 * );
 * // output:
 * // [["a", "b", "c"], ["d", "e", "f"]]
 * ```
 *
 * @param input Input to parse.
 * @param opt options of the parser.
 * @returns If you don't provide `opt.skipFirstRow` and `opt.columns`, it returns `string[][]`.
 *   If you provide `opt.skipFirstRow` or `opt.columns`, it returns `Record<string, unknown>[]`.
 */
export function parse(input: string, opt?: undefined): string[][];
export function parse<const T extends ParseOptions>(
  input: string,
  opt: T,
): ParseResult<ParseOptions, T>;
export function parse<const T extends ParseOptions>(
  input: string,
  opt: T = { skipFirstRow: false } as T,
): ParseResult<ParseOptions, T> {
  const parser = new Parser(opt);
  const r = parser.parse(input);

  if (opt.skipFirstRow || opt.columns) {
    let headers: readonly string[] = [];

    if (opt.skipFirstRow) {
      const head = r.shift();
      assert(head !== undefined);
      headers = head;
    }

    if (opt.columns) {
      headers = opt.columns;
    }

    const firstLineIndex = opt.skipFirstRow ? 1 : 0;
    return r.map((row, i) => {
      return convertRowToObject(row, headers, firstLineIndex + i);
    }) as ParseResult<ParseOptions, T>;
  }
  return r as ParseResult<ParseOptions, T>;
}
