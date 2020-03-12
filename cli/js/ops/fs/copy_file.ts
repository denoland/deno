// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

/** Synchronously copies the contents and permissions of one file to another
 * specified path, by default creating a new file if needed, else overwriting.
 * Fails if target path is a directory or is unwritable.
 *
 *       Deno.copyFileSync("from.txt", "to.txt");
 *
 * Requires `allow-read` permission on fromPath.
 * Requires `allow-write` permission on toPath. */
export function copyFileSync(fromPath: string, toPath: string): void {
  sendSync("op_copy_file", { from: fromPath, to: toPath });
}

/** Copies the contents and permissions of one file to another specified path,
 * by default creating a new file if needed, else overwriting. Fails if target
 * path is a directory or is unwritable.
 *
 *       await Deno.copyFile("from.txt", "to.txt");
 *
 * Requires `allow-read` permission on fromPath.
 * Requires `allow-write` permission on toPath. */
export async function copyFile(
  fromPath: string,
  toPath: string
): Promise<void> {
  await sendAsync("op_copy_file", { from: fromPath, to: toPath });
}
