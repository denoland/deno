// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

export interface RemoveOption {
  recursive?: boolean;
}

function req(
  path: string,
  options: RemoveOption
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  msg.Remove.startRemove(builder);
  msg.Remove.addPath(builder, path_);
  msg.Remove.addRecursive(builder, !!options.recursive);
  const inner = msg.Remove.endRemove(builder);
  return [builder, msg.Any.Remove, inner];
}

/** Removes the named file or directory synchronously. Would throw
 * error if permission denied, not found, or directory not empty if `recursive`
 * set to false.
 * `recursive` is set to false by default.
 *
 *       Deno.removeSync("/path/to/dir/or/file", {recursive: false});
 */
export function removeSync(path: string, options: RemoveOption = {}): void {
  dispatch.sendSync(...req(path, options));
}

/** Removes the named file or directory. Would throw error if
 * permission denied, not found, or directory not empty if `recursive` set
 * to false.
 * `recursive` is set to false by default.
 *
 *       await Deno.remove("/path/to/dir/or/file", {recursive: false});
 */
export async function remove(
  path: string,
  options: RemoveOption = {}
): Promise<void> {
  await dispatch.sendAsync(...req(path, options));
}
