// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

export interface RemoveOption {
  recursive?: boolean;
}

const OP_REMOVE = new JsonOp("remove");

/** Removes the named file or directory synchronously. Would throw
 * error if permission denied, not found, or directory not empty if `recursive`
 * set to false.
 * `recursive` is set to false by default.
 *
 *       Deno.removeSync("/path/to/dir/or/file", {recursive: false});
 */
export function removeSync(path: string, options: RemoveOption = {}): void {
  OP_REMOVE.sendSync({ path, recursive: !!options.recursive });
}

/** Removes the named file or directory. Would throw error if
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
  await OP_REMOVE.sendAsync({ path, recursive: !!options.recursive });
}
