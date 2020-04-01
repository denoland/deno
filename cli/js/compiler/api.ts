// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.

import { DiagnosticItem } from "../diagnostics.ts";
import * as util from "../util.ts";
import * as runtimeCompilerOps from "../ops/runtime_compiler.ts";

export interface CompilerOptions {
  allowJs?: boolean;

  allowSyntheticDefaultImports?: boolean;

  allowUmdGlobalAccess?: boolean;

  allowUnreachableCode?: boolean;

  allowUnusedLabels?: boolean;

  alwaysStrict?: boolean;

  baseUrl?: string;

  checkJs?: boolean;

  declaration?: boolean;

  declarationDir?: string;

  declarationMap?: boolean;

  downlevelIteration?: boolean;

  emitBOM?: boolean;

  emitDeclarationOnly?: boolean;

  emitDecoratorMetadata?: boolean;

  esModuleInterop?: boolean;

  experimentalDecorators?: boolean;

  inlineSourceMap?: boolean;

  inlineSources?: boolean;

  isolatedModules?: boolean;

  jsx?: "react" | "preserve" | "react-native";

  jsxFactory?: string;

  keyofStringsOnly?: string;

  useDefineForClassFields?: boolean;

  lib?: string[];

  locale?: string;

  mapRoot?: string;

  module?:
    | "none"
    | "commonjs"
    | "amd"
    | "system"
    | "umd"
    | "es6"
    | "es2015"
    | "esnext";

  noEmitHelpers?: boolean;

  noFallthroughCasesInSwitch?: boolean;

  noImplicitAny?: boolean;

  noImplicitReturns?: boolean;

  noImplicitThis?: boolean;

  noImplicitUseStrict?: boolean;

  noResolve?: boolean;

  noStrictGenericChecks?: boolean;

  noUnusedLocals?: boolean;

  noUnusedParameters?: boolean;

  outDir?: string;

  paths?: Record<string, string[]>;

  preserveConstEnums?: boolean;

  removeComments?: boolean;

  resolveJsonModule?: boolean;

  rootDir?: string;

  rootDirs?: string[];

  sourceMap?: boolean;

  sourceRoot?: string;

  strict?: boolean;

  strictBindCallApply?: boolean;

  strictFunctionTypes?: boolean;

  strictPropertyInitialization?: boolean;

  strictNullChecks?: boolean;

  suppressExcessPropertyErrors?: boolean;

  suppressImplicitAnyIndexErrors?: boolean;

  target?:
    | "es3"
    | "es5"
    | "es6"
    | "es2015"
    | "es2016"
    | "es2017"
    | "es2018"
    | "es2019"
    | "es2020"
    | "esnext";

  types?: string[];
}

function checkRelative(specifier: string): string {
  return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
    ? specifier
    : `./${specifier}`;
}

export interface TranspileOnlyResult {
  source: string;
  map?: string;
}

export async function transpileOnly(
  sources: Record<string, string>,
  options: CompilerOptions = {}
): Promise<Record<string, TranspileOnlyResult>> {
  util.log("Deno.transpileOnly", { sources: Object.keys(sources), options });
  const payload = {
    sources,
    options: JSON.stringify(options),
  };
  const result = await runtimeCompilerOps.transpile(payload);
  return JSON.parse(result);
}

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
  return JSON.parse(result);
}

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
  return JSON.parse(result);
}
