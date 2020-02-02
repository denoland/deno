// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

export interface RemoveOption {
  recursive?: boolean;
}

/** Removes the named file, directory or symlink synchronously. Would throw
 * error if permission denied, not found, or directory not empty if `recursive`
 * set to false.
 * `recursive` is set to false by default.
 *
 *       Deno.removeSync("/path/to/dir/or/file", {recursive: false});
 */
export function removeSync(path: string, options: RemoveOption = {}): void {
  sendSync(dispatch.OP_REMOVE, { path, recursive: !!options.recursive });
}

/** Removes the named file, directory or symlink. Would throw error if
 * permission denied, not found, or directory not empty if `recursive` set
 * to false.
 * `recursive` is set to false by default.
 *
 *       await Deno.remove("/path/to/dir/or/file", {recursive: false});
 */
export async function remove(
  path: string,
  options: RemoveOption = {}
): Promise<void> {
  await sendAsync(dispatch.OP_REMOVE, { path, recursive: !!options.recursive });
}
