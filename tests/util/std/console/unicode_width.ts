// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from unicode_width rust crate, Copyright (c) 2015 The Rust Project Developers. MIT license.

import data from "./_data.json" assert { type: "json" };
import { runLengthDecode } from "./_rle.ts";

let tables: Uint8Array[] | null = null;
function lookupWidth(cp: number) {
  if (!tables) tables = data.tables.map(runLengthDecode);

  const t1Offset = tables[0][(cp >> 13) & 0xff];
  const t2Offset = tables[1][128 * t1Offset + ((cp >> 6) & 0x7f)];
  const packedWidths = tables[2][16 * t2Offset + ((cp >> 2) & 0xf)];

  const width = (packedWidths >> (2 * (cp & 0b11))) & 0b11;

  return width === 3 ? 1 : width;
}

const cache = new Map<string, number | null>();
function charWidth(ch: string) {
  if (cache.has(ch)) return cache.get(ch)!;

  const cp = ch.codePointAt(0)!;
  let v: number | null = null;

  if (cp < 0x7f) {
    v = cp >= 0x20 ? 1 : cp === 0 ? 0 : null;
  } else if (cp >= 0xa0) {
    v = lookupWidth(cp);
  } else {
    v = null;
  }

  cache.set(ch, v);
  return v;
}

/**
 * Get the width of a string's constituent characters in columns in TTY-like
 * environments.
 *
 * Combine with `stripColor` from `fmt/colors.ts` to get the expected physical
 * width of a string in the console.
 *
 * @example
 * ```ts
 * import { unicodeWidth } from "https://deno.land/std@$STD_VERSION/console/unicode_width.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 * import { stripColor } from "https://deno.land/std@$STD_VERSION/fmt/colors.ts";
 *
 * assertEquals(unicodeWidth("hello world"), 11);
 * assertEquals(unicodeWidth("天地玄黃宇宙洪荒"), 16);
 * assertEquals(unicodeWidth("ｆｕｌｌｗｉｄｔｈ"), 18);
 * assertEquals(unicodeWidth(stripColor("\x1b[36mголубой\x1b[39m")), 7);
 * assertEquals(unicodeWidth(stripColor("\x1b[31m紅色\x1b[39m")), 4);
 * assertEquals(unicodeWidth(stripColor("\x1B]8;;https://deno.land\x07🦕\x1B]8;;\x07")), 2);
 * ```
 */
export function unicodeWidth(str: string) {
  return [...str].map((ch) => charWidth(ch) ?? 0).reduce((a, b) => a + b, 0);
}
