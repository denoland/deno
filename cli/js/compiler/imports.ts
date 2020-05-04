// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { MediaType, SourceFile, SourceFileJson } from "./sourcefile.ts";
import { assert } from "../util.ts";
import * as util from "../util.ts";
import * as compilerOps from "../ops/compiler.ts";

function resolveSpecifier(specifier: string, referrer: string): string {
  // The resolveModules op only handles fully qualified URLs for referrer.
  // However we will have cases where referrer is "/foo.ts". We add this dummy
  // prefix "file://" in order to use the op.
  // TODO(ry) Maybe we should perhaps ModuleSpecifier::resolve_import() to
  // handle this situation.
  let dummyPrefix = false;
  const prefix = "file://";
  if (referrer.startsWith("/")) {
    dummyPrefix = true;
    referrer = prefix + referrer;
  }
  let r = resolveModules([specifier], referrer)[0];
  if (dummyPrefix) {
    r = r.replace(prefix, "");
  }
  return r;
}

// TODO(ry) Remove. Unnecessary redirection to compilerOps.resolveModules.
export function resolveModules(
  specifiers: string[],
  referrer?: string
): string[] {
  util.log("compiler_imports::resolveModules", { specifiers, referrer });
  return compilerOps.resolveModules(specifiers, referrer);
}

// TODO(ry) Remove. Unnecessary redirection to compilerOps.fetchSourceFiles.
function fetchSourceFiles(
  specifiers: string[],
  referrer?: string
): Promise<SourceFileJson[]> {
  util.log("compiler_imports::fetchSourceFiles", { specifiers, referrer });
  return compilerOps.fetchSourceFiles(specifiers, referrer);
}

function getMediaType(filename: string): MediaType {
  const maybeExtension = /\.([a-zA-Z]+)$/.exec(filename);
  if (!maybeExtension) {
    util.log(`!!! Could not identify valid extension: "${filename}"`);
    return MediaType.Unknown;
  }
  const [, extension] = maybeExtension;
  switch (extension.toLowerCase()) {
    case "js":
      return MediaType.JavaScript;
    case "jsx":
      return MediaType.JSX;
    case "ts":
      return MediaType.TypeScript;
    case "tsx":
      return MediaType.TSX;
    case "wasm":
      return MediaType.Wasm;
    default:
      util.log(`!!! Unknown extension: "${extension}"`);
      return MediaType.Unknown;
  }
}

export function processLocalImports(
  sources: Record<string, string>,
  specifiers: Array<[string, string]>,
  referrer?: string,
  processJsImports = false
): string[] {
  if (!specifiers.length) {
    return [];
  }
  const moduleNames = specifiers.map(
    referrer
      ? ([, specifier]): string => resolveSpecifier(specifier, referrer)
      : ([, specifier]): string => specifier
  );
  for (let i = 0; i < moduleNames.length; i++) {
    const moduleName = moduleNames[i];
    assert(moduleName in sources, `Missing module in sources: "${moduleName}"`);
    const sourceFile =
      SourceFile.get(moduleName) ||
      new SourceFile({
        url: moduleName,
        filename: moduleName,
        sourceCode: sources[moduleName],
        mediaType: getMediaType(moduleName),
      });
    sourceFile.cache(specifiers[i][0], referrer);
    if (!sourceFile.processed) {
      processLocalImports(
        sources,
        sourceFile.imports(processJsImports),
        sourceFile.url,
        processJsImports
      );
    }
  }
  return moduleNames;
}

export async function processImports(
  specifiers: Array<[string, string]>,
  referrer?: string,
  processJsImports = false
): Promise<string[]> {
  if (!specifiers.length) {
    return [];
  }
  const sources = specifiers.map(([, moduleSpecifier]) => moduleSpecifier);
  const resolvedSources = resolveModules(sources, referrer);
  const sourceFiles = await fetchSourceFiles(resolvedSources, referrer);
  assert(sourceFiles.length === specifiers.length);
  for (let i = 0; i < sourceFiles.length; i++) {
    const sourceFileJson = sourceFiles[i];
    const sourceFile =
      SourceFile.get(sourceFileJson.url) || new SourceFile(sourceFileJson);
    sourceFile.cache(specifiers[i][0], referrer);
    if (!sourceFile.processed) {
      await processImports(
        sourceFile.imports(processJsImports),
        sourceFile.url,
        processJsImports
      );
    }
  }
  return resolvedSources;
}
