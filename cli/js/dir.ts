// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

/**
 * **UNSTABLE**: maybe needs permissions.
 *
 * Return a string representing the current working directory.
 *
 * If the current directory can be reached via multiple paths (due to symbolic
 * links), `cwd()` may return any one of them.
 *
 * Throws `Deno.errors.NotFound` if directory not available.
 */
export function cwd(): string {
  return sendSync("op_cwd");
}

/**
 * **UNSTABLE**: maybe needs permissions.
 *
 * Change the current working directory to the specified path.
 *
 * Throws `Deno.errors.NotFound` if directory not available.
 */
export function chdir(directory: string): void {
  sendSync("op_chdir", { directory });
}
