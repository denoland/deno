// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "../diagnostics.ts";
import { sendSync } from "./dispatch_json.ts";

/**
 * Format an array of diagnostic items and return them as a single string.
 * @param items An array of diagnostic items to format
 */
export function formatDiagnostics(items: DiagnosticItem[]): string {
  return sendSync("op_format_diagnostic", { items });
}

export interface Location {
  /** The full url for the module, e.g. `file://some/file.ts` or
   * `https://some/file.ts`. */
  filename: string;

  /** The line number in the file.  It is assumed to be 1-indexed. */
  line: number;

  /** The column number in the file.  It is assumed to be 1-indexed. */
  column: number;
}

/** Given a current location in a module, lookup the source location and
 * return it.
 *
 * When Deno transpiles code, it keep source maps of the transpiled code.  This
 * function can be used to lookup the original location.  This is automatically
 * done when accessing the `.stack` of an error, or when an uncaught error is
 * logged.  This function can be used to perform the lookup for creating better
 * error handling.
 *
 * **Note:** `line` and `column` are 1 indexed, which matches display
 * expectations, but is not typical of most index numbers in Deno.
 *
 * An example:
 *
 *       const orig = Deno.applySourceMap({
 *         location: "file://my/module.ts",
 *         line: 5,
 *         column: 15
 *       });
 *       console.log(`${orig.filename}:${orig.line}:${orig.column}`);
 *
 */
export function applySourceMap(location: Location): Location {
  const { filename, line, column } = location;
  // On this side, line/column are 1 based, but in the source maps, they are
  // 0 based, so we have to convert back and forth
  const res = sendSync("op_apply_source_map", {
    filename,
    line: line - 1,
    column: column - 1
  });
  return {
    filename: res.filename,
    line: res.line + 1,
    column: res.column + 1
  };
}
