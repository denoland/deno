// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

/** Removes the named file or (empty) directory synchronously. Would throw
 * error if permission denied, not found, or directory not empty.
 *
 *       import { removeSync } from "deno";
 *       removeSync("/path/to/empty_dir/or/file");
 */
export function removeSync(path: string): void {
  dispatch.sendSync(...req(path, false));
}

/** Removes the named file or (empty) directory. Would throw error if
 * permission denied, not found, or directory not empty.
 *
 *       import { remove } from "deno";
 *       await remove("/path/to/empty_dir/or/file");
 */
export async function remove(path: string): Promise<void> {
  await dispatch.sendAsync(...req(path, false));
}

/** Recursively removes the named file or directory synchronously.  Would throw
 * error if permission denied or not found.
 *
 *       import { removeAllSync } from "deno";
 *       removeAllSync("/path/to/dir/or/file");
 */
export function removeAllSync(path: string): void {
  dispatch.sendSync(...req(path, true));
}

/** Recursively removes the named file or directory. Would throw error if
 * permission denied or not found.
 *
 *       import { removeAll } from "deno";
 *       await removeAll("/path/to/dir/or/file");
 */
export async function removeAll(path: string): Promise<void> {
  await dispatch.sendAsync(...req(path, true));
}

function req(
  path: string,
  recursive: boolean
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  msg.Remove.startRemove(builder);
  msg.Remove.addPath(builder, path_);
  msg.Remove.addRecursive(builder, recursive);
  const inner = msg.Remove.endRemove(builder);
  return [builder, msg.Any.Remove, inner];
}
