// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";

export interface TruncateOptions {
  /** Permissions to use if creating the file (defaults to `0o666`, before
   * the process's umask).
   * It's an error to specify mode when create is set to `false`.
   * Does nothing/raises on Windows. */
  mode?: number;
}

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

/** Synchronously truncates or extends the specified file, to reach the
 * specified `len`.
 *
 *       Deno.truncateSync("hello.txt", 10);
 *
 * Requires `allow-write` permission. */
export function truncateSync(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): void {
  const args = { path, len: coerceLen(len), mode: options.mode };
  sendSync("op_truncate", args);
}

/** Truncates or extends the specified file, to reach the specified `len`.
 *
 *       await Deno.truncate("hello.txt", 10);
 *
 * Requires `allow-write` permission. */
export async function truncate(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): Promise<void> {
  const args = { path, len: coerceLen(len), mode: options.mode };
  await sendAsync("op_truncate", args);
}
