// Copyright Joyent, Inc. and other Node contributors. MIT license.
// Forked from Node's lib/internal/cli_table.js

import { hasOwnProperty } from "./util.ts";
import { stripColor } from "../colors.ts";

const tableChars = {
  middleMiddle: "─",
  rowMiddle: "┼",
  topRight: "┐",
  topLeft: "┌",
  leftMiddle: "├",
  topMiddle: "┬",
  bottomRight: "┘",
  bottomLeft: "└",
  bottomMiddle: "┴",
  rightMiddle: "┤",
  left: "│ ",
  right: " │",
  middle: " │ ",
};

function isFullWidthCodePoint(code: number): boolean {
  // Code points are partially derived from:
  // http://www.unicode.org/Public/UNIDATA/EastAsianWidth.txt
  return (
    code >= 0x1100 &&
    (code <= 0x115f || // Hangul Jamo
    code === 0x2329 || // LEFT-POINTING ANGLE BRACKET
    code === 0x232a || // RIGHT-POINTING ANGLE BRACKET
      // CJK Radicals Supplement .. Enclosed CJK Letters and Months
      (code >= 0x2e80 && code <= 0x3247 && code !== 0x303f) ||
      // Enclosed CJK Letters and Months .. CJK Unified Ideographs Extension A
      (code >= 0x3250 && code <= 0x4dbf) ||
      // CJK Unified Ideographs .. Yi Radicals
      (code >= 0x4e00 && code <= 0xa4c6) ||
      // Hangul Jamo Extended-A
      (code >= 0xa960 && code <= 0xa97c) ||
      // Hangul Syllables
      (code >= 0xac00 && code <= 0xd7a3) ||
      // CJK Compatibility Ideographs
      (code >= 0xf900 && code <= 0xfaff) ||
      // Vertical Forms
      (code >= 0xfe10 && code <= 0xfe19) ||
      // CJK Compatibility Forms .. Small Form Variants
      (code >= 0xfe30 && code <= 0xfe6b) ||
      // Halfwidth and Fullwidth Forms
      (code >= 0xff01 && code <= 0xff60) ||
      (code >= 0xffe0 && code <= 0xffe6) ||
      // Kana Supplement
      (code >= 0x1b000 && code <= 0x1b001) ||
      // Enclosed Ideographic Supplement
      (code >= 0x1f200 && code <= 0x1f251) ||
      // Miscellaneous Symbols and Pictographs 0x1f300 - 0x1f5ff
      // Emoticons 0x1f600 - 0x1f64f
      (code >= 0x1f300 && code <= 0x1f64f) ||
      // CJK Unified Ideographs Extension B .. Tertiary Ideographic Plane
      (code >= 0x20000 && code <= 0x3fffd))
  );
}

function getStringWidth(str: string): number {
  str = stripColor(str).normalize("NFC");
  let width = 0;

  for (const ch of str) {
    width += isFullWidthCodePoint(ch.codePointAt(0)!) ? 2 : 1;
  }

  return width;
}

function renderRow(row: string[], columnWidths: number[]): string {
  let out = tableChars.left;
  for (let i = 0; i < row.length; i++) {
    const cell = row[i];
    const len = getStringWidth(cell);
    const needed = (columnWidths[i] - len) / 2;
    // round(needed) + ceil(needed) will always add up to the amount
    // of spaces we need while also left justifying the output.
    out += `${" ".repeat(needed)}${cell}${" ".repeat(Math.ceil(needed))}`;
    if (i !== row.length - 1) {
      out += tableChars.middle;
    }
  }
  out += tableChars.right;
  return out;
}

export function cliTable(head: string[], columns: string[][]): string {
  const rows: string[][] = [];
  const columnWidths = head.map((h: string): number => getStringWidth(h));
  const longestColumn = columns.reduce(
    (n: number, a: string[]): number => Math.max(n, a.length),
    0
  );

  for (let i = 0; i < head.length; i++) {
    const column = columns[i];
    for (let j = 0; j < longestColumn; j++) {
      if (rows[j] === undefined) {
        rows[j] = [];
      }
      const value = (rows[j][i] = hasOwnProperty(column, j) ? column[j] : "");
      const width = columnWidths[i] || 0;
      const counted = getStringWidth(value);
      columnWidths[i] = Math.max(width, counted);
    }
  }

  const divider = columnWidths.map((i: number): string =>
    tableChars.middleMiddle.repeat(i + 2)
  );

  let result =
    `${tableChars.topLeft}${divider.join(tableChars.topMiddle)}` +
    `${tableChars.topRight}\n${renderRow(head, columnWidths)}\n` +
    `${tableChars.leftMiddle}${divider.join(tableChars.rowMiddle)}` +
    `${tableChars.rightMiddle}\n`;

  for (const row of rows) {
    result += `${renderRow(row, columnWidths)}\n`;
  }

  result +=
    `${tableChars.bottomLeft}${divider.join(tableChars.bottomMiddle)}` +
    tableChars.bottomRight;

  return result;
}
