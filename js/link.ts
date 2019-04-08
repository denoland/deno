// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

function req(
  oldname: string,
  newname: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const oldname_ = builder.createString(oldname);
  const newname_ = builder.createString(newname);
  const inner = msg.Link.createLink(builder, oldname_, newname_);
  return [builder, msg.Any.Link, inner];
}

/** Synchronously creates `newname` as a hard link to `oldname`.
 *
 *       Deno.linkSync("old/name", "new/name");
 */
export function linkSync(oldname: string, newname: string): void {
  dispatch.sendSync(...req(oldname, newname));
}

/** Creates `newname` as a hard link to `oldname`.
 *
 *       await Deno.link("old/name", "new/name");
 */
export async function link(oldname: string, newname: string): Promise<void> {
  await dispatch.sendAsync(...req(oldname, newname));
}
