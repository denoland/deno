// Ported from Go:
// https://github.com/golang/go/blob/go1.12.5/src/encoding/csv/
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { StringReader } from "../io/readers.ts";
import { assert } from "../_util/assert.ts";

const INVALID_RUNE = ["\r", "\n", '"'];

export const ERR_BARE_QUOTE = 'bare " in non-quoted-field';
export const ERR_QUOTE = 'extraneous or missing " in quoted-field';
export const ERR_INVALID_DELIM = "Invalid Delimiter";
export const ERR_FIELD_COUNT = "wrong number of fields";

export class ParseError extends Error {
  StartLine: number;
  Line: number;
  constructor(start: number, line: number, message: string) {
    super(message);
    this.StartLine = start;
    this.Line = line;
  }
}

/**
 * @property comma - Character which separates values. Default: ','
 * @property comment - Character to start a comment. Default: '#'
 * @property trimLeadingSpace - Flag to trim the leading space of the value.
 *           Default: 'false'
 * @property lazyQuotes - Allow unquoted quote in a quoted field or non double
 *           quoted quotes in quoted field. Default: 'false'
 * @property fieldsPerRecord - Enabling the check of fields for each row.
 *           If == 0, first row is used as referral for the number of fields.
 */
export interface ReadOptions {
  comma?: string;
  comment?: string;
  trimLeadingSpace?: boolean;
  lazyQuotes?: boolean;
  fieldsPerRecord?: number;
}

function chkOptions(opt: ReadOptions): void {
  if (!opt.comma) {
    opt.comma = ",";
  }
  if (!opt.trimLeadingSpace) {
    opt.trimLeadingSpace = false;
  }
  if (
    INVALID_RUNE.includes(opt.comma) ||
    (typeof opt.comment === "string" && INVALID_RUNE.includes(opt.comment)) ||
    opt.comma === opt.comment
  ) {
    throw new Error(ERR_INVALID_DELIM);
  }
}

async function readRecord(
  Startline: number,
  reader: BufReader,
  opt: ReadOptions = { comma: ",", trimLeadingSpace: false }
): Promise<string[] | null> {
  const tp = new TextProtoReader(reader);
  const lineIndex = Startline;
  let line = await readLine(tp);

  if (line === null) return null;
  if (line.length === 0) {
    return [];
  }
  // line starting with comment character is ignored
  if (opt.comment && line[0] === opt.comment) {
    return [];
  }

  assert(opt.comma != null);

  let quoteError: string | null = null;
  const quote = '"';
  const quoteLen = quote.length;
  const commaLen = opt.comma.length;
  let recordBuffer = "";
  const fieldIndexes = [] as number[];
  parseField: for (;;) {
    if (opt.trimLeadingSpace) {
      line = line.trimLeft();
    }

    if (line.length === 0 || !line.startsWith(quote)) {
      // Non-quoted string field
      const i = line.indexOf(opt.comma);
      let field = line;
      if (i >= 0) {
        field = field.substring(0, i);
      }
      // Check to make sure a quote does not appear in field.
      if (!opt.lazyQuotes) {
        const j = field.indexOf(quote);
        if (j >= 0) {
          quoteError = ERR_BARE_QUOTE;
          break parseField;
        }
      }
      recordBuffer += field;
      fieldIndexes.push(recordBuffer.length);
      if (i >= 0) {
        line = line.substring(i + commaLen);
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
          } else if (line.startsWith(opt.comma)) {
            // `","` sequence (end of field).
            line = line.substring(commaLen);
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
            quoteError = ERR_QUOTE;
            break parseField;
          }
        } else if (line.length > 0 || !(await isEOF(tp))) {
          // Hit end of line (copy all data so far).
          recordBuffer += line;
          const r = await readLine(tp);
          if (r === null) {
            if (!opt.lazyQuotes) {
              quoteError = ERR_QUOTE;
              break parseField;
            }
            fieldIndexes.push(recordBuffer.length);
            break parseField;
          }
          recordBuffer += "\n"; // preserve line feed (This is because TextProtoReader removes it.)
          line = r;
        } else {
          // Abrupt end of file (EOF on error).
          if (!opt.lazyQuotes) {
            quoteError = ERR_QUOTE;
            break parseField;
          }
          fieldIndexes.push(recordBuffer.length);
          break parseField;
        }
      }
    }
  }
  if (quoteError) {
    throw new ParseError(Startline, lineIndex, quoteError);
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
    comma: ",",
    trimLeadingSpace: false,
    lazyQuotes: false,
  }
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
        throw new ParseError(lineIndex, lineIndex, ERR_FIELD_COUNT);
      }
      result.push(lineResult);
    }
  }
  return result;
}

/**
 * Parse the CSV string/buffer with the options provided.
 *
 * HeaderOptions provides the column definition
 * and the parse function for each entry of the
 * column.
 */
export interface HeaderOptions {
  /**
   * Name of the header to be used as property
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
   * If a boolean is provided, the first line will be used as Header definitions.
   * If `string[]` or `HeaderOptions[]` those names will be used for header definition.
   */
  header: boolean | string[] | HeaderOptions[];
  /** Parse function for rows.
   * Example:
   *     const r = await parseFile('a,b,c\ne,f,g\n', {
   *      header: ["this", "is", "sparta"],
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
 * @returns If you don't provide both `opt.header` and `opt.parse`, it returns `string[][]`.
 *   If you provide `opt.header` but not `opt.parse`, it returns `object[]`.
 *   If you provide `opt.parse`, it returns an array where each element is the value returned from `opt.parse`.
 */
export async function parse(
  input: string | BufReader,
  opt: ParseOptions = {
    header: false,
  }
): Promise<unknown[]> {
  let r: string[][];
  if (input instanceof BufReader) {
    r = await readMatrix(input, opt);
  } else {
    r = await readMatrix(new BufReader(new StringReader(input)), opt);
  }
  if (opt.header) {
    let headers: HeaderOptions[] = [];
    let i = 0;
    if (Array.isArray(opt.header)) {
      if (typeof opt.header[0] !== "string") {
        headers = opt.header as HeaderOptions[];
      } else {
        const h = opt.header as string[];
        headers = h.map(
          (e): HeaderOptions => {
            return {
              name: e,
            };
          }
        );
      }
    } else {
      const head = r.shift();
      assert(head != null);
      headers = head.map(
        (e): HeaderOptions => {
          return {
            name: e,
          };
        }
      );
      i++;
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
