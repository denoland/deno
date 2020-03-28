// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ASSETS, MediaType, SourceFile } from "./sourcefile.ts";
import { OUT_DIR, WriteFileCallback, getAsset } from "./util.ts";
import { cwd } from "../ops/fs/dir.ts";
import { assert, notImplemented } from "../util.ts";
import * as util from "../util.ts";

export enum CompilerHostTarget {
  Main = "main",
  Runtime = "runtime",
  Worker = "worker",
}

export interface CompilerHostOptions {
  bundle?: boolean;

  target: CompilerHostTarget;

  writeFile: WriteFileCallback;
}

export interface ConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
}

export const defaultBundlerOptions: ts.CompilerOptions = {
  allowJs: true,
  inlineSourceMap: false,
  module: ts.ModuleKind.System,
  outDir: undefined,
  outFile: `${OUT_DIR}/bundle.js`,
  // disabled until we have effective way to modify source maps
  sourceMap: false,
};

export const defaultCompileOptions: ts.CompilerOptions = {
  allowJs: false,
  allowNonTsExtensions: true,
  checkJs: false,
  esModuleInterop: true,
  jsx: ts.JsxEmit.React,
  module: ts.ModuleKind.ESNext,
  outDir: OUT_DIR,
  resolveJsonModule: true,
  sourceMap: true,
  strict: true,
  stripComments: true,
  target: ts.ScriptTarget.ESNext,
};

export const defaultRuntimeCompileOptions: ts.CompilerOptions = {
  outDir: undefined,
};

export const defaultTranspileOptions: ts.CompilerOptions = {
  esModuleInterop: true,
  module: ts.ModuleKind.ESNext,
  sourceMap: true,
  scriptComments: true,
  target: ts.ScriptTarget.ESNext,
};

const ignoredCompilerOptions: readonly string[] = [
  "allowSyntheticDefaultImports",
  "baseUrl",
  "build",
  "composite",
  "declaration",
  "declarationDir",
  "declarationMap",
  "diagnostics",
  "downlevelIteration",
  "emitBOM",
  "emitDeclarationOnly",
  "esModuleInterop",
  "extendedDiagnostics",
  "forceConsistentCasingInFileNames",
  "help",
  "importHelpers",
  "incremental",
  "inlineSourceMap",
  "inlineSources",
  "init",
  "isolatedModules",
  "listEmittedFiles",
  "listFiles",
  "mapRoot",
  "maxNodeModuleJsDepth",
  "module",
  "moduleResolution",
  "newLine",
  "noEmit",
  "noEmitHelpers",
  "noEmitOnError",
  "noLib",
  "noResolve",
  "out",
  "outDir",
  "outFile",
  "paths",
  "preserveSymlinks",
  "preserveWatchOutput",
  "pretty",
  "rootDir",
  "rootDirs",
  "showConfig",
  "skipDefaultLibCheck",
  "skipLibCheck",
  "sourceMap",
  "sourceRoot",
  "stripInternal",
  "target",
  "traceResolution",
  "tsBuildInfoFile",
  "types",
  "typeRoots",
  "version",
  "watch",
];

function getAssetInternal(filename: string): SourceFile {
  const lastSegment = filename.split("/").pop()!;
  const url = ts.libMap.has(lastSegment)
    ? ts.libMap.get(lastSegment)!
    : lastSegment;
  const sourceFile = SourceFile.get(url);
  if (sourceFile) {
    return sourceFile;
  }
  const name = url.includes(".") ? url : `${url}.d.ts`;
  const sourceCode = getAsset(name);
  return new SourceFile({
    url,
    filename: `${ASSETS}/${name}`,
    mediaType: MediaType.TypeScript,
    sourceCode,
  });
}

export class Host implements ts.CompilerHost {
  readonly #options = defaultCompileOptions;
  #target: CompilerHostTarget;
  #writeFile: WriteFileCallback;

  /* Deno specific APIs */

  constructor({ bundle = false, target, writeFile }: CompilerHostOptions) {
    this.#target = target;
    this.#writeFile = writeFile;
    if (bundle) {
      // options we need to change when we are generating a bundle
      Object.assign(this.#options, defaultBundlerOptions);
    }
  }

  configure(path: string, configurationText: string): ConfigureResponse {
    util.log("compiler::host.configure", path);
    assert(configurationText);
    const { config, error } = ts.parseConfigFileTextToJson(
      path,
      configurationText
    );
    if (error) {
      return { diagnostics: [error] };
    }
    const { options, errors } = ts.convertCompilerOptionsFromJson(
      config.compilerOptions,
      cwd()
    );
    const ignoredOptions: string[] = [];
    for (const key of Object.keys(options)) {
      if (
        ignoredCompilerOptions.includes(key) &&
        (!(key in this.#options) || options[key] !== this.#options[key])
      ) {
        ignoredOptions.push(key);
        delete options[key];
      }
    }
    Object.assign(this.#options, options);
    return {
      ignoredOptions: ignoredOptions.length ? ignoredOptions : undefined,
      diagnostics: errors.length ? errors : undefined,
    };
  }

  mergeOptions(...options: ts.CompilerOptions[]): ts.CompilerOptions {
    Object.assign(this.#options, ...options);
    return Object.assign({}, this.#options);
  }

  /* TypeScript CompilerHost APIs */

  fileExists(_fileName: string): boolean {
    return notImplemented();
  }

  getCanonicalFileName(fileName: string): string {
    return fileName;
  }

  getCompilationSettings(): ts.CompilerOptions {
    util.log("compiler::host.getCompilationSettings()");
    return this.#options;
  }

  getCurrentDirectory(): string {
    return "";
  }

  getDefaultLibFileName(_options: ts.CompilerOptions): string {
    util.log("compiler::host.getDefaultLibFileName()");
    switch (this.#target) {
      case CompilerHostTarget.Main:
      case CompilerHostTarget.Runtime:
        return `${ASSETS}/lib.deno.window.d.ts`;
      case CompilerHostTarget.Worker:
        return `${ASSETS}/lib.deno.worker.d.ts`;
    }
  }

  getNewLine(): string {
    return "\n";
  }

  getSourceFile(
    fileName: string,
    languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void,
    shouldCreateNewSourceFile?: boolean
  ): ts.SourceFile | undefined {
    util.log("compiler::host.getSourceFile", fileName);
    try {
      assert(!shouldCreateNewSourceFile);
      const sourceFile = fileName.startsWith(ASSETS)
        ? getAssetInternal(fileName)
        : SourceFile.get(fileName);
      assert(sourceFile != null);
      if (!sourceFile.tsSourceFile) {
        assert(sourceFile.sourceCode != null);
        // even though we assert the extension for JSON modules to the compiler
        // is TypeScript, TypeScript internally analyses the filename for its
        // extension and tries to parse it as JSON instead of TS.  We have to
        // change the filename to the TypeScript file.
        sourceFile.tsSourceFile = ts.createSourceFile(
          fileName.startsWith(ASSETS)
            ? sourceFile.filename
            : fileName.toLowerCase().endsWith(".json")
            ? `${fileName}.ts`
            : fileName,
          sourceFile.sourceCode,
          languageVersion
        );
        delete sourceFile.sourceCode;
      }
      return sourceFile.tsSourceFile;
    } catch (e) {
      if (onError) {
        onError(String(e));
      } else {
        throw e;
      }
      return undefined;
    }
  }

  readFile(_fileName: string): string | undefined {
    return notImplemented();
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string
  ): Array<ts.ResolvedModuleFull | undefined> {
    util.log("compiler::host.resolveModuleNames", {
      moduleNames,
      containingFile,
    });
    return moduleNames.map((specifier) => {
      const url = SourceFile.getUrl(specifier, containingFile);
      const sourceFile = specifier.startsWith(ASSETS)
        ? getAssetInternal(specifier)
        : url
        ? SourceFile.get(url)
        : undefined;
      if (!sourceFile) {
        return undefined;
      }
      return {
        resolvedFileName: sourceFile.url,
        isExternalLibraryImport: specifier.startsWith(ASSETS),
        extension: sourceFile.extension,
      };
    });
  }

  useCaseSensitiveFileNames(): boolean {
    return true;
  }

  writeFile(
    fileName: string,
    data: string,
    _writeByteOrderMark: boolean,
    _onError?: (message: string) => void,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    util.log("compiler::host.writeFile", fileName);
    this.#writeFile(fileName, data, sourceFiles);
  }
}
