// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2025 the Deno authors. MIT license.

import type {
  FormatInputPathObject,
  ParsedPath,
} from "ext:deno_node/path/_interface.ts";
import {
  CHAR_BACKWARD_SLASH,
  CHAR_COLON,
  CHAR_DOT,
  CHAR_QUESTION_MARK,
} from "ext:deno_node/path/_constants.ts";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";

import {
  _format,
  assertPath,
  isPathSeparator,
  isPosixPathSeparator,
  isWindowsDeviceRoot,
  normalizeString,
} from "ext:deno_node/path/_util.ts";
import { assert } from "ext:deno_node/_util/asserts.ts";
import { core, primordials } from "ext:core/mod.js";
import process from "node:process";
import type * as fsGlob from "ext:deno_node/_fs/_fs_glob.ts";

const lazyLoadGlob = core.createLazyLoader<typeof fsGlob>(
  "ext:deno_node/_fs/_fs_glob.ts",
);

const {
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypePop,
  ArrayPrototypeSlice,
  StringPrototypeCharCodeAt,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeRepeat,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
  StringPrototypeToUpperCase,
  TypeError,
} = primordials;

export const sep = "\\";
export const delimiter = ";";

const WINDOWS_RESERVED_NAMES = [
  "CON",
  "PRN",
  "AUX",
  "NUL",
  "COM1",
  "COM2",
  "COM3",
  "COM4",
  "COM5",
  "COM6",
  "COM7",
  "COM8",
  "COM9",
  "LPT1",
  "LPT2",
  "LPT3",
  "LPT4",
  "LPT5",
  "LPT6",
  "LPT7",
  "LPT8",
  "LPT9",
  "COM\xb9",
  "COM\xb2",
  "COM\xb3",
  "LPT\xb9",
  "LPT\xb2",
  "LPT\xb3",
];

function isWindowsReservedName(path: string, colonIndex: number): boolean {
  const devicePart = StringPrototypeToUpperCase(
    StringPrototypeSlice(path, 0, colonIndex),
  );
  return ArrayPrototypeIncludes(WINDOWS_RESERVED_NAMES, devicePart);
}

/**
 * Resolves path segments into a `path`
 * @param pathSegments to process to path
 */
export function resolve(...pathSegments: string[]): string {
  let resolvedDevice = "";
  let resolvedTail = "";
  let resolvedAbsolute = false;

  for (let i = pathSegments.length - 1; i >= -1; i--) {
    let path: string;
    // deno-lint-ignore no-explicit-any
    const { Deno } = globalThis as any;
    if (i >= 0) {
      path = pathSegments[i];
    } else if (!resolvedDevice) {
      if (typeof Deno?.cwd !== "function") {
        throw new TypeError("Resolved a drive-letter-less path without a CWD.");
      }
      path = Deno.cwd();
      if (
        pathSegments.length === 0 ||
        ((pathSegments.length === 1 &&
          (pathSegments[0] === "" || pathSegments[0] === ".")) &&
          isPathSeparator(StringPrototypeCharCodeAt(path, 0)))
      ) {
        return path;
      }
    } else {
      if (
        typeof Deno?.env?.get !== "function" || typeof Deno?.cwd !== "function"
      ) {
        throw new TypeError("Resolved a relative path without a CWD.");
      }
      // Windows has the concept of drive-specific current working
      // directories. If we've resolved a drive letter but not yet an
      // absolute path, get cwd for that drive, or the process cwd if
      // the drive cwd is not available. We're sure the device is not
      // a UNC path at this points, because UNC paths are always absolute.
      path = process.env[`=${resolvedDevice}`] || Deno.cwd();

      // Verify that a cwd was found and that it actually points
      // to our drive. If not, default to the drive's root.
      if (
        path === undefined ||
        (StringPrototypeToLowerCase(StringPrototypeSlice(path, 0, 2)) !==
            StringPrototypeToLowerCase(resolvedDevice) &&
          StringPrototypeCharCodeAt(path, 2) === CHAR_BACKWARD_SLASH)
      ) {
        path = `${resolvedDevice}\\`;
      }
    }

    assertPath(path);

    const len = path.length;
    let rootEnd = 0;
    let device = "";
    let isAbsolute = false;
    const code = StringPrototypeCharCodeAt(path, 0);

    // Try to match a root
    if (len === 1) {
      if (isPathSeparator(code)) {
        // `path` contains just a path separator
        rootEnd = 1;
        isAbsolute = true;
      }
    } else if (isPathSeparator(code)) {
      // Possible UNC root

      // If we started with a separator, we know we at least have an
      // absolute path of some kind (UNC or otherwise)
      isAbsolute = true;

      if (isPathSeparator(StringPrototypeCharCodeAt(path, 1))) {
        // Matched double path separator at beginning
        let j = 2;
        let last = j;
        // Match 1 or more non-path separators
        while (
          j < len &&
          !isPathSeparator(StringPrototypeCharCodeAt(path, j))
        ) {
          j++;
        }
        if (j < len && j !== last) {
          const firstPart = StringPrototypeSlice(path, last, j);
          // Matched!
          last = j;
          // Match 1 or more path separators
          while (
            j < len &&
            isPathSeparator(StringPrototypeCharCodeAt(path, j))
          ) {
            j++;
          }
          if (j < len && j !== last) {
            // Matched!
            last = j;
            // Match 1 or more non-path separators
            while (
              j < len &&
              !isPathSeparator(StringPrototypeCharCodeAt(path, j))
            ) {
              j++;
            }
            if (j === len || j !== last) {
              if (firstPart !== "." && firstPart !== "?") {
                // We matched a UNC root
                device = `\\\\${firstPart}\\${
                  StringPrototypeSlice(path, last, j)
                }`;
                rootEnd = j;
              } else {
                // We matched a device root (e.g. \\\\.\\PHYSICALDRIVE0)
                device = `\\\\${firstPart}`;
                rootEnd = 4;
              }
            }
          }
        }
      } else {
        rootEnd = 1;
      }
    } else if (
      isWindowsDeviceRoot(code) &&
      StringPrototypeCharCodeAt(path, 1) === CHAR_COLON
    ) {
      // Possible device root
      device = StringPrototypeSlice(path, 0, 2);
      rootEnd = 2;
      if (len > 2 && isPathSeparator(StringPrototypeCharCodeAt(path, 2))) {
        // Treat separator following drive name as an absolute path
        // indicator
        isAbsolute = true;
        rootEnd = 3;
      }
    }

    if (device.length > 0) {
      if (resolvedDevice.length > 0) {
        if (
          StringPrototypeToLowerCase(device) !==
            StringPrototypeToLowerCase(resolvedDevice)
        ) {
          // This path points to another device so it is not applicable
          continue;
        }
      } else {
        resolvedDevice = device;
      }
    }

    if (resolvedAbsolute) {
      if (resolvedDevice.length > 0) {
        break;
      }
    } else {
      resolvedTail = `${StringPrototypeSlice(path, rootEnd)}\\${resolvedTail}`;
      resolvedAbsolute = isAbsolute;
      if (isAbsolute && resolvedDevice.length > 0) {
        break;
      }
    }
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

  return resolvedAbsolute
    ? `${resolvedDevice}\\${resolvedTail}`
    : `${resolvedDevice}${resolvedTail}` || ".";
}

/**
 * Normalizes a `path`
 * @param path to normalize
 */
export function normalize(path: string): string {
  assertPath(path);
  const len = path.length;
  if (len === 0) return ".";
  let rootEnd = 0;
  let device: string | undefined;
  let isAbsolute = false;
  const code = StringPrototypeCharCodeAt(path, 0);

  // Try to match a root
  if (len === 1) {
    // `path` contains just a single char, exit early to avoid
    // unnecessary work
    return isPosixPathSeparator(code) ? "\\" : path;
  }
  if (isPathSeparator(code)) {
    // Possible UNC root

    // If we started with a separator, we know we at least have an absolute
    // path of some kind (UNC or otherwise)
    isAbsolute = true;

    if (isPathSeparator(StringPrototypeCharCodeAt(path, 1))) {
      // Matched double path separator at beginning
      let j = 2;
      let last = j;
      // Match 1 or more non-path separators
      while (
        j < len &&
        !isPathSeparator(StringPrototypeCharCodeAt(path, j))
      ) {
        j++;
      }
      if (j < len && j !== last) {
        const firstPart = StringPrototypeSlice(path, last, j);
        // Matched!
        last = j;
        // Match 1 or more path separators
        while (
          j < len &&
          isPathSeparator(StringPrototypeCharCodeAt(path, j))
        ) {
          j++;
        }
        if (j < len && j !== last) {
          // Matched!
          last = j;
          // Match 1 or more non-path separators
          while (
            j < len &&
            !isPathSeparator(StringPrototypeCharCodeAt(path, j))
          ) {
            j++;
          }
          if (j === len || j !== last) {
            if (firstPart === "." || firstPart === "?") {
              // We matched a device root (e.g. \\\\.\\PHYSICALDRIVE0)
              device = `\\\\${firstPart}`;
              rootEnd = 4;
              const colonIndex = StringPrototypeIndexOf(path, ":");
              // Special case: handle \\?\COM1: or similar reserved device paths
              const possibleDevice = StringPrototypeSlice(
                path,
                4,
                colonIndex + 1,
              );
              if (
                isWindowsReservedName(possibleDevice, possibleDevice.length - 1)
              ) {
                device = `\\\\?\\${possibleDevice}`;
                rootEnd = 4 + possibleDevice.length;
              }
            } else if (j === len) {
              // We matched a UNC root only
              // Return the normalized version of the UNC root since there
              // is nothing left to process
              return `\\\\${firstPart}\\${StringPrototypeSlice(path, last)}\\`;
            } else {
              // We matched a UNC root with leftovers
              device = `\\\\${firstPart}\\${
                StringPrototypeSlice(path, last, j)
              }`;
              rootEnd = j;
            }
          }
        }
      }
    } else {
      rootEnd = 1;
    }
  } else {
    const colonIndex = StringPrototypeIndexOf(path, ":");
    if (colonIndex > 0) {
      if (isWindowsDeviceRoot(code) && colonIndex === 1) {
        device = StringPrototypeSlice(path, 0, 2);
        rootEnd = 2;
        if (len > 2 && isPathSeparator(StringPrototypeCharCodeAt(path, 2))) {
          isAbsolute = true;
          rootEnd = 3;
        }
      } else if (isWindowsReservedName(path, colonIndex)) {
        device = StringPrototypeSlice(path, 0, colonIndex + 1);
        rootEnd = colonIndex + 1;
      }
    }
  }

  let tail = rootEnd < len
    ? normalizeString(
      StringPrototypeSlice(path, rootEnd),
      !isAbsolute,
      "\\",
      isPathSeparator,
    )
    : "";
  if (tail.length === 0 && !isAbsolute) {
    tail = ".";
  }
  if (
    tail.length > 0 &&
    isPathSeparator(StringPrototypeCharCodeAt(path, len - 1))
  ) {
    tail += "\\";
  }
  if (
    !isAbsolute && device === undefined && StringPrototypeIncludes(path, ":")
  ) {
    // If the original path was not absolute and if we have not been able to
    // resolve it relative to a particular device, we need to ensure that the
    // `tail` has not become something that Windows might interpret as an
    // absolute path. See CVE-2024-36139.
    if (
      tail.length >= 2 &&
      isWindowsDeviceRoot(StringPrototypeCharCodeAt(tail, 0)) &&
      StringPrototypeCharCodeAt(tail, 1) === CHAR_COLON
    ) {
      return `.\\${tail}`;
    }
    let index = StringPrototypeIndexOf(path, ":");

    do {
      if (
        index === len - 1 ||
        isPathSeparator(StringPrototypeCharCodeAt(path, index + 1))
      ) {
        return `.\\${tail}`;
      }
    } while ((index = StringPrototypeIndexOf(path, ":", index + 1)) !== -1);
  }
  const colonIndex = StringPrototypeIndexOf(path, ":");
  if (isWindowsReservedName(path, colonIndex)) {
    return `.\\${device ?? ""}${tail}`;
  }
  if (device === undefined) {
    return isAbsolute ? `\\${tail}` : tail;
  }
  return isAbsolute ? `${device}\\${tail}` : `${device}${tail}`;
}

/**
 * Verifies whether path is absolute
 * @param path to verify
 */
export function isAbsolute(path: string): boolean {
  assertPath(path);
  const len = path.length;
  if (len === 0) return false;

  const code = StringPrototypeCharCodeAt(path, 0);
  if (isPathSeparator(code)) {
    return true;
  } else if (isWindowsDeviceRoot(code)) {
    // Possible device root

    if (len > 2 && StringPrototypeCharCodeAt(path, 1) === CHAR_COLON) {
      if (isPathSeparator(StringPrototypeCharCodeAt(path, 2))) return true;
    }
  }
  return false;
}

/**
 * Join all given a sequence of `paths`,then normalizes the resulting path.
 * @param paths to be joined and normalized
 */
export function join(...paths: string[]): string {
  const pathsCount = paths.length;
  if (pathsCount === 0) return ".";

  let joined: string | undefined;
  let firstPart: string | null = null;
  for (let i = 0; i < pathsCount; ++i) {
    const path = paths[i];
    assertPath(path);
    if (path.length > 0) {
      if (joined === undefined) joined = firstPart = path;
      else joined += `\\${path}`;
    }
  }

  if (joined === undefined) return ".";

  // Make sure that the joined path doesn't start with two slashes, because
  // normalize() will mistake it for an UNC path then.
  //
  // This step is skipped when it is very clear that the user actually
  // intended to point at an UNC path. This is assumed when the first
  // non-empty string arguments starts with exactly two slashes followed by
  // at least one more non-slash character.
  //
  // Note that for normalize() to treat a path as an UNC path it needs to
  // have at least 2 components, so we don't filter for that here.
  // This means that the user can use join to construct UNC paths from
  // a server name and a share name; for example:
  //   path.join('//server', 'share') -> '\\\\server\\share\\')
  let needsReplace = true;
  let slashCount = 0;
  assert(firstPart != null);
  if (isPathSeparator(StringPrototypeCharCodeAt(firstPart, 0))) {
    ++slashCount;
    const firstLen = firstPart.length;
    if (firstLen > 1) {
      if (isPathSeparator(StringPrototypeCharCodeAt(firstPart, 1))) {
        ++slashCount;
        if (firstLen > 2) {
          if (isPathSeparator(StringPrototypeCharCodeAt(firstPart, 2))) {
            ++slashCount;
          } else {
            // We matched a UNC path in the first part
            needsReplace = false;
          }
        }
      }
    }
  }
  if (needsReplace) {
    // Find any more consecutive slashes we need to replace
    for (; slashCount < joined.length; ++slashCount) {
      if (!isPathSeparator(StringPrototypeCharCodeAt(joined, slashCount))) {
        break;
      }
    }

    // Replace the slashes if needed
    if (slashCount >= 2) {
      joined = `\\${StringPrototypeSlice(joined, slashCount)}`;
    }
  }

  return normalize(joined);
}

/**
 * It will solve the relative path from `from` to `to`, for instance:
 *  from = 'C:\\orandea\\test\\aaa'
 *  to = 'C:\\orandea\\impl\\bbb'
 * The output of the function should be: '..\\..\\impl\\bbb'
 * @param from relative path
 * @param to relative path
 */
export function relative(from: string, to: string): string {
  assertPath(from);
  assertPath(to);

  if (from === to) return "";

  const fromOrig = resolve(from);
  const toOrig = resolve(to);

  if (fromOrig === toOrig) return "";

  from = StringPrototypeToLowerCase(fromOrig);
  to = StringPrototypeToLowerCase(toOrig);

  if (from === to) return "";

  if (fromOrig.length !== from.length || toOrig.length !== to.length) {
    const fromSplit = StringPrototypeSplit(fromOrig, "\\");
    const toSplit = StringPrototypeSplit(toOrig, "\\");
    if (fromSplit[fromSplit.length - 1] === "") {
      ArrayPrototypePop(fromSplit);
    }
    if (toSplit[toSplit.length - 1] === "") {
      ArrayPrototypePop(toSplit);
    }

    const fromLen = fromSplit.length;
    const toLen = toSplit.length;
    const length = fromLen < toLen ? fromLen : toLen;

    let i;
    for (i = 0; i < length; i++) {
      if (
        StringPrototypeToLowerCase(fromSplit[i]) !==
          StringPrototypeToLowerCase(toSplit[i])
      ) {
        break;
      }
    }

    if (i === 0) {
      return toOrig;
    } else if (i === length) {
      if (toLen > length) {
        return ArrayPrototypeJoin(ArrayPrototypeSlice(toSplit, i), "\\");
      }
      if (fromLen > length) {
        return StringPrototypeRepeat("..\\", fromLen - 1 - i) + "..";
      }
      return "";
    }

    return StringPrototypeRepeat("..\\", fromLen - i) +
      ArrayPrototypeJoin(ArrayPrototypeSlice(toSplit, i), "\\");
  }

  // Trim any leading backslashes
  let fromStart = 0;
  while (
    fromStart < from.length &&
    StringPrototypeCharCodeAt(from, fromStart) === CHAR_BACKWARD_SLASH
  ) {
    fromStart++;
  }
  // Trim trailing backslashes (applicable to UNC paths only)
  let fromEnd = from.length;
  while (
    fromEnd - 1 > fromStart &&
    StringPrototypeCharCodeAt(from, fromEnd - 1) === CHAR_BACKWARD_SLASH
  ) {
    fromEnd--;
  }
  const fromLen = fromEnd - fromStart;

  // Trim any leading backslashes
  let toStart = 0;
  while (
    toStart < to.length &&
    StringPrototypeCharCodeAt(to, toStart) === CHAR_BACKWARD_SLASH
  ) {
    toStart++;
  }
  // Trim trailing backslashes (applicable to UNC paths only)
  let toEnd = to.length;
  while (
    toEnd - 1 > toStart &&
    StringPrototypeCharCodeAt(to, toEnd - 1) === CHAR_BACKWARD_SLASH
  ) {
    toEnd--;
  }
  const toLen = toEnd - toStart;

  // Compare paths to find the longest common path from root
  const length = fromLen < toLen ? fromLen : toLen;
  let lastCommonSep = -1;
  let i = 0;
  for (; i < length; i++) {
    const fromCode = StringPrototypeCharCodeAt(from, fromStart + i);
    if (fromCode !== StringPrototypeCharCodeAt(to, toStart + i)) {
      break;
    } else if (fromCode === CHAR_BACKWARD_SLASH) {
      lastCommonSep = i;
    }
  }

  // We found a mismatch before the first common path separator was seen, so
  // return the original `to`.
  if (i !== length) {
    if (lastCommonSep === -1) {
      return toOrig;
    }
  } else {
    if (toLen > length) {
      if (
        StringPrototypeCharCodeAt(to, toStart + i) ===
          CHAR_BACKWARD_SLASH
      ) {
        // We get here if `from` is the exact base path for `to`.
        // For example: from='C:\\foo\\bar'; to='C:\\foo\\bar\\baz'
        return StringPrototypeSlice(toOrig, toStart + i + 1);
      }
      if (i === 2) {
        // We get here if `from` is the device root.
        // For example: from='C:\\'; to='C:\\foo'
        return StringPrototypeSlice(toOrig, toStart + i);
      }
    }
    if (fromLen > length) {
      if (
        StringPrototypeCharCodeAt(from, fromStart + i) ===
          CHAR_BACKWARD_SLASH
      ) {
        // We get here if `to` is the exact base path for `from`.
        // For example: from='C:\\foo\\bar'; to='C:\\foo'
        lastCommonSep = i;
      } else if (i === 2) {
        // We get here if `to` is the device root.
        // For example: from='C:\\foo\\bar'; to='C:\\'
        lastCommonSep = 3;
      }
    }
    if (lastCommonSep === -1) {
      lastCommonSep = 0;
    }
  }

  let out = "";
  // Generate the relative path based on the path difference between `to` and
  // `from`
  for (i = fromStart + lastCommonSep + 1; i <= fromEnd; ++i) {
    if (
      i === fromEnd ||
      StringPrototypeCharCodeAt(from, i) === CHAR_BACKWARD_SLASH
    ) {
      out += out.length === 0 ? ".." : "\\..";
    }
  }

  toStart += lastCommonSep;

  // Lastly, append the rest of the destination (`to`) path that comes after
  // the common path parts
  if (out.length > 0) {
    return out + StringPrototypeSlice(toOrig, toStart, toEnd);
  }
  if (StringPrototypeCharCodeAt(toOrig, toStart) === CHAR_BACKWARD_SLASH) {
    ++toStart;
  }
  return StringPrototypeSlice(toOrig, toStart, toEnd);
}

/**
 * Resolves path to a namespace path
 * @param path to resolve to namespace
 */
export function toNamespacedPath(path: string): string {
  // Note: this will *probably* throw somewhere.
  if (typeof path !== "string") return path;
  if (path.length === 0) return "";

  const resolvedPath = resolve(path);

  if (resolvedPath.length <= 2) {
    return path;
  }

  if (StringPrototypeCharCodeAt(resolvedPath, 0) === CHAR_BACKWARD_SLASH) {
    // Possible UNC root

    if (StringPrototypeCharCodeAt(resolvedPath, 1) === CHAR_BACKWARD_SLASH) {
      const code = StringPrototypeCharCodeAt(resolvedPath, 2);
      if (code !== CHAR_QUESTION_MARK && code !== CHAR_DOT) {
        // Matched non-long UNC root, convert the path to a long UNC path
        return `\\\\?\\UNC\\${StringPrototypeSlice(resolvedPath, 2)}`;
      }
    }
  } else if (
    isWindowsDeviceRoot(StringPrototypeCharCodeAt(resolvedPath, 0)) &&
    StringPrototypeCharCodeAt(resolvedPath, 1) === CHAR_COLON &&
    StringPrototypeCharCodeAt(resolvedPath, 2) === CHAR_BACKWARD_SLASH
  ) {
    // Matched device root, convert the path to a long UNC path
    return `\\\\?\\${resolvedPath}`;
  }

  return resolvedPath;
}

/**
 * Return the directory name of a `path`.
 * @param path to determine name for
 */
export function dirname(path: string): string {
  assertPath(path);
  const len = path.length;
  if (len === 0) return ".";
  let rootEnd = -1;
  let end = -1;
  let matchedSlash = true;
  let offset = 0;
  const code = StringPrototypeCharCodeAt(path, 0);

  // Try to match a root
  if (len > 1) {
    if (isPathSeparator(code)) {
      // Possible UNC root

      rootEnd = offset = 1;

      if (isPathSeparator(StringPrototypeCharCodeAt(path, 1))) {
        // Matched double path separator at beginning
        let j = 2;
        let last = j;
        // Match 1 or more non-path separators
        for (; j < len; ++j) {
          if (isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
        }
        if (j < len && j !== last) {
          // Matched!
          last = j;
          // Match 1 or more path separators
          for (; j < len; ++j) {
            if (!isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
          }
          if (j < len && j !== last) {
            // Matched!
            last = j;
            // Match 1 or more non-path separators
            for (; j < len; ++j) {
              if (isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
            }
            if (j === len) {
              // We matched a UNC root only
              return path;
            }
            if (j !== last) {
              // We matched a UNC root with leftovers

              // Offset by 1 to include the separator after the UNC root to
              // treat it as a "normal root" on top of a (UNC) root
              rootEnd = offset = j + 1;
            }
          }
        }
      }
    } else if (isWindowsDeviceRoot(code)) {
      // Possible device root

      if (StringPrototypeCharCodeAt(path, 1) === CHAR_COLON) {
        rootEnd = offset = 2;
        if (len > 2) {
          if (isPathSeparator(StringPrototypeCharCodeAt(path, 2))) {
            rootEnd = offset = 3;
          }
        }
      }
    }
  } else if (isPathSeparator(code)) {
    // `path` contains just a path separator, exit early to avoid
    // unnecessary work
    return path;
  }

  for (let i = len - 1; i >= offset; --i) {
    if (isPathSeparator(StringPrototypeCharCodeAt(path, i))) {
      if (!matchedSlash) {
        end = i;
        break;
      }
    } else {
      // We saw the first non-path separator
      matchedSlash = false;
    }
  }

  if (end === -1) {
    if (rootEnd === -1) return ".";
    else end = rootEnd;
  }
  return StringPrototypeSlice(path, 0, end);
}

/**
 * Return the last portion of a `path`. Trailing directory separators are ignored.
 * @param path to process
 * @param ext of path directory
 */
export function basename(path: string, ext = ""): string {
  if (ext !== undefined && typeof ext !== "string") {
    throw new ERR_INVALID_ARG_TYPE("ext", ["string"], ext);
  }

  assertPath(path);

  let start = 0;
  let end = -1;
  let matchedSlash = true;
  let i: number;

  // Check for a drive letter prefix so as not to mistake the following
  // path separator as an extra separator at the end of the path that can be
  // disregarded
  if (path.length >= 2) {
    const drive = StringPrototypeCharCodeAt(path, 0);
    if (isWindowsDeviceRoot(drive)) {
      if (StringPrototypeCharCodeAt(path, 1) === CHAR_COLON) start = 2;
    }
  }

  if (ext !== undefined && ext.length > 0 && ext.length <= path.length) {
    if (ext.length === path.length && ext === path) return "";
    let extIdx = ext.length - 1;
    let firstNonSlashEnd = -1;
    for (i = path.length - 1; i >= start; --i) {
      const code = StringPrototypeCharCodeAt(path, i);
      if (isPathSeparator(code)) {
        // If we reached a path separator that was not part of a set of path
        // separators at the end of the string, stop now
        if (!matchedSlash) {
          start = i + 1;
          break;
        }
      } else {
        if (firstNonSlashEnd === -1) {
          // We saw the first non-path separator, remember this index in case
          // we need it if the extension ends up not matching
          matchedSlash = false;
          firstNonSlashEnd = i + 1;
        }
        if (extIdx >= 0) {
          // Try to match the explicit extension
          if (code === StringPrototypeCharCodeAt(ext, extIdx)) {
            if (--extIdx === -1) {
              // We matched the extension, so mark this as the end of our path
              // component
              end = i;
            }
          } else {
            // Extension does not match, so our result is the entire path
            // component
            extIdx = -1;
            end = firstNonSlashEnd;
          }
        }
      }
    }

    if (start === end) end = firstNonSlashEnd;
    else if (end === -1) end = path.length;
    return StringPrototypeSlice(path, start, end);
  } else {
    for (i = path.length - 1; i >= start; --i) {
      if (isPathSeparator(StringPrototypeCharCodeAt(path, i))) {
        // If we reached a path separator that was not part of a set of path
        // separators at the end of the string, stop now
        if (!matchedSlash) {
          start = i + 1;
          break;
        }
      } else if (end === -1) {
        // We saw the first non-path separator, mark this as the end of our
        // path component
        matchedSlash = false;
        end = i + 1;
      }
    }

    if (end === -1) return "";
    return StringPrototypeSlice(path, start, end);
  }
}

/**
 * Return the extension of the `path`.
 * @param path with extension
 */
export function extname(path: string): string {
  assertPath(path);
  let start = 0;
  let startDot = -1;
  let startPart = 0;
  let end = -1;
  let matchedSlash = true;
  // Track the state of characters (if any) we see before our first dot and
  // after any path separator we find
  let preDotState = 0;

  // Check for a drive letter prefix so as not to mistake the following
  // path separator as an extra separator at the end of the path that can be
  // disregarded

  if (
    path.length >= 2 &&
    StringPrototypeCharCodeAt(path, 1) === CHAR_COLON &&
    isWindowsDeviceRoot(StringPrototypeCharCodeAt(path, 0))
  ) {
    start = startPart = 2;
  }

  for (let i = path.length - 1; i >= start; --i) {
    const code = StringPrototypeCharCodeAt(path, i);
    if (isPathSeparator(code)) {
      // If we reached a path separator that was not part of a set of path
      // separators at the end of the string, stop now
      if (!matchedSlash) {
        startPart = i + 1;
        break;
      }
      continue;
    }
    if (end === -1) {
      // We saw the first non-path separator, mark this as the end of our
      // extension
      matchedSlash = false;
      end = i + 1;
    }
    if (code === CHAR_DOT) {
      // If this is our first dot, mark it as the start of our extension
      if (startDot === -1) startDot = i;
      else if (preDotState !== 1) preDotState = 1;
    } else if (startDot !== -1) {
      // We saw a non-dot and non-path separator before our dot, so we should
      // have a good chance at having a non-empty extension
      preDotState = -1;
    }
  }

  if (
    startDot === -1 ||
    end === -1 ||
    // We saw a non-dot character immediately before the dot
    preDotState === 0 ||
    // The (right-most) trimmed path component is exactly '..'
    (preDotState === 1 && startDot === end - 1 && startDot === startPart + 1)
  ) {
    return "";
  }
  return StringPrototypeSlice(path, startDot, end);
}

/**
 * Generate a path from `FormatInputPathObject` object.
 * @param pathObject with path
 */
export function format(pathObject: FormatInputPathObject): string {
  if (pathObject === null || typeof pathObject !== "object") {
    throw new ERR_INVALID_ARG_TYPE("pathObject", ["Object"], pathObject);
  }
  return _format("\\", pathObject);
}

/**
 * Return a `ParsedPath` object of the `path`.
 * @param path to process
 */
export function parse(path: string): ParsedPath {
  assertPath(path);

  const ret: ParsedPath = { root: "", dir: "", base: "", ext: "", name: "" };

  const len = path.length;
  if (len === 0) return ret;

  let rootEnd = 0;
  let code = StringPrototypeCharCodeAt(path, 0);

  // Try to match a root
  if (len > 1) {
    if (isPathSeparator(code)) {
      // Possible UNC root

      rootEnd = 1;
      if (isPathSeparator(StringPrototypeCharCodeAt(path, 1))) {
        // Matched double path separator at beginning
        let j = 2;
        let last = j;
        // Match 1 or more non-path separators
        for (; j < len; ++j) {
          if (isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
        }
        if (j < len && j !== last) {
          // Matched!
          last = j;
          // Match 1 or more path separators
          for (; j < len; ++j) {
            if (!isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
          }
          if (j < len && j !== last) {
            // Matched!
            last = j;
            // Match 1 or more non-path separators
            for (; j < len; ++j) {
              if (isPathSeparator(StringPrototypeCharCodeAt(path, j))) break;
            }
            if (j === len) {
              // We matched a UNC root only

              rootEnd = j;
            } else if (j !== last) {
              // We matched a UNC root with leftovers

              rootEnd = j + 1;
            }
          }
        }
      }
    } else if (isWindowsDeviceRoot(code)) {
      // Possible device root

      if (StringPrototypeCharCodeAt(path, 1) === CHAR_COLON) {
        rootEnd = 2;
        if (len > 2) {
          if (isPathSeparator(StringPrototypeCharCodeAt(path, 2))) {
            if (len === 3) {
              // `path` contains just a drive root, exit early to avoid
              // unnecessary work
              ret.root = ret.dir = path;
              return ret;
            }
            rootEnd = 3;
          }
        } else {
          // `path` contains just a drive root, exit early to avoid
          // unnecessary work
          ret.root = ret.dir = path;
          return ret;
        }
      }
    }
  } else if (isPathSeparator(code)) {
    // `path` contains just a path separator, exit early to avoid
    // unnecessary work
    ret.root = ret.dir = path;
    return ret;
  }

  if (rootEnd > 0) ret.root = StringPrototypeSlice(path, 0, rootEnd);

  let startDot = -1;
  let startPart = rootEnd;
  let end = -1;
  let matchedSlash = true;
  let i = path.length - 1;

  // Track the state of characters (if any) we see before our first dot and
  // after any path separator we find
  let preDotState = 0;

  // Get non-dir info
  for (; i >= rootEnd; --i) {
    code = StringPrototypeCharCodeAt(path, i);
    if (isPathSeparator(code)) {
      // If we reached a path separator that was not part of a set of path
      // separators at the end of the string, stop now
      if (!matchedSlash) {
        startPart = i + 1;
        break;
      }
      continue;
    }
    if (end === -1) {
      // We saw the first non-path separator, mark this as the end of our
      // extension
      matchedSlash = false;
      end = i + 1;
    }
    if (code === CHAR_DOT) {
      // If this is our first dot, mark it as the start of our extension
      if (startDot === -1) startDot = i;
      else if (preDotState !== 1) preDotState = 1;
    } else if (startDot !== -1) {
      // We saw a non-dot and non-path separator before our dot, so we should
      // have a good chance at having a non-empty extension
      preDotState = -1;
    }
  }

  if (
    startDot === -1 ||
    end === -1 ||
    // We saw a non-dot character immediately before the dot
    preDotState === 0 ||
    // The (right-most) trimmed path component is exactly '..'
    (preDotState === 1 && startDot === end - 1 && startDot === startPart + 1)
  ) {
    if (end !== -1) {
      ret.base = ret.name = StringPrototypeSlice(path, startPart, end);
    }
  } else {
    ret.name = StringPrototypeSlice(path, startPart, startDot);
    ret.base = StringPrototypeSlice(path, startPart, end);
    ret.ext = StringPrototypeSlice(path, startDot, end);
  }

  // If the directory is the root, use the entire root as the `dir` including
  // the trailing slash if any (`C:\abc` -> `C:\`). Otherwise, strip out the
  // trailing slash (`C:\abc\def` -> `C:\abc`).
  if (startPart > 0 && startPart !== rootEnd) {
    ret.dir = StringPrototypeSlice(path, 0, startPart - 1);
  } else ret.dir = ret.root;

  return ret;
}

export const _makeLong = toNamespacedPath;

let lazyMatchGlobPattern: typeof fsGlob.matchGlobPattern;
export const matchesGlob = (path: string, pattern: string): boolean => {
  lazyMatchGlobPattern ??= lazyLoadGlob().matchGlobPattern;
  return lazyMatchGlobPattern(path, pattern, true);
};

export default {
  basename,
  delimiter,
  dirname,
  extname,
  format,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toNamespacedPath,
  _makeLong,
  matchesGlob,
};
