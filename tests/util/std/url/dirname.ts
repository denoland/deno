// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { dirname as posixDirname } from "../path/posix/dirname.ts";
import { strip } from "./_strip.ts";

/**
 * Return the directory path of a `URL`.  A directory path is the portion of a
 * `URL` up to but excluding the final path segment.  The final path segment,
 * along with any query or hash values are removed. If there is no path segment
 * then the URL origin is returned. Example, for the URL
 * `https://deno.land/std/path/mod.ts`, the directory path is
 * `https://deno.land/std/path`.
 *
 * @example
 * ```ts
 * import { dirname } from "https://deno.land/std@$STD_VERSION/url/dirname.ts";
 *
 * console.log(dirname("https://deno.land/std/path/mod.ts?a=b").href); // "https://deno.land/std/path"
 * console.log(dirname("https://deno.land/").href); // "https://deno.land"
 * ```
 *
 * @param url - url to extract the directory from.
 * @returns a new URL containing the directory path of the URL.
 */
export function dirname(url: string | URL): URL {
  url = new URL(url);
  strip(url);
  url.pathname = posixDirname(url.pathname);
  return url;
}
