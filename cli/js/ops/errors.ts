// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "../diagnostics.ts";
import { core } from "../core.ts";

export function formatDiagnostics(items: DiagnosticItem[]): string {
  return core.dispatchJson.sendSync("op_format_diagnostic", { items });
}

export interface Location {
  fileName: string;
  lineNumber: number;
  columnNumber: number;
}

export function applySourceMap(location: Location): Location {
  const { fileName, lineNumber, columnNumber } = location;
  const res = core.dispatchJson.sendSync("op_apply_source_map", {
    fileName,
    lineNumber: lineNumber,
    columnNumber: columnNumber,
  });
  return {
    fileName: res.fileName,
    lineNumber: res.lineNumber,
    columnNumber: res.columnNumber,
  };
}
