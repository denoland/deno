// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";

/**
 * Truncates or extends the specified file synchronously,
 * updating the size of this file to become size.
 *
 *     import { truncateSync } from "deno";
 *
 *     truncateSync("hello.txt", 10);
 */
export function truncateSync(name: string, len?: number): void {
  dispatch.sendSync(...req(name, len));
}

/**
 * Truncates or extends the specified file,
 * updating the size of this file to become size.
 *
 *     import { truncate } from "deno";
 *
 *     await truncate("hello.txt", 10);
 */
export async function truncate(name: string, len?: number): Promise<void> {
  await dispatch.sendAsync(...req(name, len));
}

function req(
  name: string,
  len?: number
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const name_ = builder.createString(name);
  len = len && len > 0 ? Math.floor(len) : 0;
  fbs.Truncate.startTruncate(builder);
  fbs.Truncate.addName(builder, name_);
  fbs.Truncate.addLen(builder, len);
  const msg = fbs.Truncate.endTruncate(builder);
  return [builder, fbs.Any.Truncate, msg];
}
