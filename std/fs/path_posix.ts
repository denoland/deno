// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
/** This module is browser compatible. */

import {
  assertPath,
  isPosixPathSeparator,
  normalizeString,
} from "../path/_util.ts";
import { CHAR_FORWARD_SLASH } from "../path/_constants.ts";
// fs.resolvePath([from ...], to)
/**
 * Resolves `pathSegments` into an absolute path.
 * @param pathSegments an array of path segments
 */
export function resolvePath(...pathSegments: string[]): string {
  let resolvedPath = "";
  let resolvedAbsolute = false;

  for (let i = pathSegments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
    let path: string;

    if (i >= 0) path = pathSegments[i];
    else {
      if (globalThis.Deno == null) {
        throw new TypeError("Resolved a relative path without a CWD.");
      }
      path = Deno.cwd();
    }

    assertPath(path);

    // Skip empty entries
    if (path.length === 0) {
      continue;
    }

    resolvedPath = `${path}/${resolvedPath}`;
    resolvedAbsolute = path.charCodeAt(0) === CHAR_FORWARD_SLASH;
  }

  // At this point the path should be resolved to a full absolute path, but
  // handle relative paths to be safe (might happen when process.cwd() fails)

  // Normalize the path
  resolvedPath = normalizeString(
    resolvedPath,
    !resolvedAbsolute,
    "/",
    isPosixPathSeparator,
  );

  if (resolvedAbsolute) {
    if (resolvedPath.length > 0) return `/${resolvedPath}`;
    else return "/";
  } else if (resolvedPath.length > 0) return resolvedPath;
  else return ".";
}
