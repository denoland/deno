// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "deno_dispatch_json";
import { opNamespace } from "./namespace.ts";

const OP_MKDIR = new JsonOp(opNamespace, "mkdir");

/** Creates a new directory with the specified path synchronously.
 * If `recursive` is set to true, nested directories will be created (also known
 * as "mkdir -p").
 * `mode` sets permission bits (before umask) on UNIX and does nothing on
 * Windows.
 *
 *       Deno.mkdirSync("new_dir");
 *       Deno.mkdirSync("nested/directories", true);
 */
export function mkdirSync(path: string, recursive = false, mode = 0o777): void {
  OP_MKDIR.sendSync({ path, recursive, mode });
}

/** Creates a new directory with the specified path.
 * If `recursive` is set to true, nested directories will be created (also known
 * as "mkdir -p").
 * `mode` sets permission bits (before umask) on UNIX and does nothing on
 * Windows.
 *
 *       await Deno.mkdir("new_dir");
 *       await Deno.mkdir("nested/directories", true);
 */
export async function mkdir(
  path: string,
  recursive = false,
  mode = 0o777
): Promise<void> {
  await OP_MKDIR.sendAsync({ path, recursive, mode });
}
