// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isWindows } from "./_os.ts";
import { isAbsolute as posixIsAbsolute } from "./posix/is_absolute.ts";
import { isAbsolute as windowsIsAbsolute } from "./windows/is_absolute.ts";

/**
 * Verifies whether provided path is absolute
 * @param path to be verified as absolute
 */
export function isAbsolute(path: string): boolean {
  return isWindows ? windowsIsAbsolute(path) : posixIsAbsolute(path);
}
