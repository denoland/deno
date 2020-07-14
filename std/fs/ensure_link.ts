// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "../path/mod.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { exists, existsSync } from "./exists.ts";
import { getFileInfoType } from "./_util.ts";

/**
 * Ensures that the hard link exists.
 * If the directory structure does not exist, it is created.
 *
 * @param src the source file path. Directory hard links are not allowed.
 * @param dest the destination link path
 */
export async function ensureLink(src: string, dest: string): Promise<void> {
  if (await exists(dest)) {
    const destStatInfo = await Deno.lstat(dest);
    const destFilePathType = getFileInfoType(destStatInfo);
    if (destFilePathType !== "file") {
      throw new Error(
        `Ensure path exists, expected 'file', got '${destFilePathType}'`,
      );
    }
    return;
  }

  await ensureDir(path.dirname(dest));

  await Deno.link(src, dest);
}

/**
 * Ensures that the hard link exists.
 * If the directory structure does not exist, it is created.
 *
 * @param src the source file path. Directory hard links are not allowed.
 * @param dest the destination link path
 */
export function ensureLinkSync(src: string, dest: string): void {
  if (existsSync(dest)) {
    const destStatInfo = Deno.lstatSync(dest);
    const destFilePathType = getFileInfoType(destStatInfo);
    if (destFilePathType !== "file") {
      throw new Error(
        `Ensure path exists, expected 'file', got '${destFilePathType}'`,
      );
    }
    return;
  }

  ensureDirSync(path.dirname(dest));

  Deno.linkSync(src, dest);
}
