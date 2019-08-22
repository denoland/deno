// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync, msg, flatbuffers } from "./dispatch_flatbuffers";
import * as util from "./util";

function req(
  filename: string,
  atime: number | Date,
  mtime: number | Date
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const atimeSec = atime instanceof Date ? Math.floor(+atime / 1000) : atime;
  const mtimeSec = mtime instanceof Date ? Math.floor(+mtime / 1000) : mtime;

  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const atimeParts = util.splitNumberToParts(atimeSec);
  const atimeMS_ = builder.createLong(atimeParts[0], atimeParts[1]);
  const mtimeParts = util.splitNumberToParts(mtimeSec);
  const mtimeMS_ = builder.createLong(mtimeParts[0], mtimeParts[1]);

  const inner = msg.Utime.createUtime(builder, filename_, atimeMS_, mtimeMS_);
  return [builder, msg.Any.Utime, inner];
}

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
  sendSync(...req(filename, atime, mtime));
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
  await sendAsync(...req(filename, atime, mtime));
}
