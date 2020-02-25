// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

/**
 * `cwd()` Return a string representing the current working directory.
 * If the current directory can be reached via multiple paths
 * (due to symbolic links), `cwd()` may return
 * any one of them.
 * throws `NotFound` exception if directory not available
 */
export function cwd(): string {
  return sendSync("op_cwd");
}

/**
 * `chdir()` Change the current working directory to path.
 * throws `NotFound` exception if directory not available
 */
export function chdir(directory: string): void {
  sendSync("op_chdir", { directory });
}
