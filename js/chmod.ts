// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

const OP_CHMOD = new JsonOp("chmod");

/** Changes the permission of a specific file/directory of specified path
 * synchronously.
 *
 *       Deno.chmodSync("/path/to/file", 0o666);
 */
export function chmodSync(path: string, mode: number): void {
  OP_CHMOD.sendSync({ path, mode });
}

/** Changes the permission of a specific file/directory of specified path.
 *
 *       await Deno.chmod("/path/to/file", 0o666);
 */
export async function chmod(path: string, mode: number): Promise<void> {
  await OP_CHMOD.sendAsync({ path, mode });
}
