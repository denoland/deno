// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "./diagnostics.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";

// TODO(bartlomieju): move to `repl.ts`?
export function formatError(errString: string): string {
  const res = sendSync(dispatch.OP_FORMAT_ERROR, { error: errString });
  return res.error;
}

/**
 * Format an array of diagnostic items and return them as a single string.
 * @param items An array of diagnostic items to format
 */
export function formatDiagnostics(items: DiagnosticItem[]): string {
  return sendSync(dispatch.OP_FORMAT_DIAGNOSTIC, { items });
}
