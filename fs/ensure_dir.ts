// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/**
 * Ensures that the directory exists. If the directory structure does not exist, it is created. Like mkdir -p.
 * @export
 * @param {string} dir
 * @returns {Promise<void>}
 */
export async function ensureDir(dir: string): Promise<void> {
  try {
    // if dir exists
    await Deno.stat(dir);
  } catch {
    // if dir not exists. then create it.
    await Deno.mkdir(dir, true);
  }
}

/**
 * Ensures that the directory exists. If the directory structure does not exist, it is created. Like mkdir -p.
 * @export
 * @param {string} dir
 * @returns {void}
 */
export function ensureDirSync(dir: string): void {
  try {
    // if dir exists
    Deno.statSync(dir);
  } catch {
    // if dir not exists. then create it.
    Deno.mkdirSync(dir, true);
  }
}
