// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// This module is browser compatible.

export function stripTrailingSeparators(
  segment: string,
  isSep: (char: number) => boolean,
): string {
  if (segment.length <= 1) {
    return segment;
  }

  let end = segment.length;

  for (let i = segment.length - 1; i > 0; i--) {
    if (isSep(segment.charCodeAt(i))) {
      end = i;
    } else {
      break;
    }
  }

  return segment.slice(0, end);
}
