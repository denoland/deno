// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { dirname } from "../path/dirname.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { toPathString } from "./_util.ts";

/**
 * Ensures that the hard link exists.
 * If the directory structure does not exist, it is created.
 *
 * @example
 * ```ts
 * import { ensureSymlink } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
 *
 * ensureSymlink("./folder/targetFile.dat", "./folder/targetFile.link.dat"); // returns promise
 * ```
 *
 * @param src the source file path. Directory hard links are not allowed.
 * @param dest the destination link path
 */
export async function ensureLink(src: string | URL, dest: string | URL) {
  dest = toPathString(dest);
  await ensureDir(dirname(dest));

  await Deno.link(toPathString(src), dest);
}

/**
 * Ensures that the hard link exists.
 * If the directory structure does not exist, it is created.
 *
 * @example
 * ```ts
 * import { ensureSymlinkSync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
 *
 * ensureSymlinkSync("./folder/targetFile.dat", "./folder/targetFile.link.dat"); // void
 * ```
 *
 * @param src the source file path. Directory hard links are not allowed.
 * @param dest the destination link path
 */
export function ensureLinkSync(src: string | URL, dest: string | URL) {
  dest = toPathString(dest);
  ensureDirSync(dirname(dest));

  Deno.linkSync(toPathString(src), dest);
}
