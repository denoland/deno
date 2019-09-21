// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

const OP_COPY_FILE = new JsonOp("copy_file");

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
  OP_COPY_FILE.sendSync({ from, to });
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
  await OP_COPY_FILE.sendAsync({ from, to });
}
