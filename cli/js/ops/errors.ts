// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "../diagnostics.ts";
import { sendSync } from "./dispatch_json.ts";

export function formatDiagnostics(items: DiagnosticItem[]): string {
  return sendSync("op_format_diagnostic", { items });
}

export interface Location {
  fileName: string;
  lineNumber: number;
  columnNumber: number;
}

export function applySourceMap(location: Location): Location {
  const { fileName, lineNumber, columnNumber } = location;
  // On this side, line/column are 1 based, but in the source maps, they are
  // 0 based, so we have to convert back and forth
  const res = sendSync("op_apply_source_map", {
    fileName,
    lineNumber: lineNumber - 1,
    columnNumber: columnNumber - 1,
  });
  return {
    fileName: res.fileName,
    lineNumber: res.lineNumber + 1,
    columnNumber: res.columnNumber + 1,
  };
}
