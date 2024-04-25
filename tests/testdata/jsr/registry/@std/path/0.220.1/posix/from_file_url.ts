// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { assertArg } from "../_common/from_file_url.ts";

/**
 * Converts a file URL to a path string.
 *
 * ```ts
 * import { fromFileUrl } from "@std/path/posix/from_file_url";
 *
 * fromFileUrl("file:///home/foo"); // "/home/foo"
 * ```
 * @param url of a file URL
 */
export function fromFileUrl(url: URL | string): string {
  url = assertArg(url);
  return decodeURIComponent(
    url.pathname.replace(/%(?![0-9A-Fa-f]{2})/g, "%25"),
  );
}
