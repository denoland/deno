// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";
import * as util from "./util.ts";
import { build } from "./build.ts";

/** **UNSTABLE**: `type` argument type may be changed to `"dir" | "file"`.
 *
 * Creates `newname` as a symbolic link to `oldname`. The type argument can be
 * set to `dir` or `file`. Is only available on Windows and ignored on other
 * platforms.
 *
 *       Deno.symlinkSync("old/name", "new/name");
 *
 * Requires `allow-read` and `allow-write` permissions. */
export function symlinkSync(
  oldname: string,
  newname: string,
  type?: string
): void {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  sendSync("op_symlink", { oldname, newname });
}

/** **UNSTABLE**: `type` argument may be changed to "dir" | "file"
 *
 * Creates `newname` as a symbolic link to `oldname`. The type argument can be
 * set to `dir` or `file`. Is only available on Windows and ignored on other
 * platforms.
 *
 *       await Deno.symlink("old/name", "new/name");
 *
 * Requires `allow-read` and `allow-write` permissions. */
export async function symlink(
  oldname: string,
  newname: string,
  type?: string
): Promise<void> {
  if (build.os === "win" && type) {
    return util.notImplemented();
  }
  await sendAsync("op_symlink", { oldname, newname });
}
