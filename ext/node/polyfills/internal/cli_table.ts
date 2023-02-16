// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { getStringWidth } from "internal:deno_node/polyfills/internal/util/inspect.mjs";

// The use of Unicode characters below is the only non-comment use of non-ASCII
// Unicode characters in Node.js built-in modules. If they are ever removed or
// rewritten with \u escapes, then a test will need to be (re-)added to Node.js
// core to verify that Unicode characters work in built-ins.
// Refs: https://github.com/nodejs/node/issues/10673
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

const renderRow = (row: string[], columnWidths: number[]) => {
  let out = tableChars.left;
  for (let i = 0; i < row.length; i++) {
    const cell = row[i];
    const len = getStringWidth(cell);
    const needed = (columnWidths[i] - len) / 2;
    // round(needed) + ceil(needed) will always add up to the amount
    // of spaces we need while also left justifying the output.
    out += " ".repeat(needed) + cell +
      " ".repeat(Math.ceil(needed));
    if (i !== row.length - 1) {
      out += tableChars.middle;
    }
  }
  out += tableChars.right;
  return out;
};

const table = (head: string[], columns: string[][]) => {
  const rows: string[][] = [];
  const columnWidths = head.map((h) => getStringWidth(h));
  const longestColumn = Math.max(...columns.map((a) => a.length));

  for (let i = 0; i < head.length; i++) {
    const column = columns[i];
    for (let j = 0; j < longestColumn; j++) {
      if (rows[j] === undefined) {
        rows[j] = [];
      }
      const value = rows[j][i] = Object.hasOwn(column, j) ? column[j] : "";
      const width = columnWidths[i] || 0;
      const counted = getStringWidth(value);
      columnWidths[i] = Math.max(width, counted);
    }
  }

  const divider = columnWidths.map((i) =>
    tableChars.middleMiddle.repeat(i + 2)
  );

  let result = tableChars.topLeft +
    divider.join(tableChars.topMiddle) +
    tableChars.topRight + "\n" +
    renderRow(head, columnWidths) + "\n" +
    tableChars.leftMiddle +
    divider.join(tableChars.rowMiddle) +
    tableChars.rightMiddle + "\n";

  for (const row of rows) {
    result += `${renderRow(row, columnWidths)}\n`;
  }

  result += tableChars.bottomLeft +
    divider.join(tableChars.bottomMiddle) +
    tableChars.bottomRight;

  return result;
};
export default table;
