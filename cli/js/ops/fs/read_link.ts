// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

/** Returns the destination of the named symbolic link.
 *
 *       const targetPath = Deno.readlinkSync("symlink/path");
 *
 * Requires `allow-read` permission. */
export function readlinkSync(path: string): string {
  return sendSync("op_read_link", { path });
}

/** Resolves to the destination of the named symbolic link.
 *
 *       const targetPath = await Deno.readlink("symlink/path");
 *
 * Requires `allow-read` permission. */
export async function readlink(path: string): Promise<string> {
  return await sendAsync("op_read_link", { path });
}
