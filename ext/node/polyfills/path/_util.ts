// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2025 the Deno authors. MIT license.

import type { FormatInputPathObject } from "ext:deno_node/path/_interface.ts";
import {
  CHAR_BACKWARD_SLASH,
  CHAR_DOT,
  CHAR_FORWARD_SLASH,
  CHAR_LOWERCASE_A,
  CHAR_LOWERCASE_Z,
  CHAR_UPPERCASE_A,
  CHAR_UPPERCASE_Z,
} from "ext:deno_node/path/_constants.ts";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import { primordials } from "ext:core/mod.js";
const {
  StringPrototypeCharCodeAt,
  StringPrototypeLastIndexOf,
  StringPrototypeSlice,
} = primordials;

export function assertPath(path: string) {
  if (typeof path !== "string") {
    throw new ERR_INVALID_ARG_TYPE("path", ["string"], path);
  }
}

export function isPosixPathSeparator(code: number): boolean {
  return code === CHAR_FORWARD_SLASH;
}

export function isPathSeparator(code: number): boolean {
  return isPosixPathSeparator(code) || code === CHAR_BACKWARD_SLASH;
}

export function isWindowsDeviceRoot(code: number): boolean {
  return (
    (code >= CHAR_LOWERCASE_A && code <= CHAR_LOWERCASE_Z) ||
    (code >= CHAR_UPPERCASE_A && code <= CHAR_UPPERCASE_Z)
  );
}

// Resolves . and .. elements in a path with directory names
export function normalizeString(
  path: string,
  allowAboveRoot: boolean,
  separator: string,
  isPathSeparator: (code: number) => boolean,
): string {
  let res = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let code: number | undefined;
  for (let i = 0, len = path.length; i <= len; ++i) {
    if (i < len) code = StringPrototypeCharCodeAt(path, i);
    else if (isPathSeparator(code!)) break;
    else code = CHAR_FORWARD_SLASH;

    if (isPathSeparator(code!)) {
      if (lastSlash === i - 1 || dots === 1) {
        // NOOP
      } else if (lastSlash !== i - 1 && dots === 2) {
        if (
          res.length < 2 ||
          lastSegmentLength !== 2 ||
          StringPrototypeCharCodeAt(res, res.length - 1) !== CHAR_DOT ||
          StringPrototypeCharCodeAt(res, res.length - 2) !== CHAR_DOT
        ) {
          if (res.length > 2) {
            const lastSlashIndex = StringPrototypeLastIndexOf(res, separator);
            if (lastSlashIndex === -1) {
              res = "";
              lastSegmentLength = 0;
            } else {
              res = StringPrototypeSlice(res, 0, lastSlashIndex);
              lastSegmentLength = res.length - 1 -
                StringPrototypeLastIndexOf(res, separator);
            }
            lastSlash = i;
            dots = 0;
            continue;
          } else if (res.length === 2 || res.length === 1) {
            res = "";
            lastSegmentLength = 0;
            lastSlash = i;
            dots = 0;
            continue;
          }
        }
        if (allowAboveRoot) {
          if (res.length > 0) res += `${separator}..`;
          else res = "..";
          lastSegmentLength = 2;
        }
      } else {
        if (res.length > 0) {
          res += separator + StringPrototypeSlice(path, lastSlash + 1, i);
        } else {
          res = StringPrototypeSlice(path, lastSlash + 1, i);
        }
        lastSegmentLength = i - lastSlash - 1;
      }
      lastSlash = i;
      dots = 0;
    } else if (code === CHAR_DOT && dots !== -1) {
      ++dots;
    } else {
      dots = -1;
    }
  }
  return res;
}

function formatExt(ext) {
  return ext ? `${ext[0] === "." ? "" : "."}${ext}` : "";
}

export function _format(
  sep: string,
  pathObject: FormatInputPathObject,
): string {
  const dir: string | undefined = pathObject.dir || pathObject.root;
  const base: string = pathObject.base ||
    (pathObject.name || "") + formatExt(pathObject.ext);
  if (!dir) return base;
  if (dir === pathObject.root) return dir + base;
  return dir + sep + base;
}
