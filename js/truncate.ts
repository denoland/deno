// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync, msg, flatbuffers } from "./dispatch_flatbuffers";

function req(
  name: string,
  len?: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const name_ = builder.createString(name);
  len = len && len > 0 ? Math.floor(len) : 0;
  const inner = msg.Truncate.createTruncate(builder, name_, len);
  return [builder, msg.Any.Truncate, inner];
}

/** Truncates or extends the specified file synchronously, updating the size of
 * this file to become size.
 *
 *       Deno.truncateSync("hello.txt", 10);
 */
export function truncateSync(name: string, len?: number): void {
  sendSync(...req(name, len));
}

/**
 * Truncates or extends the specified file, updating the size of this file to
 * become size.
 *
 *       await Deno.truncate("hello.txt", 10);
 */
export async function truncate(name: string, len?: number): Promise<void> {
  await sendAsync(...req(name, len));
}
