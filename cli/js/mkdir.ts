// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

type MkdirArgs = { path: string; recursive: boolean; perm?: number };

// TODO(ry) The complexity in argument parsing is to support deprecated forms of
// mkdir and mkdirSync.
function mkdirArgs(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  perm?: number
): MkdirArgs {
  const args: MkdirArgs = { path, recursive: false };
  if (typeof optionsOrRecursive == "boolean") {
    args.recursive = optionsOrRecursive;
    if (perm) {
      args.perm = perm;
    }
  } else if (optionsOrRecursive) {
    if (typeof optionsOrRecursive.recursive == "boolean") {
      args.recursive = optionsOrRecursive.recursive;
    }
    if (optionsOrRecursive.perm) {
      args.perm = optionsOrRecursive.perm;
    }
  }
  return args;
}

export interface MkdirOptions {
  /** Defaults to `false`. If set to `true`, means that any intermediate
   * directories will also be created (as with the shell command `mkdir -p`).
   * Intermediate directories are created with the same permissions.
   * When recursive is set to `true`, succeeds silently (without changing any
   * permissions) if a directory already exists at the path. */
  recursive?: boolean;
  /** Permissions to use when creating the directory (defaults to `0o777`,
   * before the process's umask).
   * Does nothing/raises on Windows. */
  perm?: number;
}

/** Synchronously creates a new directory with the specified path.
 *
 *       Deno.mkdirSync("new_dir");
 *       Deno.mkdirSync("nested/directories", { recursive: true });
 *
 * Requires `allow-write` permission. */
export function mkdirSync(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  perm?: number
): void {
  sendSync("op_mkdir", mkdirArgs(path, optionsOrRecursive, perm));
}

/** Creates a new directory with the specified path.
 *
 *       await Deno.mkdir("new_dir");
 *       await Deno.mkdir("nested/directories", { recursive: true });
 *
 * Requires `allow-write` permission. */
export async function mkdir(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  perm?: number
): Promise<void> {
  await sendAsync("op_mkdir", mkdirArgs(path, optionsOrRecursive, perm));
}
