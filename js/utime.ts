// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

function toSecondsFromEpoch(v: number | Date): number {
  return v instanceof Date ? v.valueOf() / 1000 : v;
}

const OP_UTIME = new JsonOp("utime");

/** Synchronously changes the access and modification times of a file system
 * object referenced by `filename`. Given times are either in seconds
 * (Unix epoch time) or as `Date` objects.
 *
 *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
 */
export function utimeSync(
  filename: string,
  atime: number | Date,
  mtime: number | Date
): void {
  OP_UTIME.sendSync({
    filename,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime)
  });
}

/** Changes the access and modification times of a file system object
 * referenced by `filename`. Given times are either in seconds
 * (Unix epoch time) or as `Date` objects.
 *
 *       await Deno.utime("myfile.txt", 1556495550, new Date());
 */
export async function utime(
  filename: string,
  atime: number | Date,
  mtime: number | Date
): Promise<void> {
  await OP_UTIME.sendAsync({
    filename,
    // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
    atime: toSecondsFromEpoch(atime),
    mtime: toSecondsFromEpoch(mtime)
  });
}
