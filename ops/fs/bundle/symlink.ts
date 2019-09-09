// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "deno_dispatch_json";
import { opNamespace } from "./namespace.ts";
import { notImplemented, build } from "deno_util";

const OP_SYMLINK = new JsonOp(opNamespace, "symlink")

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
    return notImplemented();
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
    return notImplemented();
  }
  await OP_SYMLINK.sendAsync({ oldname, newname });
}
