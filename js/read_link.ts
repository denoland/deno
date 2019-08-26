// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json";
import * as dispatch from "./dispatch";

/** Returns the destination of the named symbolic link synchronously.
 *
 *       const targetPath = Deno.readlinkSync("symlink/path");
 */
export function readlinkSync(name: string): string {
  return sendSync(dispatch.OP_READ_LINK, { name });
}

/** Returns the destination of the named symbolic link.
 *
 *       const targetPath = await Deno.readlink("symlink/path");
 */
export async function readlink(name: string): Promise<string> {
  return await sendAsync(dispatch.OP_READ_LINK, { name });
}
