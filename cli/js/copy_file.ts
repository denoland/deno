// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

/** Copies the contents of a file to another by name synchronously.
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 *
 * It would also copy the permission of the original file
 * to the destination.
 *
 *       Deno.copyFileSync("from.txt", "to.txt");
 */
export function copyFileSync(from: string, to: string): void {
  sendSync("op_copy_file", { from, to });
}

/** Copies the contents of a file to another by name.
 *
 * Creates a new file if target does not exists, and if target exists,
 * overwrites original content of the target file.
 *
 * It would also copy the permission of the original file
 * to the destination.
 *
 *       await Deno.copyFile("from.txt", "to.txt");
 */
export async function copyFile(from: string, to: string): Promise<void> {
  await sendAsync("op_copy_file", { from, to });
}
