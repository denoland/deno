// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

/** Changes the permission of a specific file/directory of specified path
 * synchronously.
 *
 *       import { chmodSync } from "deno";
 *       chmodSync("/path/to/file", 0o666);
 */
export function chmodSync(path: string, mode: number): void {
  dispatch.sendSync(...req(path, mode));
}

/** Changes the permission of a specific file/directory of specified path.
 *
 *       import { chmod } from "deno";
 *       await chmod("/path/to/file", 0o666);
 */
export async function chmod(path: string, mode: number): Promise<void> {
  await dispatch.sendAsync(...req(path, mode));
}

function req(
  path: string,
  mode: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  msg.Chmod.startChmod(builder);
  msg.Chmod.addPath(builder, path_);
  msg.Chmod.addMode(builder, mode);
  const inner = msg.Chmod.endChmod(builder);
  return [builder, msg.Any.Chmod, inner];
}
