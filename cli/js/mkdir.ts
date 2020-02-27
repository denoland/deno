// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

// TODO(ry) The complexity in argument parsing is to support deprecated forms of
// mkdir and mkdirSync.
function mkdirArgs(
  path: string,
  optionsOrRecursive?: MkdirOption | boolean,
  mode?: number
): { path: string; recursive: boolean; mode: number } {
  const args = { path, recursive: false, mode: 0o777 };
  if (typeof optionsOrRecursive == "boolean") {
    args.recursive = optionsOrRecursive;
    if (mode) {
      args.mode = mode;
    }
  } else if (optionsOrRecursive) {
    if (typeof optionsOrRecursive.recursive == "boolean") {
      args.recursive = optionsOrRecursive.recursive;
    }
    if (optionsOrRecursive.mode) {
      args.mode = optionsOrRecursive.mode;
    }
  }
  return args;
}

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
  sendSync("op_mkdir", mkdirArgs(path, optionsOrRecursive, mode));
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
  await sendAsync("op_mkdir", mkdirArgs(path, optionsOrRecursive, mode));
}
