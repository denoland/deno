// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "deno_dispatch_json";
import { opNamespace } from "./namespace.ts";

const OP_CHOWN = new JsonOp(opNamespace, "chown");

/**
 * Change owner of a regular file or directory synchronously. Unix only at the moment.
 * @param path path to the file
 * @param uid user id of the new owner
 * @param gid group id of the new owner
 */
export function chownSync(path: string, uid: number, gid: number): void {
  OP_CHOWN.sendSync({ path, uid, gid });
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
  await OP_CHOWN.sendAsync({ path, uid, gid });
}
