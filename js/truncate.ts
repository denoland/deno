// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

const OP_TRUNCATE = new JsonOp("truncate");

/** Truncates or extends the specified file synchronously, updating the size of
 * this file to become size.
 *
 *       Deno.truncateSync("hello.txt", 10);
 */
export function truncateSync(name: string, len?: number): void {
  OP_TRUNCATE.sendSync({ name, len: coerceLen(len) });
}

/**
 * Truncates or extends the specified file, updating the size of this file to
 * become size.
 *
 *       await Deno.truncate("hello.txt", 10);
 */
export async function truncate(name: string, len?: number): Promise<void> {
  await OP_TRUNCATE.sendAsync({ name, len: coerceLen(len) });
}
