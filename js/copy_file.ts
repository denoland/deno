// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

/** Copies the contents of a file to another by name synchronously.
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 *
 * It would also copy the permission of the original file
 * to the destination.
 *
 *       import { copyFileSync } from "deno";
 *       copyFileSync("from.txt", "to.txt");
 */
export function copyFileSync(from: string, to: string): void {
  dispatch.sendSync(...req(from, to));
}

/** Copies the contents of a file to another by name.
 *
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 *
 * It would also copy the permission of the original file
 * to the destination.
 *
 *       import { copyFile } from "deno";
 *       await copyFile("from.txt", "to.txt");
 */
export async function copyFile(from: string, to: string): Promise<void> {
  await dispatch.sendAsync(...req(from, to));
}

function req(
  from: string,
  to: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const from_ = builder.createString(from);
  const to_ = builder.createString(to);
  msg.CopyFile.startCopyFile(builder);
  msg.CopyFile.addFrom(builder, from_);
  msg.CopyFile.addTo(builder, to_);
  const inner = msg.CopyFile.endCopyFile(builder);
  return [builder, msg.Any.CopyFile, inner];
}
