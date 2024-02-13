// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { assertPath } from "./assert_path.ts";

export function stripSuffix(name: string, suffix: string): string {
  if (suffix.length >= name.length) {
    return name;
  }

  const lenDiff = name.length - suffix.length;

  for (let i = suffix.length - 1; i >= 0; --i) {
    if (name.charCodeAt(lenDiff + i) !== suffix.charCodeAt(i)) {
      return name;
    }
  }

  return name.slice(0, -suffix.length);
}

export function lastPathSegment(
  path: string,
  isSep: (char: number) => boolean,
  start = 0,
): string {
  let matchedNonSeparator = false;
  let end = path.length;

  for (let i = path.length - 1; i >= start; --i) {
    if (isSep(path.charCodeAt(i))) {
      if (matchedNonSeparator) {
        start = i + 1;
        break;
      }
    } else if (!matchedNonSeparator) {
      matchedNonSeparator = true;
      end = i + 1;
    }
  }

  return path.slice(start, end);
}

export function assertArgs(path: string, suffix: string) {
  assertPath(path);
  if (path.length === 0) return path;
  if (typeof suffix !== "string") {
    throw new TypeError(
      `Suffix must be a string. Received ${JSON.stringify(suffix)}`,
    );
  }
}
