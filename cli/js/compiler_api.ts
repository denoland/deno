// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.

import { DiagnosticItem } from "./diagnostics.ts";
import * as util from "./util.ts";
import * as runtimeCompilerOps from "./ops/runtime_compiler.ts";
import { TranspileOnlyResult } from "./ops/runtime_compiler.ts";
import { CompilerOptions } from "./compiler_options.ts";

function checkRelative(specifier: string): string {
  return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
    ? specifier
    : `./${specifier}`;
}

// TODO(bartlomieju): change return type to interface?
export function transpileOnly(
  sources: Record<string, string>,
  options: CompilerOptions = {}
): Promise<Record<string, TranspileOnlyResult>> {
  util.log("Deno.transpileOnly", { sources: Object.keys(sources), options });
  const payload = {
    sources,
    options: JSON.stringify(options),
  };
  return runtimeCompilerOps.transpile(payload);
}

// TODO(bartlomieju): change return type to interface?
export async function compile(
  rootName: string,
  sources?: Record<string, string>,
  options: CompilerOptions = {}
): Promise<[DiagnosticItem[] | undefined, Record<string, string>]> {
  const payload = {
    rootName: sources ? rootName : checkRelative(rootName),
    sources,
    options: JSON.stringify(options),
    bundle: false,
  };
  util.log("Deno.compile", {
    rootName: payload.rootName,
    sources: !!sources,
    options,
  });
  const result = await runtimeCompilerOps.compile(payload);
  util.assert(result.emitMap);
  const maybeDiagnostics =
    result.diagnostics.length === 0 ? undefined : result.diagnostics;

  const emitMap: Record<string, string> = {};

  for (const [key, emmitedSource] of Object.entries(result.emitMap)) {
    emitMap[key] = emmitedSource.contents;
  }

  return [maybeDiagnostics, emitMap];
}

// TODO(bartlomieju): change return type to interface?
export async function bundle(
  rootName: string,
  sources?: Record<string, string>,
  options: CompilerOptions = {}
): Promise<[DiagnosticItem[] | undefined, string]> {
  const payload = {
    rootName: sources ? rootName : checkRelative(rootName),
    sources,
    options: JSON.stringify(options),
    bundle: true,
  };
  util.log("Deno.bundle", {
    rootName: payload.rootName,
    sources: !!sources,
    options,
  });
  const result = await runtimeCompilerOps.compile(payload);
  util.assert(result.output);
  const maybeDiagnostics =
    result.diagnostics.length === 0 ? undefined : result.diagnostics;
  return [maybeDiagnostics, result.output];
}
