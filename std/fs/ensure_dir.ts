// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { getFileInfoType } from "./utils.ts";
/**
 * Ensures that the directory exists.
 * If the directory structure does not exist, it is created. Like mkdir -p.
 */
export async function ensureDir(dir: string): Promise<void> {
  let pathExists = false;
  try {
    // if dir exists
    const stat = await Deno.stat(dir);
    pathExists = true;
    if (!stat.isDirectory()) {
      throw new Error(
        `Ensure path exists, expected 'dir', got '${getFileInfoType(stat)}'`
      );
    }
  } catch (err) {
    if (pathExists) {
      throw err;
    }
    // if dir not exists. then create it.
    await Deno.mkdir(dir, true);
  }
}

/**
 * Ensures that the directory exists.
 * If the directory structure does not exist, it is created. Like mkdir -p.
 */
export function ensureDirSync(dir: string): void {
  let pathExists = false;
  try {
    // if dir exists
    const stat = Deno.statSync(dir);
    pathExists = true;
    if (!stat.isDirectory()) {
      throw new Error(
        `Ensure path exists, expected 'dir', got '${getFileInfoType(stat)}'`
      );
    }
  } catch (err) {
    if (pathExists) {
      throw err;
    }
    // if dir not exists. then create it.
    Deno.mkdirSync(dir, true);
  }
}
