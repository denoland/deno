// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "./path/mod.ts";
import { ensureDir, ensureDirSync } from "./ensure_dir.ts";
/**
 * Ensures that the file exists.
 * If the file that is requested to be created is in directories that do not exist, these directories are created. If the file already exists, it is NOT MODIFIED.
 * @export
 * @param {string} filePath
 * @returns {Promise<void>}
 */
export async function ensureFile(filePath: string): Promise<void> {
  try {
    // if file exists
    await Deno.stat(filePath);
  } catch {
    // if file not exists
    // ensure dir exists
    await ensureDir(path.dirname(filePath));
    // create file
    await Deno.writeFile(filePath, new Uint8Array());
  }
}

/**
 * Ensures that the file exists.
 * If the file that is requested to be created is in directories that do not exist, these directories are created. If the file already exists, it is NOT MODIFIED.
 * @export
 * @param {string} filePath
 * @returns {void}
 */
export function ensureFileSync(filePath: string): void {
  try {
    // if file exists
    Deno.statSync(filePath);
  } catch {
    // if file not exists
    // ensure dir exists
    ensureDirSync(path.dirname(filePath));
    // create file
    Deno.writeFileSync(filePath, new Uint8Array());
  }
}
