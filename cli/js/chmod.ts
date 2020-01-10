// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

/** Changes the permission of a specific file/directory of specified path
 * synchronously.
 *
 *       Deno.chmodSync("/path/to/file", 0o666);
 */
export function chmodSync(path: string, mode: number): void {
  sendSync(dispatch.OP_CHMOD, { path, mode });
}

/** Changes the permission of a specific file/directory of specified path.
 *
 *       await Deno.chmod("/path/to/file", 0o666);
 */
export async function chmod(path: string, mode: number): Promise<void> {
  await sendAsync(dispatch.OP_CHMOD, { path, mode });
}
