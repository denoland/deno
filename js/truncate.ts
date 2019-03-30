// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

function req(
  name: string,
  len?: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  len = len && len > 0 ? Math.floor(len) : 0;
  msg.Truncate.startTruncate(builder);
  msg.Truncate.addName(builder, name_);
  msg.Truncate.addLen(builder, len);
  const inner = msg.Truncate.endTruncate(builder);
  return [builder, msg.Any.Truncate, inner];
}

/** Truncates or extends the specified file synchronously, updating the size of
 * this file to become size.
 *
 *       Deno.truncateSync("hello.txt", 10);
 */
export function truncateSync(name: string, len?: number): void {
  dispatch.sendSync(...req(name, len));
}

/**
 * Truncates or extends the specified file, updating the size of this file to
 * become size.
 *
 *       await Deno.truncate("hello.txt", 10);
 */
export async function truncate(name: string, len?: number): Promise<void> {
  await dispatch.sendAsync(...req(name, len));
}
