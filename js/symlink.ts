// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import * as util from "./util";

function req(
  oldname: string,
  newname: string,
  type?: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  // TODO Use type for Windows.
  if (type) {
    return util.notImplemented();
  }
  const builder = flatbuffers.createBuilder();
  const oldname_ = builder.createString(oldname);
  const newname_ = builder.createString(newname);
  msg.Symlink.startSymlink(builder);
  msg.Symlink.addOldname(builder, oldname_);
  msg.Symlink.addNewname(builder, newname_);
  const inner = msg.Symlink.endSymlink(builder);
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
  dispatch.sendSync(...req(oldname, newname, type));
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
  await dispatch.sendAsync(...req(oldname, newname, type));
}
