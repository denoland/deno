// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "./diagnostics.ts";
import { sendSync } from "./dispatch_json.ts";

/**
 * Format an array of diagnostic items and return them as a single string.
 * @param items An array of diagnostic items to format
 */
export function formatDiagnostics(items: DiagnosticItem[]): string {
  return sendSync("op_format_diagnostic", { items });
}
