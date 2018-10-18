// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

/** Write a new file, with given filename and data synchronously.
 *
 *       import { writeFileSync } from "deno";
 *
 *       const encoder = new TextEncoder("utf-8");
 *       const data = encoder.encode("Hello world\n");
 *       writeFileSync("hello.txt", data);
 */
export function writeFileSync(
  filename: string,
  data: Uint8Array,
  perm = 0o666
): void {
  dispatch.sendSync(...req(filename, data, perm));
}

/** Write a new file, with given filename and data.
 *
 *       import { writeFile } from "deno";
 *
 *       const encoder = new TextEncoder("utf-8");
 *       const data = encoder.encode("Hello world\n");
 *       await writeFile("hello.txt", data);
 */
export async function writeFile(
  filename: string,
  data: Uint8Array,
  perm = 0o666
): Promise<void> {
  await dispatch.sendAsync(...req(filename, data, perm));
}

function req(
  filename: string,
  data: Uint8Array,
  perm: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, Uint8Array] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  msg.WriteFile.startWriteFile(builder);
  msg.WriteFile.addFilename(builder, filename_);
  msg.WriteFile.addPerm(builder, perm);
  const inner = msg.WriteFile.endWriteFile(builder);
  return [builder, msg.Any.WriteFile, inner, data];
}
