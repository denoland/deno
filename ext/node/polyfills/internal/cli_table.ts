// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { getStringWidth } from "ext:deno_node/internal/util/inspect.mjs";
import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  SafeArrayIterator,
  StringPrototypeRepeat,
  MathCeil,
  MathMax,
  ObjectHasOwn,
} = primordials;

const tableChars = {
  middleMiddle: "\u2500",
  rowMiddle: "\u253c",
  topRight: "\u2510",
  topLeft: "\u250c",
  leftMiddle: "\u251c",
  topMiddle: "\u252c",
  bottomRight: "\u2518",
  bottomLeft: "\u2514",
  bottomMiddle: "\u2534",
  rightMiddle: "\u2524",
  left: "\u2502 ",
  right: " \u2502",
  middle: " \u2502 ",
};

const renderRow = (row: string[], columnWidths: number[]) => {
  let out = tableChars.left;
  for (let i = 0; i < row.length; i++) {
    const cell = row[i];
    const len = getStringWidth(cell);
    const needed = columnWidths[i] - len;
    // round(needed) + ceil(needed) will always add up to the amount
    // of spaces we need while also left justifying the output.
    out += cell + StringPrototypeRepeat(" ", MathCeil(needed));
    if (i !== row.length - 1) {
      out += tableChars.middle;
    }
  }
  out += tableChars.right;
  return out;
};

const table = (head: string[], columns: string[][]) => {
  const rows: string[][] = [];
  const columnWidths = ArrayPrototypeMap(head, (h) => getStringWidth(h));
  const longestColumn = MathMax(
    ...new SafeArrayIterator(ArrayPrototypeMap(columns, (a) => a.length)),
  );

  for (let i = 0; i < head.length; i++) {
    const column = columns[i];
    for (let j = 0; j < longestColumn; j++) {
      if (rows[j] === undefined) {
        rows[j] = [];
      }
      const value = rows[j][i] = ObjectHasOwn(column, j) ? column[j] : "";
      const width = columnWidths[i] || 0;
      const counted = getStringWidth(value);
      columnWidths[i] = MathMax(width, counted);
    }
  }

  const divider = ArrayPrototypeMap(
    columnWidths,
    (i) => StringPrototypeRepeat(tableChars.middleMiddle, i + 2),
  );

  let result = tableChars.topLeft +
    ArrayPrototypeJoin(divider, tableChars.topMiddle) +
    tableChars.topRight + "\n" +
    renderRow(head, columnWidths) + "\n" +
    tableChars.leftMiddle +
    ArrayPrototypeJoin(divider, tableChars.rowMiddle) +
    tableChars.rightMiddle + "\n";

  for (const row of new SafeArrayIterator(rows)) {
    result += `${renderRow(row, columnWidths)}\n`;
  }

  result += tableChars.bottomLeft +
    ArrayPrototypeJoin(divider, tableChars.bottomMiddle) +
    tableChars.bottomRight;

  return result;
};
export default table;
