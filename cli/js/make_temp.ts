// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export interface MakeTempOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}

/** makeTempDirSync is the synchronous version of `makeTempDir`.
 *
 *       const tempDirName0 = Deno.makeTempDirSync();
 *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
 */
export function makeTempDirSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_dir", options);
}

/** makeTempDir creates a new temporary directory in the directory `dir`, its
 * name beginning with `prefix` and ending with `suffix`.
 * It returns the full path to the newly created directory.
 * If `dir` is unspecified, tempDir uses the default directory for temporary
 * files. Multiple programs calling tempDir simultaneously will not choose the
 * same directory. It is the caller's responsibility to remove the directory
 * when no longer needed.
 *
 *       const tempDirName0 = await Deno.makeTempDir();
 *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' });
 */
export async function makeTempDir(
  options: MakeTempOptions = {}
): Promise<string> {
  return await sendAsync("op_make_temp_dir", options);
}

/** makeTempFileSync is the synchronous version of `makeTempFile`.
 *
 *       const tempFileName0 = Deno.makeTempFileSync();
 *       const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });
 */
export function makeTempFileSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_file", options);
}

/** makeTempFile creates a new temporary file in the directory `dir`, its
 * name beginning with `prefix` and ending with `suffix`.
 * It returns the full path to the newly created file.
 * If `dir` is unspecified, tempFile uses the default directory for temporary
 * files. Multiple programs calling tempFile simultaneously will not choose the
 * same directory. It is the caller's responsibility to remove the file
 * when no longer needed.
 *
 *       const tempFileName0 = await Deno.makeTempFile();
 *       const tempFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });
 */
export async function makeTempFile(
  options: MakeTempOptions = {}
): Promise<string> {
  return await sendAsync("op_make_temp_file", options);
}
