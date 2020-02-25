// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ASSETS, MediaType, SourceFile } from "./compiler_sourcefile.ts";
import { OUT_DIR, WriteFileCallback, getAsset } from "./compiler_util.ts";
import { cwd } from "./dir.ts";
import { assert, notImplemented } from "./util.ts";
import * as util from "./util.ts";

/** Specifies the target that the host should use to inform the TypeScript
 * compiler of what types should be used to validate the program against. */
export enum CompilerHostTarget {
  /** The main isolate library, where the main program runs. */
  Main = "main",
  /** The runtime API library. */
  Runtime = "runtime",
  /** The worker isolate library, where worker programs run. */
  Worker = "worker"
}

export interface CompilerHostOptions {
  /** Flag determines if the host should assume a single bundle output. */
  bundle?: boolean;

  /** Determines what the default library that should be used when type checking
   * TS code. */
  target: CompilerHostTarget;

  /** A function to be used when the program emit occurs to write out files. */
  writeFile: WriteFileCallback;
}

export interface ConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
}

/** Options that need to be used when generating a bundle (either trusted or
 * runtime). */
export const defaultBundlerOptions: ts.CompilerOptions = {
  inlineSourceMap: false,
  module: ts.ModuleKind.System,
  outDir: undefined,
  outFile: `${OUT_DIR}/bundle.js`,
  // disabled until we have effective way to modify source maps
  sourceMap: false
};

/** Default options used by the compiler Host when compiling. */
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
  target: ts.ScriptTarget.ESNext
};

/** Options that need to be used when doing a runtime (non bundled) compilation */
export const defaultRuntimeCompileOptions: ts.CompilerOptions = {
  outDir: undefined
};

/** Default options used when doing a transpile only. */
export const defaultTranspileOptions: ts.CompilerOptions = {
  esModuleInterop: true,
  module: ts.ModuleKind.ESNext,
  sourceMap: true,
  scriptComments: true,
  target: ts.ScriptTarget.ESNext
};

/** Options that either do nothing in Deno, or would cause undesired behavior
 * if modified. */
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
  "watch"
];

export class Host implements ts.CompilerHost {
  private readonly _options = defaultCompileOptions;

  private _target: CompilerHostTarget;

  private _writeFile: WriteFileCallback;

  private _getAsset(filename: string): SourceFile {
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
      sourceCode
    });
  }

  /* Deno specific APIs */

  /** Provides the `ts.HostCompiler` interface for Deno. */
  constructor(options: CompilerHostOptions) {
    const { bundle = false, target, writeFile } = options;
    this._target = target;
    this._writeFile = writeFile;
    if (bundle) {
      // options we need to change when we are generating a bundle
      Object.assign(this._options, defaultBundlerOptions);
    }
  }

  /** Take a configuration string, parse it, and use it to merge with the
   * compiler's configuration options.  The method returns an array of compiler
   * options which were ignored, or `undefined`. */
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
        (!(key in this._options) || options[key] !== this._options[key])
      ) {
        ignoredOptions.push(key);
        delete options[key];
      }
    }
    Object.assign(this._options, options);
    return {
      ignoredOptions: ignoredOptions.length ? ignoredOptions : undefined,
      diagnostics: errors.length ? errors : undefined
    };
  }

  /** Merge options into the host's current set of compiler options and return
   * the merged set. */
  mergeOptions(...options: ts.CompilerOptions[]): ts.CompilerOptions {
    Object.assign(this._options, ...options);
    return Object.assign({}, this._options);
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
    return this._options;
  }

  getCurrentDirectory(): string {
    return "";
  }

  getDefaultLibFileName(_options: ts.CompilerOptions): string {
    util.log("compiler::host.getDefaultLibFileName()");
    switch (this._target) {
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
        ? this._getAsset(fileName)
        : SourceFile.get(fileName);
      assert(sourceFile != null);
      if (!sourceFile.tsSourceFile) {
        assert(sourceFile.sourceCode != null);
        sourceFile.tsSourceFile = ts.createSourceFile(
          fileName.startsWith(ASSETS) ? sourceFile.filename : fileName,
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
      containingFile
    });
    return moduleNames.map(specifier => {
      const url = SourceFile.getUrl(specifier, containingFile);
      const sourceFile = specifier.startsWith(ASSETS)
        ? this._getAsset(specifier)
        : url
        ? SourceFile.get(url)
        : undefined;
      if (!sourceFile) {
        return undefined;
      }
      return {
        resolvedFileName: sourceFile.url,
        isExternalLibraryImport: specifier.startsWith(ASSETS),
        extension: sourceFile.extension
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
    this._writeFile(fileName, data, sourceFiles);
  }
}
