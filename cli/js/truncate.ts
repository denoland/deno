// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface TruncateOptions {
  /** Defaults to `false`. If set to `true`, no file, directory, or symlink is
   * allowed to exist at the target location. When createNew is set to `true`,
   * create is ignored. */
  createNew?: boolean;
  /** Sets the option to allow overwriting existing file. Note that setting
   * `{ ..., clobber: false, create: true }` has the same effect as
   * `{ ..., createNew: true }`. */
  clobber?: boolean;
  /** Sets the option to allow creating a new file, if one doesn't already
   * exist at the specified path (defaults to `true`). */
  create?: boolean;
  /** Permissions to use if creating the file (defaults to `0o666`, before
   * the process's umask).
   * It's an error to specify perm when create is set to `false`.
   * Does nothing/raises on Windows. */
  perm?: number;
}

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

interface TruncateArgs {
  createNew: boolean;
  create: boolean;
  perm?: number;
  path?: string;
  len?: number;
}

/** Synchronously truncates or extends the specified file, to reach the
 * specified `len`.
 *
 *       Deno.truncateSync("hello.txt", 10);
 *
 * Requires `allow-write` permission, and `allow-read` if create is `false`. */
export function truncateSync(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): void {
  const args = checkOptions(options);
  args.path = path;
  args.len = coerceLen(len);
  sendSync("op_truncate", args);
}

/** Truncates or extends the specified file, to reach the specified `len`.
 *
 *       await Deno.truncate("hello.txt", 10);
 *
 * Requires `allow-write` permission, and `allow-read` if create is `false`. */
export async function truncate(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): Promise<void> {
  const args = checkOptions(options);
  args.path = path;
  args.len = coerceLen(len);
  await sendAsync("op_truncate", args);
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: TruncateOptions): TruncateArgs {
  let createNew = options.createNew;
  const create = options.create;
  if (options.clobber) {
    if (createNew) {
      throw new Error("'clobber' option incompatible with 'createNew' option");
    }
  } else if (options.clobber === false) {
    if (create !== false) {
      if (createNew === false) {
        throw new Error("one of options 'clobber' or 'createNew' is implied");
      }
      createNew = true;
    } else if (!createNew) {
      throw new Error(
        "one of 'clobber', 'create', or 'createNew' options is required"
      );
    }
  }
  return {
    ...options,
    createNew: !!createNew,
    create: createNew || create !== false
  };
}
