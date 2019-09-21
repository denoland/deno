// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

const OP_LINK = new JsonOp("link");

/** Synchronously creates `newname` as a hard link to `oldname`.
 *
 *       Deno.linkSync("old/name", "new/name");
 */
export function linkSync(oldname: string, newname: string): void {
  OP_LINK.sendSync({ oldname, newname });
}

/** Creates `newname` as a hard link to `oldname`.
 *
 *       await Deno.link("old/name", "new/name");
 */
export async function link(oldname: string, newname: string): Promise<void> {
  await OP_LINK.sendAsync({ oldname, newname });
}
