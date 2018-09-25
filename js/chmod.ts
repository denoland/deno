import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";
/**
 * Synchronously changes the file permissions.
 *
 *     import { chmodSync } from "deno";
 *     chmodSync(path, mode);
 */
export function chmodSync(path: string, mode: number): void {
  dispatch.sendSync(...req(path, mode));
}
function req(
  path: string,
  mode: number
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const path_ = builder.createString(path);
  fbs.Chmod.startChmod(builder);
  fbs.Chmod.addPath(builder, path_);
  fbs.Chmod.addMode(builder, mode);
  const msg = fbs.Chmod.endChmod(builder);
  return [builder, fbs.Any.Chmod, msg];
}
