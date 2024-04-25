// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isWindows } from "./_os.ts";
import { fromFileUrl as posixFromFileUrl } from "./posix/from_file_url.ts";
import { fromFileUrl as windowsFromFileUrl } from "./windows/from_file_url.ts";

/**
 * Converts a file URL to a path string.
 *
 * ```ts
 * import { fromFileUrl } from "@std/path/from_file_url";
 *
 * // posix
 * fromFileUrl("file:///home/foo"); // "/home/foo"
 *
 * // win32
 * fromFileUrl("file:///home/foo"); // "\\home\\foo"
 * fromFileUrl("file:///C:/Users/foo"); // "C:\\Users\\foo"
 * fromFileUrl("file://localhost/home/foo"); // "\\\\localhost\\home\\foo"
 * ```
 * @param url of a file URL
 */
export function fromFileUrl(url: string | URL): string {
  return isWindows ? windowsFromFileUrl(url) : posixFromFileUrl(url);
}
