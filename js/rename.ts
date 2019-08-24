// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json";
import * as dispatch from "./dispatch";

/** Synchronously renames (moves) `oldpath` to `newpath`. If `newpath` already
 * exists and is not a directory, `renameSync()` replaces it. OS-specific
 * restrictions may apply when `oldpath` and `newpath` are in different
 * directories.
 *
 *       Deno.renameSync("old/path", "new/path");
 */
export function renameSync(oldpath: string, newpath: string): void {
  sendSync(dispatch.OP_RENAME, { oldpath, newpath });
}

/** Renames (moves) `oldpath` to `newpath`. If `newpath` already exists and is
 * not a directory, `rename()` replaces it. OS-specific restrictions may apply
 * when `oldpath` and `newpath` are in different directories.
 *
 *       await Deno.rename("old/path", "new/path");
 */
export async function rename(oldpath: string, newpath: string): Promise<void> {
  await sendAsync(dispatch.OP_RENAME, { oldpath, newpath });
}
