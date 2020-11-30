// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
/** This module is browser compatible. */

import {
  assertPath,
  isPathSeparator,
  isWindowsDeviceRoot,
  normalizeString,
} from "../path/_util.ts";
import { CHAR_COLON } from "../path/_constants.ts";
/**
 * Resolves path segments into a `path`
 * @param pathSegments to process to path
 */
export function resolvePath(...pathSegments: string[]): string {
  let resolvedDevice = "";
  let resolvedTail = "";
  let resolvedAbsolute = false;

  for (let i = pathSegments.length - 1; i >= -1; i--) {
    let path: string;
    if (i >= 0) {
      path = pathSegments[i];
    } else if (!resolvedDevice) {
      if (globalThis.Deno == null) {
        throw new TypeError("Resolved a drive-letter-less path without a CWD.");
      }
      path = Deno.cwd();
    } else {
      if (globalThis.Deno == null) {
        throw new TypeError("Resolved a relative path without a CWD.");
      }
      // Windows has the concept of drive-specific current working
      // directories. If we've resolved a drive letter but not yet an
      // absolute path, get cwd for that drive, or the process cwd if
      // the drive cwd is not available. We're sure the device is not
      // a UNC path at this points, because UNC paths are always absolute.
      path = Deno.env.get(`=${resolvedDevice}`) || Deno.cwd();

      // Verify that a cwd was found and that it actually points
      // to our drive. If not, default to the drive's root.
      if (
        path === undefined ||
        path.slice(0, 3).toLowerCase() !== `${resolvedDevice.toLowerCase()}\\`
      ) {
        path = `${resolvedDevice}\\`;
      }
    }

    assertPath(path);

    const len = path.length;

    // Skip empty entries
    if (len === 0) continue;

    let rootEnd = 0;
    let device = "";
    let isAbsolute = false;
    const code = path.charCodeAt(0);

    // Try to match a root
    if (len > 1) {
      if (isPathSeparator(code)) {
        // Possible UNC root

        // If we started with a separator, we know we at least have an
        // absolute path of some kind (UNC or otherwise)
        isAbsolute = true;

        if (isPathSeparator(path.charCodeAt(1))) {
          // Matched double path separator at beginning
          let j = 2;
          let last = j;
          // Match 1 or more non-path separators
          for (; j < len; ++j) {
            if (isPathSeparator(path.charCodeAt(j))) break;
          }
          if (j < len && j !== last) {
            const firstPart = path.slice(last, j);
            // Matched!
            last = j;
            // Match 1 or more path separators
            for (; j < len; ++j) {
              if (!isPathSeparator(path.charCodeAt(j))) break;
            }
            if (j < len && j !== last) {
              // Matched!
              last = j;
              // Match 1 or more non-path separators
              for (; j < len; ++j) {
                if (isPathSeparator(path.charCodeAt(j))) break;
              }
              if (j === len) {
                // We matched a UNC root only
                device = `\\\\${firstPart}\\${path.slice(last)}`;
                rootEnd = j;
              } else if (j !== last) {
                // We matched a UNC root with leftovers

                device = `\\\\${firstPart}\\${path.slice(last, j)}`;
                rootEnd = j;
              }
            }
          }
        } else {
          rootEnd = 1;
        }
      } else if (isWindowsDeviceRoot(code)) {
        // Possible device root

        if (path.charCodeAt(1) === CHAR_COLON) {
          device = path.slice(0, 2);
          rootEnd = 2;
          if (len > 2) {
            if (isPathSeparator(path.charCodeAt(2))) {
              // Treat separator following drive name as an absolute path
              // indicator
              isAbsolute = true;
              rootEnd = 3;
            }
          }
        }
      }
    } else if (isPathSeparator(code)) {
      // `path` contains just a path separator
      rootEnd = 1;
      isAbsolute = true;
    }

    if (
      device.length > 0 &&
      resolvedDevice.length > 0 &&
      device.toLowerCase() !== resolvedDevice.toLowerCase()
    ) {
      // This path points to another device so it is not applicable
      continue;
    }

    if (resolvedDevice.length === 0 && device.length > 0) {
      resolvedDevice = device;
    }
    if (!resolvedAbsolute) {
      resolvedTail = `${path.slice(rootEnd)}\\${resolvedTail}`;
      resolvedAbsolute = isAbsolute;
    }

    if (resolvedAbsolute && resolvedDevice.length > 0) break;
  }

  // At this point the path should be resolved to a full absolute path,
  // but handle relative paths to be safe (might happen when process.cwd()
  // fails)

  // Normalize the tail path
  resolvedTail = normalizeString(
    resolvedTail,
    !resolvedAbsolute,
    "\\",
    isPathSeparator,
  );

  return resolvedDevice + (resolvedAbsolute ? "\\" : "") + resolvedTail || ".";
}
