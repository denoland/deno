// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { SeekMode } from "../../io.ts";

/** Synchronously seek a file ID to the given offset under mode given by `whence`.
 *
 * Returns the number of cursor position.
 *
 *       const file = Deno.openSync("/foo/bar.txt");
 *       const position = Deno.seekSync(file.rid, 0, 0);
 */
export function seekSync(
  rid: number,
  offset: number,
  whence: SeekMode
): number {
  return sendSync("op_seek", { rid, offset, whence });
}

/** Seek a file ID to the given offset under mode given by `whence`.
 *
 * Resolves with the number of cursor position.
 *
 *      const file = await Deno.open("/foo/bar.txt");
 *      const position = await Deno.seek(file.rid, 0, 0);
 */
export async function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<number> {
  return await sendAsync("op_seek", { rid, offset, whence });
}
