// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

/** Synchronously creates `newname` as a hard link to `oldname`.
 *
 *       Deno.linkSync("old/name", "new/name");
 */
export function linkSync(oldname: string, newname: string): void {
  sendSync("op_link", { oldname, newname });
}

/** Creates `newname` as a hard link to `oldname`.
 *
 *       await Deno.link("old/name", "new/name");
 */
export async function link(oldname: string, newname: string): Promise<void> {
  await sendAsync("op_link", { oldname, newname });
}
