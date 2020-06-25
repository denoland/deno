// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsync } from "./dispatch_json.ts";
import { DiagnosticItem } from "../diagnostics.ts";

interface CompileRequest {
  rootName: string;
  sources?: Record<string, string>;
  options?: string;
  bundle: boolean;
}

interface CompileResponse {
  diagnostics: DiagnosticItem[];
  output?: string;
  emitMap?: Record<string, Record<string, string>>;
}

export function compile(request: CompileRequest): Promise<CompileResponse> {
  return sendAsync("op_compile", request);
}

interface TranspileRequest {
  sources: Record<string, string>;
  options?: string;
}

export interface TranspileOnlyResult {
  source: string;
  map?: string;
}

export function transpile(
  request: TranspileRequest
): Promise<Record<string, TranspileOnlyResult>> {
  return sendAsync("op_transpile", request);
}
