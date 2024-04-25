// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isWindows } from "./_os.ts";
import { toFileUrl as posixToFileUrl } from "./posix/to_file_url.ts";
import { toFileUrl as windowsToFileUrl } from "./windows/to_file_url.ts";

/**
 * Converts a path string to a file URL.
 *
 * ```ts
 * import { toFileUrl } from "@std/path/to_file_url";
 *
 * // posix
 * toFileUrl("/home/foo"); // new URL("file:///home/foo")
 *
 * // win32
 * toFileUrl("\\home\\foo"); // new URL("file:///home/foo")
 * toFileUrl("C:\\Users\\foo"); // new URL("file:///C:/Users/foo")
 * toFileUrl("\\\\127.0.0.1\\home\\foo"); // new URL("file://127.0.0.1/home/foo")
 * ```
 * @param path to convert to file URL
 */
export function toFileUrl(path: string): URL {
  return isWindows ? windowsToFileUrl(path) : posixToFileUrl(path);
}
