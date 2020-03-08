// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";

export interface MakeTempOptions {
  /** Directory where the temporary directory should be created (defaults to
   * the env variable TMPDIR, or the system's default, usually /tmp). */
  dir?: string;
  /** String that should precede the random portion of the temporary
   * directory's name. */
  prefix?: string;
  /** String that should follow the random portion of the temporary
   * directory's name. */
  suffix?: string;
}

/** Synchronously creates a new temporary directory in the directory `dir`,
 * its name beginning with `prefix` and ending with `suffix`.
 *
 * It returns the full path to the newly created directory.
 *
 * If `dir` is unspecified, uses the default directory for temporary files.
 * Multiple programs calling this function simultaneously will create different
 * directories. It is the caller's responsibility to remove the directory when
 * no longer needed.
 *
 *       const tempDirName0 = Deno.makeTempDirSync();
 *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
 *
 * Requires `allow-write` permission. */
export function makeTempDirSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_dir", options);
}

/** Creates a new temporary directory in the directory `dir`, its name
 * beginning with `prefix` and ending with `suffix`.
 *
 * It resolves to the full path to the newly created directory.
 *
 * If `dir` is unspecified, uses the default directory for temporary files.
 * Multiple programs calling this function simultaneously will create different
 * directories. It is the caller's responsibility to remove the directory when
 * no longer needed.
 *
 *       const tempDirName0 = await Deno.makeTempDir();
 *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' });
 *
 * Requires `allow-write` permission. */
export async function makeTempDir(
  options: MakeTempOptions = {}
): Promise<string> {
  return await sendAsync("op_make_temp_dir", options);
}

/** Synchronously creates a new temporary file in the directory `dir`, its name
 * beginning with `prefix` and ending with `suffix`.
 *
 * It returns the full path to the newly created file.
 *
 * If `dir` is unspecified, uses the default directory for temporary files.
 * Multiple programs calling this function simultaneously will create different
 * files. It is the caller's responsibility to remove the file when
 * no longer needed.
 *
 *       const tempFileName0 = Deno.makeTempFileSync();
 *       const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });
 *
 * Requires `allow-write` permission. */
export function makeTempFileSync(options: MakeTempOptions = {}): string {
  return sendSync("op_make_temp_file", options);
}

/** Creates a new temporary file in the directory `dir`, its name
 * beginning with `prefix` and ending with `suffix`.
 *
 * It resolves to the full path to the newly created file.
 *
 * If `dir` is unspecified, uses the default directory for temporary files.
 * Multiple programs calling this function simultaneously will create different
 * files. It is the caller's responsibility to remove the file when
 * no longer needed.
 *
 *       const tempFileName0 = await Deno.makeTempFile();
 *       const tempFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });
 *
 * Requires `allow-write` permission. */
export async function makeTempFile(
  options: MakeTempOptions = {}
): Promise<string> {
  return await sendAsync("op_make_temp_file", options);
}
