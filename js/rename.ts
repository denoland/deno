// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";

/**
 * Synchronously renames (moves) oldpath to newpath. If newpath already exists 
 * and is not a directory, Rename replaces it. OS-specific restrictions may 
 * apply when oldpath and newpath are in different directories.
 *
 *     import { renameSync } from "deno";
 *     renameSync("old/path", "new/path");
 */
export function renameSync(oldpath: string, newpath: string): void {
  dispatch.sendSync(...req(oldpath, newpath));
}

/**
 * Renames (moves) oldpath to newpath. If newpath already exists 
 * and is not a directory, Rename replaces it. OS-specific restrictions may 
 * apply when oldpath and newpath are in different directories.
 *
 *     import { rename } from "deno";
 *     await rename("old/path", "new/path");
 */
export async function rename(oldpath: string, newpath: string): Promise<void> {
  await dispatch.sendAsync(...req(oldpath, newpath));
}

function req(
  oldpath: string,
  newpath: string
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const oldpath_ = builder.createString(oldpath);
  const newpath_ = builder.createString(newpath);
  fbs.Rename.startRename(builder);
  fbs.Rename.addOldpath(builder, oldpath_);
  fbs.Rename.addNewpath(builder, newpath_);
  const msg = fbs.Rename.endRename(builder);
  return [builder, fbs.Any.Rename, msg];
}
