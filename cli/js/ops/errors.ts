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
  const res = sendSync("op_apply_source_map", location);
  return {
    fileName: res.fileName,
    lineNumber: res.lineNumber,
    columnNumber: res.columnNumber,
  };
}
