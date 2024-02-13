// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { join as posixJoin } from "../path/posix/join.ts";

/**
 * Join a base `URL` and a series of `paths`, then normalizes the resulting URL.
 *
 * @example
 * ```ts
 * import { join } from "https://deno.land/std@$STD_VERSION/url/join.ts";
 *
 * console.log(join("https://deno.land/", "std", "path", "mod.ts").href);
 * // Outputs: "https://deno.land/std/path/mod.ts"
 *
 * console.log(join("https://deno.land", "//std", "path/", "/mod.ts").href);
 * // Outputs: "https://deno.land/path/mod.ts"
 * ```
 *
 * @param url the base URL to be joined with the paths and normalized
 * @param paths array of path segments to be joined to the base URL
 * @returns a complete URL string containing the base URL joined with the paths
 */
export function join(url: string | URL, ...paths: string[]): URL {
  url = new URL(url);
  url.pathname = posixJoin(url.pathname, ...paths);
  return url;
}
