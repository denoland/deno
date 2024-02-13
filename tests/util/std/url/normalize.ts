// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { normalize as posixNormalize } from "../path/posix/normalize.ts";

/**
 * Normalize the `URL`, resolving `'..'` and `'.'` segments and multiple
 * `'/'`s into `'//'` after protocol and remaining into `'/'`.
 *
 * @example
 * ```ts
 * import { normalize } from "https://deno.land/std@$STD_VERSION/url/normalize.ts";
 *
 * console.log(normalize("https:///deno.land///std//assert//.//mod.ts").href);
 * // Outputs: "https://deno.land/std/path/mod.ts"
 *
 * console.log(normalize("https://deno.land/std/assert/../async/retry.ts").href);
 * // Outputs: "https://deno.land/std/async/retry.ts"
 * ```
 *
 * @param url to be normalized
 * @returns normalized URL
 */
export function normalize(url: string | URL): URL {
  url = new URL(url);
  url.pathname = posixNormalize(url.pathname);
  return url;
}
