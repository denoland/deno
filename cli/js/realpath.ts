// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

/** Returns absolute normalized path with symbolic links resolved synchronously.
 *
 *       const realPath = Deno.realpathSync("./some/path");
 */
export function realpathSync(path: string): string {
  return sendSync(dispatch.OP_REALPATH, { path });
}

/** Returns absolute normalized path with symbolic links resolved.
 *
 *       const realPath = await Deno.realpath("./some/path");
 */
export async function realpath(path: string): Promise<string> {
  return await sendAsync(dispatch.OP_REALPATH, { path });
}
