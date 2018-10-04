// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";

/**
 * Copies the contents of a file to another by name synchronously.
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 * It would also copy the permission of the original file
 * to the destination.
 *
 *     import { copyFileSync } from "deno";
 *     copyFileSync("from.txt", "to.txt");
 */
export function copyFileSync(from: string, to: string): void {
  dispatch.sendSync(...req(from, to));
}

/**
 * Copies the contents of a file to another by name.
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 * It would also copy the permission of the original file
 * to the destination.
 *
 *     import { copyFile } from "deno";
 *     await copyFile("from.txt", "to.txt");
 */
export async function copyFile(from: string, to: string): Promise<void> {
  await dispatch.sendAsync(...req(from, to));
}

function req(
  from: string,
  to: string
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const from_ = builder.createString(from);
  const to_ = builder.createString(to);
  fbs.CopyFile.startCopyFile(builder);
  fbs.CopyFile.addFrom(builder, from_);
  fbs.CopyFile.addTo(builder, to_);
  const inner = fbs.CopyFile.endCopyFile(builder);
  return [builder, fbs.Any.CopyFile, inner];
}
