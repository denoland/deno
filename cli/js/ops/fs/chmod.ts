// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

/** Synchronously changes the permission of a specific file/directory of
 * specified path.  Ignores the process's umask.
 *
 *       Deno.chmodSync("/path/to/file", 0o666);
 *
 * Requires `allow-write` permission. */
export function chmodSync(path: string, mode: number): void {
  sendSync("op_chmod", { path, mode });
}

/** Changes the permission of a specific file/directory of specified path.
 * Ignores the process's umask.
 *
 *       await Deno.chmod("/path/to/file", 0o666);
 *
 * Requires `allow-write` permission. */
export async function chmod(path: string, mode: number): Promise<void> {
  await sendAsync("op_chmod", { path, mode });
}
