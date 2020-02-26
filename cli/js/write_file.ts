// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { open, openSync, OpenOptions } from "./files.ts";
import { writeAll, writeAllSync } from "./buffer.ts";

/** Options for writing to a file. */
export interface WriteFileOptions {
  /** Defaults to `false`. If set to `true`, will append to a file instead of
   * overwriting previous contents. */
  append?: boolean;
  /** Defaults to `false`. If set to `true`, no file, directory, or symlink is
   * allowed to exist at the target location. When createNew is set to `true`,
   * create is ignored. */
  createNew?: boolean;
  /** Sets the option to allow overwriting existing file (defaults to `true`).
   * Note that setting `{ ..., clobber: false, create: true }` has the same
   * effect as `{ ..., createNew: true }`. */
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

/** Synchronously write data to the given path, by default creating a new
 * file if needed, else overwriting.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       Deno.writeFileSync("hello.txt", data);
 *
 * Requires `allow-write` permission, and `allow-read` if create is `false`.
 */
export function writeFileSync(
  path: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): void {
  const openOptions: OpenOptions = checkOptions(options);
  openOptions.write = true;
  openOptions.truncate = !openOptions.append;
  const file = openSync(path, openOptions);
  writeAllSync(file, data);
  file.close();
}

/** Write data to the given path, by default creating a new file if needed,
 * else overwriting.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       await Deno.writeFile("hello.txt", data);
 *
 * Requires `allow-write` permission, and `allow-read` if create is `false`.
 */
export async function writeFile(
  path: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): Promise<void> {
  const openOptions: OpenOptions = checkOptions(options);
  openOptions.write = true;
  openOptions.truncate = !openOptions.append;
  const file = await open(path, openOptions);
  await writeAll(file, data);
  file.close();
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: WriteFileOptions): WriteFileOptions {
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
