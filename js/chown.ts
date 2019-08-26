// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json";
import * as dispatch from "./dispatch";

/**
 * Change owner of a regular file or directory synchronously. Unix only at the moment.
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export function chownSync(path: string, uid: number, gid: number): void {
  sendSync(dispatch.OP_CHOWN, { path, uid, gid });
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
  await sendAsync(dispatch.OP_CHOWN, { path, uid, gid });
}
