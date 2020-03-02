// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface RemoveOptions {
  /** Defaults to `false`. If set to `true`, path will be removed even if
   * it's a non-empty directory. */
  recursive?: boolean;
}

/** Synchronously removes the named file or directory. Throws error if
 * permission denied, path not found, or path is a non-empty directory and
 * the `recursive` option isn't set to `true`.
 *
 *       Deno.removeSync("/path/to/dir/or/file", { recursive: false });
 *
 * Requires `allow-write` permission. */
export function removeSync(path: string, options: RemoveOptions = {}): void {
  sendSync("op_remove", { path, recursive: !!options.recursive });
}

/** Removes the named file or directory. Throws error if permission denied,
 * path not found, or path is a non-empty directory and the `recursive`
 * option isn't set to `true`.
 *
 *       await Deno.remove("/path/to/dir/or/file", { recursive: false });
 *
 * Requires `allow-write` permission. */
export async function remove(
  path: string,
  options: RemoveOptions = {}
): Promise<void> {
  await sendAsync("op_remove", { path, recursive: !!options.recursive });
}
