// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

function toSecondsFromEpoch(v: number | Date): number {
  return v instanceof Date ? Math.trunc(v.valueOf() / 1000) : v;
}

export function utimeSync(
  path: string,
  atime: number | Date,
  mtime: number | Date
): void {
  core.dispatchJson.sendSync("op_utime", {
    path,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime),
  });
}

export async function utime(
  path: string,
  atime: number | Date,
  mtime: number | Date
): Promise<void> {
  await core.dispatchJson.sendAsync("op_utime", {
    path,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime),
  });
}
