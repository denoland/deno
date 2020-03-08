// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";

function toSecondsFromEpoch(v: number | Date): number {
  return v instanceof Date ? v.valueOf() / 1000 : v;
}

/** **UNSTABLE**: needs investigation into high precision time.
 *
 * Synchronously changes the access and modification times of a file system
 * object referenced by `path`. Given times are either in seconds (UNIX epoch
 * time) or as `Date` objects.
 *
 *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
 *
 * Requires `allow-write` permission. */
export function utimeSync(
  path: string,
  atime: number | Date,
  mtime: number | Date
): void {
  sendSync("op_utime", {
    path,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime)
  });
}

/** **UNSTABLE**: needs investigation into high precision time.
 *
 * Changes the access and modification times of a file system object
 * referenced by `path`. Given times are either in seconds (UNIX epoch time)
 * or as `Date` objects.
 *
 *       await Deno.utime("myfile.txt", 1556495550, new Date());
 *
 * Requires `allow-write` permission. */
export async function utime(
  path: string,
  atime: number | Date,
  mtime: number | Date
): Promise<void> {
  await sendAsync("op_utime", {
    path,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime)
  });
}
