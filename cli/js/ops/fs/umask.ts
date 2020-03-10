// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "../dispatch_json.ts";

/**
 * **UNSTABLE**: maybe needs `allow-env` permissions.
 *
 * If `mask` is provided, sets the process umask. Always returns what the umask
 * was before the call.
 */
export function umask(mask?: number): number {
  return sendSync("op_umask", { mask });
}
