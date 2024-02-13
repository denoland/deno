// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { basename as posixBasename } from "../path/posix/basename.ts";
import { strip } from "./_strip.ts";

/**
 * Return the last portion of a `URL`, or the host name if there is no path.
 * Trailing `/`s are ignored, and optional suffix is removed.
 *
 * @example
 * ```ts
 * import { basename } from "https://deno.land/std@$STD_VERSION/url/basename.ts";
 *
 * // basename accepts a string or URL
 * console.log(basename("https://deno.land/std/assert/mod.ts"));  // "mod.ts"
 * console.log(basename(new URL("https://deno.land/std/assert/mod.ts"))); // "mod.ts"
 *
 * // basename accepts an optional suffix to remove
 * console.log(basename(new URL("https://deno.land/std/assert/mod.ts"), ".ts")); // "mod"
 *
 * // basename does not include query parameters or hash fragments
 * console.log(basename(new URL("https://deno.land/std/assert/mod.ts?a=b"))); // "mod.ts"
 * console.log(basename(new URL("https://deno.land/std/assert/mod.ts#header"))); // "mod.ts"
 *
 * // If no path is present, the host name is returned
 * console.log(basename(new URL("https://deno.land/"))); // "deno.land"
 * ```
 *
 * @param url - url to extract the final path segment from.
 * @param suffix - optional suffix to remove from extracted name.
 * @returns the last portion of the URL `path`, or the URL origin if there is no path.
 */
export function basename(url: string | URL, suffix?: string): string {
  url = new URL(url);
  strip(url);
  return posixBasename(url.href, suffix);
}
