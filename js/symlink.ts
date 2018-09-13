// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";

/**
 * Synchronously creates newname as a symbolic link to oldname.
 *
 *     import { symlinkSync } from "deno";
 *     symlinkSync("old/name", "new/name");
 */
export function symlinkSync(oldname: string, newname: string): void {
  dispatch.sendSync(...req(oldname, newname));
}

/**
 * Creates newname as a symbolic link to oldname.
 *
 *     import { symlink } from "deno";
 *     await symlink("old/name", "new/name");
 */
export async function symlink(oldname: string, newname: string): Promise<void> {
  await dispatch.sendAsync(...req(oldname, newname));
}

function req(
  oldname: string,
  newname: string
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const oldname_ = builder.createString(oldname);
  const newname_ = builder.createString(newname);
  fbs.Symlink.startSymlink(builder);
  fbs.Symlink.addOldname(builder, oldname_);
  fbs.Symlink.addNewname(builder, newname_);
  const msg = fbs.Symlink.endSymlink(builder);
  return [builder, fbs.Any.Symlink, msg];
}
