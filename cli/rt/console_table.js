// Copyright Joyent, Inc. and other Node contributors. MIT license.
// Forked from Node's lib/internal/cli_table.js
System.register(
  "$deno$/web/console_table.ts",
  ["$deno$/web/text_encoding.ts", "$deno$/web/util.ts"],
  function (exports_33, context_33) {
    "use strict";
    let text_encoding_ts_3, util_ts_5, encoder, tableChars, colorRegExp;
    const __moduleName = context_33 && context_33.id;
    function removeColors(str) {
      return str.replace(colorRegExp, "");
    }
    function countBytes(str) {
      const normalized = removeColors(String(str)).normalize("NFC");
      return encoder.encode(normalized).byteLength;
    }
    function renderRow(row, columnWidths) {
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
    function cliTable(head, columns) {
      const rows = [];
      const columnWidths = head.map((h) => countBytes(h));
      const longestColumn = columns.reduce((n, a) => Math.max(n, a.length), 0);
      for (let i = 0; i < head.length; i++) {
        const column = columns[i];
        for (let j = 0; j < longestColumn; j++) {
          if (rows[j] === undefined) {
            rows[j] = [];
          }
          const value = (rows[j][i] = util_ts_5.hasOwnProperty(column, j)
            ? column[j]
            : "");
          const width = columnWidths[i] || 0;
          const counted = countBytes(value);
          columnWidths[i] = Math.max(width, counted);
        }
      }
      const divider = columnWidths.map((i) =>
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
    exports_33("cliTable", cliTable);
    return {
      setters: [
        function (text_encoding_ts_3_1) {
          text_encoding_ts_3 = text_encoding_ts_3_1;
        },
        function (util_ts_5_1) {
          util_ts_5 = util_ts_5_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_3.TextEncoder();
        tableChars = {
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
        colorRegExp = /\u001b\[\d\d?m/g;
      },
    };
  }
);
