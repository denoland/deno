// Copyright Joyent, Inc. and other Node contributors. MIT license.
// Forked from Node's lib/internal/cli_table.js

import { TextEncoder } from "./text_encoding.ts";
import { hasOwnProperty } from "./util.ts";

const encoder = new TextEncoder();

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

const colorRegExp = /\u001b\[\d\d?m/g;

function removeColors(str: string): string {
  return str.replace(colorRegExp, "");
}

function countBytes(str: string): number {
  const normalized = removeColors(String(str)).normalize("NFC");

  return encoder.encode(normalized).byteLength;
}

function renderRow(row: string[], columnWidths: number[]): string {
  let out = tableChars.left;
  for (let i = 0; i < row.length; i++) {
    const cell = row[i];
    const len = countBytes(cell);
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
  const columnWidths = head.map((h: string): number => countBytes(h));
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
      const counted = countBytes(value);
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
