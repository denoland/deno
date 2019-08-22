// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync, msg, flatbuffers } from "./dispatch_flatbuffers";
import * as util from "./util";
import { platform } from "./build";

function req(
  oldname: string,
  newname: string,
  type?: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  if (platform.os === "win" && type) {
    return util.notImplemented();
  }
  const builder = flatbuffers.createBuilder();
  const oldname_ = builder.createString(oldname);
  const newname_ = builder.createString(newname);
  const inner = msg.Symlink.createSymlink(builder, oldname_, newname_);
  return [builder, msg.Any.Symlink, inner];
}

/** Synchronously creates `newname` as a symbolic link to `oldname`. The type
 * argument can be set to `dir` or `file` and is only available on Windows
 * (ignored on other platforms).
 *
 *       Deno.symlinkSync("old/name", "new/name");
 */
export function symlinkSync(
  oldname: string,
  newname: string,
  type?: string
): void {
  sendSync(...req(oldname, newname, type));
}

/** Creates `newname` as a symbolic link to `oldname`. The type argument can be
 * set to `dir` or `file` and is only available on Windows (ignored on other
 * platforms).
 *
 *       await Deno.symlink("old/name", "new/name");
 */
export async function symlink(
  oldname: string,
  newname: string,
  type?: string
): Promise<void> {
  await sendAsync(...req(oldname, newname, type));
}
