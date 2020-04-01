// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

function toSecondsFromEpoch(v: number | Date): number {
  return v instanceof Date ? v.valueOf() / 1000 : v;
}

export function utimeSync(
  path: string,
  atime: number | Date,
  mtime: number | Date
): void {
  sendSync("op_utime", {
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
  await sendAsync("op_utime", {
    path,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime),
  });
}
