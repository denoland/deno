// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

// TODO(ry) The complexity in argument parsing is to support deprecated forms of
// mkdir and mkdirSync.

export interface MkdirOption {
  recursive?: boolean;
  mode?: number;
}

/** Creates a new directory with the specified path synchronously.
 * If `recursive` is set to true, nested directories will be created (also known
 * as "mkdir -p").
 * `mode` sets permission bits (before umask) on UNIX and does nothing on
 * Windows.
 *
 *       Deno.mkdirSync("new_dir");
 *       Deno.mkdirSync("nested/directories", { recursive: true });
 */
export function mkdirSync(
  path: string,
  optionsOrRecursive?: MkdirOption | boolean,
  mode?: number
): void {
  const args = { path, recursive: false, mode: 0o777 };
  if (typeof optionsOrRecursive == "boolean") {
    args.recursive = optionsOrRecursive;
    args.mode = mode!;
  } else if (optionsOrRecursive) {
    args.recursive = optionsOrRecursive.recursive!;
    args.mode = optionsOrRecursive.mode!;
  }
  sendSync(dispatch.OP_MKDIR, args);
}

/** Creates a new directory with the specified path.
 * If `recursive` is set to true, nested directories will be created (also known
 * as "mkdir -p").
 * `mode` sets permission bits (before umask) on UNIX and does nothing on
 * Windows.
 *
 *       await Deno.mkdir("new_dir");
 *       await Deno.mkdir("nested/directories", { recursive: true });
 */
export async function mkdir(
  path: string,
  optionsOrRecursive?: MkdirOption | boolean,
  mode?: number
): Promise<void> {
  const args = { path, recursive: false, mode: 0o777 };
  if (typeof optionsOrRecursive == "boolean") {
    args.recursive = optionsOrRecursive;
    args.mode = mode!;
  } else if (optionsOrRecursive) {
    args.recursive = optionsOrRecursive.recursive!;
    args.mode = optionsOrRecursive.mode!;
  }
  await sendAsync(dispatch.OP_MKDIR, args);
}
