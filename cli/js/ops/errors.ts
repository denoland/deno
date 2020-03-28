// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { DiagnosticItem } from "../diagnostics.ts";
import { sendSync } from "./dispatch_json.ts";

export function formatDiagnostics(items: DiagnosticItem[]): string {
  return sendSync("op_format_diagnostic", { items });
}

export interface Location {
  filename: string;

  line: number;

  column: number;
}

export function applySourceMap(location: Location): Location {
  const { filename, line, column } = location;
  // On this side, line/column are 1 based, but in the source maps, they are
  // 0 based, so we have to convert back and forth
  const res = sendSync("op_apply_source_map", {
    filename,
    line: line - 1,
    column: column - 1,
  });
  return {
    filename: res.filename,
    line: res.line + 1,
    column: res.column + 1,
  };
}
