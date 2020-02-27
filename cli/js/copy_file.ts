// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface CopyFileOptions {
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
}

interface CopyFileArgs {
  createNew: boolean;
  create: boolean;
  from?: string;
  to?: string;
}

/** Synchronously copies the contents and permissions of one file to another
 * specified path, by default creating a new file if needed, else overwriting.
 * Fails if target path is a directory or is unwritable.
 *
 *       Deno.copyFileSync("from.txt", "to.txt");
 *
 * Requires `allow-read` permission on fromPath.
 * Requires `allow-write` permission on toPath, and `allow-read` if create is
 * `false`. */
export function copyFileSync(
  fromPath: string,
  toPath: string,
  options: CopyFileOptions = {}
): void {
  const args = checkOptions(options);
  args.from = fromPath;
  args.to = toPath;
  sendSync("op_copy_file", args);
}

/** Copies the contents and permissions of one file to another specified path,
 * by default creating a new file if needed, else overwriting. Fails if target
 * path is a directory or is unwritable.
 *
 *       await Deno.copyFile("from.txt", "to.txt");
 *
 * Requires `allow-read` permission on fromPath.
 * Requires `allow-write` permission on toPath, and `allow-read` if create is
 * `false`. */
export async function copyFile(
  fromPath: string,
  toPath: string,
  options: CopyFileOptions = {}
): Promise<void> {
  const args = checkOptions(options);
  args.from = fromPath;
  args.to = toPath;
  await sendAsync("op_copy_file", args);
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: CopyFileOptions): CopyFileArgs {
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
