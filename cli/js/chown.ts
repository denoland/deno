// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";

/** Synchronously change owner of a regular file or directory. Linux/Mac OS
 * only at the moment.
 *
 * Requires `allow-write` permission.
 *
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export function chownSync(path: string, uid: number, gid: number): void {
  sendSync("op_chown", { path, uid, gid });
}

/** Change owner of a regular file or directory. Linux/Mac OS only at the
 * moment.
 *
 * Requires `allow-write` permission.
 *
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export async function chown(
  path: string,
  uid: number,
  gid: number
): Promise<void> {
  await sendAsync("op_chown", { path, uid, gid });
}
