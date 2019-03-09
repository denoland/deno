// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";

function req(
  filename: string,
  data: Uint8Array,
  options: WriteFileOptions
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset, Uint8Array] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  msg.WriteFile.startWriteFile(builder);
  msg.WriteFile.addFilename(builder, filename_);
  // Perm is not updated by default
  if (options.perm !== undefined && options.perm !== null) {
    msg.WriteFile.addUpdatePerm(builder, true);
    msg.WriteFile.addPerm(builder, options.perm!);
  } else {
    msg.WriteFile.addUpdatePerm(builder, false);
    msg.WriteFile.addPerm(builder, 0o666);
  }
  // Create is turned on by default
  if (options.create !== undefined) {
    msg.WriteFile.addIsCreate(builder, !!options.create);
  } else {
    msg.WriteFile.addIsCreate(builder, true);
  }
  msg.WriteFile.addIsAppend(builder, !!options.append);
  const inner = msg.WriteFile.endWriteFile(builder);
  return [builder, msg.Any.WriteFile, inner, data];
}

/** Options for writing to a file.
 * `perm` would change the file's permission if set.
 * `create` decides if the file should be created if not exists (default: true)
 * `append` decides if the file should be appended (default: false)
 */
export interface WriteFileOptions {
  perm?: number;
  create?: boolean;
  append?: boolean;
}

/** Write a new file, with given filename and data synchronously.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       Deno.writeFileSync("hello.txt", data);
 */
export function writeFileSync(
  filename: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): void {
  dispatch.sendSync(...req(filename, data, options));
}

/** Write a new file, with given filename and data.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       await Deno.writeFile("hello.txt", data);
 */
export async function writeFile(
  filename: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): Promise<void> {
  await dispatch.sendAsync(...req(filename, data, options));
}
