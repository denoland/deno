// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";

/**
 * Write a new file, with given filename and data synchronously.
 *
 *     import { writeFileSync } from "deno";
 *
 *     const encoder = new TextEncoder("utf-8");
 *     const data = encoder.encode("Hello world\n");
 *     writeFileSync("hello.txt", data);
 */
export function writeFileSync(
  filename: string,
  data: Uint8Array,
  perm = 0o666
): void {
  dispatch.sendSync(...req(filename, data, perm));
}

/**
 * Write a new file, with given filename and data.
 *
 *     import { writeFile } from "deno";
 *
 *     const encoder = new TextEncoder("utf-8");
 *     const data = encoder.encode("Hello world\n");
 *     await writeFile("hello.txt", data);
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
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset, Uint8Array] {
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.WriteFile.startWriteFile(builder);
  fbs.WriteFile.addFilename(builder, filename_);
  fbs.WriteFile.addPerm(builder, perm);
  const inner = fbs.WriteFile.endWriteFile(builder);
  return [builder, fbs.Any.WriteFile, inner, data];
}
