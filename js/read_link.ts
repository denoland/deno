// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

const OP_READ_LINK = new JsonOp("read_link");

/** Returns the destination of the named symbolic link synchronously.
 *
 *       const targetPath = Deno.readlinkSync("symlink/path");
 */
export function readlinkSync(name: string): string {
  return OP_READ_LINK.sendSync({ name });
}

/** Returns the destination of the named symbolic link.
 *
 *       const targetPath = await Deno.readlink("symlink/path");
 */
export async function readlink(name: string): Promise<string> {
  return await OP_READ_LINK.sendAsync({ name });
}
