// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface OpenOptions {
  /** Sets the option for read access. This option, when `true`, means that the
   * file should be read-able if opened. */
  read?: boolean;
  /** Sets the option for write access. This option, when `true`, means that
   * the file should be write-able if opened. If the file already exists,
   * any write calls on it will overwrite its contents, by default without
   * truncating it. */
  write?: boolean;
  /**Sets the option for the append mode. This option, when `true`, means that
   * writes will append to a file instead of overwriting previous contents.
   * Note that setting `{ write: true, append: true }` has the same effect as
   * setting only `{ append: true }`. */
  append?: boolean;
  /** Sets the option for truncating a previous file. If a file is
   * successfully opened with this option set it will truncate the file to `0`
   * length if it already exists. The file must be opened with write access
   * for truncate to work. */
  truncate?: boolean;
  /** Sets the option to allow creating a new file, if one doesn't already
   * exist at the specified path. Requires write or append access to be
   * used. */
  create?: boolean;
  /** Defaults to `false`. If set to `true`, no file, directory, or symlink is
   * allowed to exist at the target location. Requires write or append
   * access to be used. When createNew is set to `true`, create and truncate
   * are ignored. */
  createNew?: boolean;
}

/** A set of string literals which specify the open mode of a file.
 *
 * |Value |Description                                                                                       |
 * |------|--------------------------------------------------------------------------------------------------|
 * |`"r"` |Read-only. Default. Starts at beginning of file.                                                  |
 * |`"r+"`|Read-write. Start at beginning of file.                                                           |
 * |`"w"` |Write-only. Opens and truncates existing file or creates new one for writing only.                |
 * |`"w+"`|Read-write. Opens and truncates existing file or creates new one for writing and reading.         |
 * |`"a"` |Write-only. Opens existing file or creates new one. Each write appends content to the end of file.|
 * |`"a+"`|Read-write. Behaves like `"a"` and allows to read from file.                                      |
 * |`"x"` |Write-only. Exclusive create - creates new file only if one doesn't exist already.                |
 * |`"x+"`|Read-write. Behaves like `x` and allows reading from file.                                        |
 */
export type OpenMode = "r" | "r+" | "w" | "w+" | "a" | "a+" | "x" | "x+";

export function openSync(
  path: string,
  mode: OpenMode | undefined,
  options: OpenOptions | undefined
): number {
  return sendSync("op_open", { path, options, mode });
}

export async function open(
  path: string,
  mode: OpenMode | undefined,
  options: OpenOptions | undefined
): Promise<number> {
  return await sendAsync("op_open", {
    path,
    options,
    mode
  });
}
