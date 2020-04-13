// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { MediaType, SourceFile, SourceFileJson } from "./sourcefile.ts";
import { normalizeString, CHAR_FORWARD_SLASH } from "./util.ts";
import { cwd } from "../ops/fs/dir.ts";
import { assert } from "../util.ts";
import * as util from "../util.ts";
import * as compilerOps from "../ops/compiler.ts";

function resolvePath(...pathSegments: string[]): string {
  let resolvedPath = "";
  let resolvedAbsolute = false;

  for (let i = pathSegments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
    let path: string;

    if (i >= 0) path = pathSegments[i];
    else path = cwd();

    // Skip empty entries
    if (path.length === 0) {
      continue;
    }

    resolvedPath = `${path}/${resolvedPath}`;
    resolvedAbsolute = path.charCodeAt(0) === CHAR_FORWARD_SLASH;
  }

  // At this point the path should be resolved to a full absolute path, but
  // handle relative paths to be safe (might happen when cwd() fails)

  // Normalize the path
  resolvedPath = normalizeString(
    resolvedPath,
    !resolvedAbsolute,
    "/",
    (code) => code === CHAR_FORWARD_SLASH
  );

  if (resolvedAbsolute) {
    if (resolvedPath.length > 0) return `/${resolvedPath}`;
    else return "/";
  } else if (resolvedPath.length > 0) return resolvedPath;
  else return ".";
}

function resolveSpecifier(specifier: string, referrer: string): string {
  if (!specifier.startsWith(".")) {
    return specifier;
  }
  const pathParts = referrer.split("/");
  pathParts.pop();
  let path = pathParts.join("/");
  path = path.endsWith("/") ? path : `${path}/`;
  return resolvePath(path, specifier);
}

export function resolveModules(
  specifiers: string[],
  referrer?: string
): string[] {
  util.log("compiler_imports::resolveModules", { specifiers, referrer });
  return compilerOps.resolveModules(specifiers, referrer);
}

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
    case "json":
      return MediaType.Json;
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
