// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

export function ftruncateSync(rid: number, len?: number): void {
  core.dispatchJson.sendSync("op_ftruncate", { rid, len: coerceLen(len) });
}

export async function ftruncate(rid: number, len?: number): Promise<void> {
  await core.dispatchJson.sendAsync("op_ftruncate", {
    rid,
    len: coerceLen(len),
  });
}

export function truncateSync(path: string, len?: number): void {
  core.dispatchJson.sendSync("op_truncate", { path, len: coerceLen(len) });
}

export async function truncate(path: string, len?: number): Promise<void> {
  await core.dispatchJson.sendAsync("op_truncate", {
    path,
    len: coerceLen(len),
  });
}
