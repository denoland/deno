// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

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
export function truncateSync(name: string, len?: number): void {
  sendSync("op_truncate", { name, len: coerceLen(len) });
}

/** Truncates or extends the specified file, to reach the specified `len`.
 *
 *       await Deno.truncate("hello.txt", 10);
 *
 * Requires `allow-write` permission. */
export async function truncate(name: string, len?: number): Promise<void> {
  await sendAsync("op_truncate", { name, len: coerceLen(len) });
}
