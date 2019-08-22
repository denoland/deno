// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync, msg, flatbuffers } from "./dispatch_flatbuffers";

function req(
  path: string,
  uid: number,
  gid: number
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  const inner = msg.Chown.createChown(builder, path_, uid, gid);
  return [builder, msg.Any.Chown, inner];
}

/**
 * Change owner of a regular file or directory synchronously. Unix only at the moment.
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export function chownSync(path: string, uid: number, gid: number): void {
  sendSync(...req(path, uid, gid));
}

/**
 * Change owner of a regular file or directory asynchronously. Unix only at the moment.
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export async function chown(
  path: string,
  uid: number,
  gid: number
): Promise<void> {
  await sendAsync(...req(path, uid, gid));
}
