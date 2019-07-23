// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { stat, statSync } from "./stat";
import { open, openSync } from "./files";
import { chmod, chmodSync } from "./chmod";
import { writeAll, writeAllSync } from "./buffer";

/** Options for writing to a file.
 * `perm` would change the file's permission if set.
 * `create` decides if the file should be created if not exists (default: true)
 * `append` decides if the file should be appended (default: false)
 */
export interface WriteFileOptions {
  perm?: number;
  create?: boolean;
  append?: boolean;
}

/** Write a new file, with given filename and data synchronously.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       Deno.writeFileSync("hello.txt", data);
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

/** Write a new file, with given filename and data.
 *
 *       const encoder = new TextEncoder();
 *       const data = encoder.encode("Hello world\n");
 *       await Deno.writeFile("hello.txt", data);
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
