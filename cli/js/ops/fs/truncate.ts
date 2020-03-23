// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

export function truncateSync(path: string, len?: number): void {
  sendSync("op_truncate", { path, len: coerceLen(len) });
}

export async function truncate(path: string, len?: number): Promise<void> {
  await sendAsync("op_truncate", { path, len: coerceLen(len) });
}
