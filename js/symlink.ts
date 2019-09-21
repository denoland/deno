// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";
import * as util from "./util.ts";
import { build } from "./build.ts";

const OP_SYMLINK = new JsonOp("symlink");

/** Synchronously creates `newname` as a symbolic link to `oldname`. The type
 * argument can be set to `dir` or `file` and is only available on Windows
 * (ignored on other platforms).
 *
 *       Deno.symlinkSync("old/name", "new/name");
 */
export function symlinkSync(
  oldname: string,
  newname: string,
  type?: string
): void {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  OP_SYMLINK.sendSync({ oldname, newname });
}

/** Creates `newname` as a symbolic link to `oldname`. The type argument can be
 * set to `dir` or `file` and is only available on Windows (ignored on other
 * platforms).
 *
 *       await Deno.symlink("old/name", "new/name");
 */
export async function symlink(
  oldname: string,
  newname: string,
  type?: string
): Promise<void> {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  await OP_SYMLINK.sendAsync({ oldname, newname });
}
