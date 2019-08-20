// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync, msg, flatbuffers } from "./dispatch_flatbuffers";

export interface RemoveOption {
  recursive?: boolean;
}

function req(
  path: string,
  options: RemoveOption
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  const inner = msg.Remove.createRemove(builder, path_, !!options.recursive);
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
  sendSync(...req(path, options));
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
  await sendAsync(...req(path, options));
}
