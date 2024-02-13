// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Strips any hash (eg. `#header`) or search parameters (eg. `?foo=bar`) from the provided URL.
 *
 * (Mutates the original url provided)
 * @param url to be stripped.
 */
export function strip(url: URL) {
  url.hash = "";
  url.search = "";
}
