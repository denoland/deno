// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { isWindows } from "./_os.ts";
import type { ParsedPath } from "./_interface.ts";
import { parse as posixParse } from "./posix/parse.ts";
import { parse as windowsParse } from "./windows/parse.ts";

/**
 * Return a `ParsedPath` object of the `path`.
 * @param path to process
 */
export function parse(path: string): ParsedPath {
  return isWindows ? windowsParse(path) : posixParse(path);
}
