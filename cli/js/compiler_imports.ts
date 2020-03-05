// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  MediaType,
  SourceFile,
  SourceFileJson
} from "./compiler_sourcefile.ts";
import { normalizeString, CHAR_FORWARD_SLASH } from "./compiler_util.ts";
import { cwd } from "./dir.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import { assert } from "./util.ts";
import * as util from "./util.ts";

/** Resolve a path to the final path segment passed. */
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
    code => code === CHAR_FORWARD_SLASH
  );

  if (resolvedAbsolute) {
    if (resolvedPath.length > 0) return `/${resolvedPath}`;
    else return "/";
  } else if (resolvedPath.length > 0) return resolvedPath;
  else return ".";
}

/** Resolve a relative specifier based on the referrer.  Used when resolving
 * modules internally within the runtime compiler API. */
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

/** Ops to Rust to resolve modules' URLs. */
export function resolveModules(
  specifiers: string[],
  referrer?: string
): string[] {
  util.log("compiler_imports::resolveModules", { specifiers, referrer });
  return sendSync("op_resolve_modules", { specifiers, referrer });
}

/** Ops to Rust to fetch modules meta data. */
function fetchSourceFiles(
  specifiers: string[],
  referrer?: string
): Promise<SourceFileJson[]> {
  util.log("compiler_imports::fetchSourceFiles", { specifiers, referrer });
  return sendAsync("op_fetch_source_files", {
    specifiers,
    referrer
  });
}

/** Given a filename, determine the media type based on extension.  Used when
 * resolving modules internally in a runtime compile. */
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

/** Recursively process the imports of modules from within the supplied sources,
 * generating `SourceFile`s of any imported files.
 *
 * Specifiers are supplied in an array of tuples where the first is the
 * specifier that will be requested in the code and the second is the specifier
 * that should be actually resolved. */
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
        mediaType: getMediaType(moduleName)
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

/** Recursively process the imports of modules, generating `SourceFile`s of any
 * imported files.
 *
 * Specifiers are supplied in an array of tuples where the first is the
 * specifier that will be requested in the code and the second is the specifier
 * that should be actually resolved. */
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
