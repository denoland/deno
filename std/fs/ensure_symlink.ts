// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "./path/mod.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { exists, existsSync } from "./exists.ts";
import { getFileInfoType } from "./utils.ts";

/**
 * Ensures that the link exists.
 * If the directory structure does not exist, it is created.
 *
 * @param src the source file path
 * @param dest the destination link path
 */
export async function ensureSymlink(src: string, dest: string): Promise<void> {
  const srcStatInfo = await Deno.lstat(src);
  const srcFilePathType = getFileInfoType(srcStatInfo);

  if (await exists(dest)) {
    const destStatInfo = await Deno.lstat(dest);
    const destFilePathType = getFileInfoType(destStatInfo);
    if (destFilePathType !== "symlink") {
      throw new Error(
        `Ensure path exists, expected 'symlink', got '${destFilePathType}'`
      );
    }
    return;
  }

  await ensureDir(path.dirname(dest));

  await Deno.symlink(src, dest, srcFilePathType);
}

/**
 * Ensures that the link exists.
 * If the directory structure does not exist, it is created.
 *
 * @param src the source file path
 * @param dest the destination link path
 */
export function ensureSymlinkSync(src: string, dest: string): void {
  const srcStatInfo = Deno.lstatSync(src);
  const srcFilePathType = getFileInfoType(srcStatInfo);

  if (existsSync(dest)) {
    const destStatInfo = Deno.lstatSync(dest);
    const destFilePathType = getFileInfoType(destStatInfo);
    if (destFilePathType !== "symlink") {
      throw new Error(
        `Ensure path exists, expected 'symlink', got '${destFilePathType}'`
      );
    }
    return;
  }

  ensureDirSync(path.dirname(dest));

  Deno.symlinkSync(src, dest, srcFilePathType);
}
