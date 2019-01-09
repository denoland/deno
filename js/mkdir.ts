// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

/** Creates a new directory with the specified path and permission
 * synchronously.
 *
 *       import { mkdirSync } from "deno";
 *       mkdirSync("new_dir");
 */
export function mkdirSync(path: string, mode = 0o777): void {
  dispatch.sendSync(...req(path, mode));
}

/** Creates a new directory with the specified path and permission.
 *
 *       import { mkdir } from "deno";
 *       await mkdir("new_dir");
 */
export async function mkdir(path: string, mode = 0o777): Promise<void> {
  await dispatch.sendAsync(...req(path, mode));
}

function req(
  path: string,
  mode: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  msg.Mkdir.startMkdir(builder);
  msg.Mkdir.addPath(builder, path_);
  msg.Mkdir.addMode(builder, mode);
  const inner = msg.Mkdir.endMkdir(builder);
  return [builder, msg.Any.Mkdir, inner];
}
