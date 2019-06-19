// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "./path/mod.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
import { getFileInfoType } from "./utils.ts";

/**
 * Ensures that the file exists.
 * If the file that is requested to be created is in directories that do not
 * exist.
 * these directories are created. If the file already exists,
 * it is NOTMODIFIED.
 */
export async function ensureFile(filePath: string): Promise<void> {
  let pathExists = false;
  try {
    // if file exists
    const stat = await Deno.lstat(filePath);
    pathExists = true;
    if (!stat.isFile()) {
      throw new Error(
        `Ensure path exists, expected 'file', got '${getFileInfoType(stat)}'`
      );
    }
  } catch (err) {
    if (pathExists) {
      throw err;
    }
    // if file not exists
    // ensure dir exists
    await ensureDir(path.dirname(filePath));
    // create file
    await Deno.writeFile(filePath, new Uint8Array());
  }
}

/**
 * Ensures that the file exists.
 * If the file that is requested to be created is in directories that do not
 * exist,
 * these directories are created. If the file already exists,
 * it is NOT MODIFIED.
 */
export function ensureFileSync(filePath: string): void {
  let pathExists = false;
  try {
    // if file exists
    const stat = Deno.statSync(filePath);
    pathExists = true;
    if (!stat.isFile()) {
      throw new Error(
        `Ensure path exists, expected 'file', got '${getFileInfoType(stat)}'`
      );
    }
  } catch (err) {
    if (pathExists) {
      throw err;
    }
    // if file not exists
    // ensure dir exists
    ensureDirSync(path.dirname(filePath));
    // create file
    Deno.writeFileSync(filePath, new Uint8Array());
  }
}
