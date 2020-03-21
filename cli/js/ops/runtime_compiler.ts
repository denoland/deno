// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsync } from "./dispatch_json.ts";

interface CompileRequest {
  rootName: string;
  sources?: Record<string, string>;
  options?: string;
  bundle: boolean;
}

export function compile(request: CompileRequest): Promise<string> {
  return sendAsync("op_compile", request);
}

interface TranspileRequest {
  sources: Record<string, string>;
  options?: string;
}

export function transpile(request: TranspileRequest): Promise<string> {
  return sendAsync("op_transpile", request);
}
