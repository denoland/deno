// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

const WHITESPACE_ENCODINGS: Record<string, string> = {
  "\u0009": "%09",
  "\u000A": "%0A",
  "\u000B": "%0B",
  "\u000C": "%0C",
  "\u000D": "%0D",
  "\u0020": "%20",
};

export function encodeWhitespace(string: string): string {
  return string.replaceAll(/[\s]/g, (c) => {
    return WHITESPACE_ENCODINGS[c] ?? c;
  });
}
