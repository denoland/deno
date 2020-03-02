// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { stat, statSync } from "./stat.ts";
import { open, openSync } from "./files.ts";
import { chmod, chmodSync } from "./chmod.ts";
import { writeAll, writeAllSync } from "./buffer.ts";

/** Options for writing to a file. */
export interface WriteFileOptions {
  /** Defaults to `false`. If set to `true`, will append to a file instead of
   * overwriting previous contents. */
  append?: boolean;
  /** Sets the option to allow creating a new file, if one doesn't already
   * exist at the specified path (defaults to `true`). */
  create?: boolean;
  /** Permissions always applied to file. */
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
  filename: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): void {
  if (options.create !== undefined) {
    const create = !!options.create;
    if (!create) {
      // verify that file exists
      statSync(filename);
    }
  }

  const openMode = !!options.append ? "a" : "w";
  const file = openSync(filename, openMode);

  if (options.perm !== undefined && options.perm !== null) {
    chmodSync(filename, options.perm);
  }

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
  filename: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): Promise<void> {
  if (options.create !== undefined) {
    const create = !!options.create;
    if (!create) {
      // verify that file exists
      await stat(filename);
    }
  }

  const openMode = !!options.append ? "a" : "w";
  const file = await open(filename, openMode);

  if (options.perm !== undefined && options.perm !== null) {
    await chmod(filename, options.perm);
  }

  await writeAll(file, data);
  file.close();
}
