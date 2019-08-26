// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json";
import * as dispatch from "./dispatch";
import * as util from "./util";
import { platform } from "./build";

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
  if (platform.os === "win" && type) {
    return util.notImplemented();
  }
  sendSync(dispatch.OP_SYMLINK, { oldname, newname });
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
  if (platform.os === "win" && type) {
    return util.notImplemented();
  }
  await sendAsync(dispatch.OP_SYMLINK, { oldname, newname });
}
