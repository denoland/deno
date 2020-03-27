// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { getMappedModuleName, parseTypeDirectives } from "./type_directives.ts";
import { assert, log } from "../util.ts";

// Warning! The values in this enum are duplicated in `cli/msg.rs`
// Update carefully!
export enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  TSX = 3,
  Json = 4,
  Wasm = 5,
  Unknown = 6,
}

export interface SourceFileJson {
  url: string;
  filename: string;
  mediaType: MediaType;
  sourceCode: string;
}

export const ASSETS = "$asset$";

function getExtension(fileName: string, mediaType: MediaType): ts.Extension {
  switch (mediaType) {
    case MediaType.JavaScript:
      return ts.Extension.Js;
    case MediaType.JSX:
      return ts.Extension.Jsx;
    case MediaType.TypeScript:
      return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
    case MediaType.TSX:
      return ts.Extension.Tsx;
    case MediaType.Json:
      // we internally compile JSON, so what gets provided to the TypeScript
      // compiler is an ES module, but in order to get TypeScript to handle it
      // properly we have to pretend it is TS.
      return ts.Extension.Ts;
    case MediaType.Wasm:
      // Custom marker for Wasm type.
      return ts.Extension.Js;
    case MediaType.Unknown:
    default:
      throw TypeError(
        `Cannot resolve extension for "${fileName}" with mediaType "${MediaType[mediaType]}".`
      );
  }
}

/** A global cache of module source files that have been loaded. */
const moduleCache: Map<string, SourceFile> = new Map();
/** A map of maps which cache source files for quicker modules resolution. */
const specifierCache: Map<string, Map<string, SourceFile>> = new Map();

export class SourceFile {
  extension!: ts.Extension;
  filename!: string;

  importedFiles?: Array<[string, string]>;

  mediaType!: MediaType;
  processed = false;
  sourceCode?: string;
  tsSourceFile?: ts.SourceFile;
  url!: string;

  constructor(json: SourceFileJson) {
    if (moduleCache.has(json.url)) {
      throw new TypeError("SourceFile already exists");
    }
    Object.assign(this, json);
    this.extension = getExtension(this.url, this.mediaType);
    moduleCache.set(this.url, this);
  }

  cache(moduleSpecifier: string, containingFile?: string): void {
    containingFile = containingFile || "";
    let innerCache = specifierCache.get(containingFile);
    if (!innerCache) {
      innerCache = new Map();
      specifierCache.set(containingFile, innerCache);
    }
    innerCache.set(moduleSpecifier, this);
  }

  imports(processJsImports: boolean): Array<[string, string]> {
    if (this.processed) {
      throw new Error("SourceFile has already been processed.");
    }
    assert(this.sourceCode != null);
    // we shouldn't process imports for files which contain the nocheck pragma
    // (like bundles)
    if (this.sourceCode.match(/\/{2}\s+@ts-nocheck/)) {
      log(`Skipping imports for "${this.filename}"`);
      return [];
    }

    const preProcessedFileInfo = ts.preProcessFile(
      this.sourceCode,
      true,
      this.mediaType === MediaType.JavaScript ||
        this.mediaType === MediaType.JSX
    );
    this.processed = true;
    const files = (this.importedFiles = [] as Array<[string, string]>);

    function process(references: Array<{ fileName: string }>): void {
      for (const { fileName } of references) {
        files.push([fileName, fileName]);
      }
    }

    const {
      importedFiles,
      referencedFiles,
      libReferenceDirectives,
      typeReferenceDirectives,
    } = preProcessedFileInfo;
    const typeDirectives = parseTypeDirectives(this.sourceCode);
    if (typeDirectives) {
      for (const importedFile of importedFiles) {
        files.push([
          importedFile.fileName,
          getMappedModuleName(importedFile, typeDirectives),
        ]);
      }
    } else if (
      !(
        !processJsImports &&
        (this.mediaType === MediaType.JavaScript ||
          this.mediaType === MediaType.JSX)
      )
    ) {
      process(importedFiles);
    }
    process(referencedFiles);
    // built in libs comes across as `"dom"` for example, and should be filtered
    // out during pre-processing as they are either already cached or they will
    // be lazily fetched by the compiler host.  Ones that contain full files are
    // not filtered out and will be fetched as normal.
    process(
      libReferenceDirectives.filter(
        ({ fileName }) => !ts.libMap.has(fileName.toLowerCase())
      )
    );
    process(typeReferenceDirectives);
    return files;
  }

  static getUrl(
    moduleSpecifier: string,
    containingFile: string
  ): string | undefined {
    const containingCache = specifierCache.get(containingFile);
    if (containingCache) {
      const sourceFile = containingCache.get(moduleSpecifier);
      return sourceFile && sourceFile.url;
    }
    return undefined;
  }

  static get(url: string): SourceFile | undefined {
    return moduleCache.get(url);
  }

  static has(url: string): boolean {
    return moduleCache.has(url);
  }
}
