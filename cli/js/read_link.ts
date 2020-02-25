// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

/** Returns the destination of the named symbolic link synchronously.
 *
 *       const targetPath = Deno.readlinkSync("symlink/path");
 */
export function readlinkSync(name: string): string {
  return sendSync("op_read_link", { name });
}

/** Returns the destination of the named symbolic link.
 *
 *       const targetPath = await Deno.readlink("symlink/path");
 */
export async function readlink(name: string): Promise<string> {
  return await sendAsync("op_read_link", { name });
}
