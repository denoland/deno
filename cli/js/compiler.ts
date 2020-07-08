// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) Combine this implementation with //deno_typescript/compiler_main.js

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to compile TS/WASM to JS.
//
// It provides a single functions that should be called by Rust:
//  - `bootstrapTsCompilerRuntime`
// This functions must be called when creating isolate
// to properly setup runtime.

// NOTE: this import has side effects!
import "./ts_global.d.ts";

import { bold, cyan, yellow } from "./colors.ts";
import type { CompilerOptions } from "./compiler_options.ts";
import type { Diagnostic, DiagnosticItem } from "./diagnostics.ts";
import { fromTypeScriptDiagnostic } from "./diagnostics_util.ts";
import type { TranspileOnlyResult } from "./ops/runtime_compiler.ts";
import { bootstrapWorkerRuntime } from "./runtime_worker.ts";
import { assert, log, notImplemented } from "./util.ts";
import { core } from "./core.ts";

// We really don't want to depend on JSON dispatch during snapshotting, so
// this op exchanges strings with Rust as raw byte arrays.
function getAsset(name: string): string {
  const opId = core.ops()["op_fetch_asset"];
  const sourceCodeBytes = core.dispatch(opId, core.encode(name));
  return core.decode(sourceCodeBytes!);
}

// Constants used by `normalizeString` and `resolvePath`
const CHAR_DOT = 46; /* . */
const CHAR_FORWARD_SLASH = 47; /* / */
// Using incremental compile APIs requires that all
// paths must be either relative or absolute. Since
// analysis in Rust operates on fully resolved URLs,
// it makes sense to use the same scheme here.
const ASSETS = "asset://";
const OUT_DIR = "deno://";
// This constant is passed to compiler settings when
// doing incremental compiles. Contents of this
// file are passed back to Rust and saved to $DENO_DIR.
const TS_BUILD_INFO = "cache:///tsbuildinfo.json";

// TODO(Bartlomieju): this check should be done in Rust
const IGNORED_COMPILER_OPTIONS: readonly string[] = [
  "allowSyntheticDefaultImports",
  "allowUmdGlobalAccess",
  "assumeChangesOnlyAffectDirectDependencies",
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
  "generateCpuProfile",
  "help",
  "importHelpers",
  "incremental",
  "inlineSourceMap",
  "inlineSources",
  "init",
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

const DEFAULT_BUNDLER_OPTIONS: ts.CompilerOptions = {
  allowJs: true,
  inlineSourceMap: false,
  module: ts.ModuleKind.System,
  outDir: undefined,
  outFile: `${OUT_DIR}/bundle.js`,
  // disabled until we have effective way to modify source maps
  sourceMap: false,
};

const DEFAULT_INCREMENTAL_COMPILE_OPTIONS: ts.CompilerOptions = {
  allowJs: false,
  allowNonTsExtensions: true,
  checkJs: false,
  esModuleInterop: true,
  incremental: true,
  inlineSourceMap: true,
  jsx: ts.JsxEmit.React,
  module: ts.ModuleKind.ESNext,
  outDir: OUT_DIR,
  resolveJsonModule: true,
  sourceMap: false,
  strict: true,
  stripComments: true,
  target: ts.ScriptTarget.ESNext,
  tsBuildInfoFile: TS_BUILD_INFO,
};

const DEFAULT_COMPILE_OPTIONS: ts.CompilerOptions = {
  allowJs: false,
  allowNonTsExtensions: true,
  checkJs: false,
  esModuleInterop: true,
  jsx: ts.JsxEmit.React,
  module: ts.ModuleKind.ESNext,
  outDir: OUT_DIR,
  sourceMap: true,
  strict: true,
  removeComments: true,
  target: ts.ScriptTarget.ESNext,
};

const DEFAULT_TRANSPILE_OPTIONS: ts.CompilerOptions = {
  esModuleInterop: true,
  inlineSourceMap: true,
  jsx: ts.JsxEmit.React,
  module: ts.ModuleKind.ESNext,
  removeComments: true,
  target: ts.ScriptTarget.ESNext,
};

const DEFAULT_RUNTIME_COMPILE_OPTIONS: ts.CompilerOptions = {
  outDir: undefined,
};

const DEFAULT_RUNTIME_TRANSPILE_OPTIONS: ts.CompilerOptions = {
  esModuleInterop: true,
  module: ts.ModuleKind.ESNext,
  sourceMap: true,
  scriptComments: true,
  target: ts.ScriptTarget.ESNext,
};

enum CompilerHostTarget {
  Main = "main",
  Runtime = "runtime",
  Worker = "worker",
}

interface CompilerHostOptions {
  bundle?: boolean;
  target: CompilerHostTarget;
  unstable?: boolean;
  writeFile: WriteFileCallback;
  incremental?: boolean;
}

type IncrementalCompilerHostOptions = Omit<
  CompilerHostOptions,
  "incremental"
> & {
  rootNames?: string[];
  buildInfo?: string;
};

interface HostConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
}

interface ConfigureResponse extends HostConfigureResponse {
  options: ts.CompilerOptions;
}

// Warning! The values in this enum are duplicated in `cli/msg.rs`
// Update carefully!
enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  TSX = 3,
  Json = 4,
  Wasm = 5,
  Unknown = 6,
}

interface SourceFileJson {
  url: string;
  filename: string;
  mediaType: MediaType;
  sourceCode: string;
  versionHash: string;
}

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

/** A global cache of module source files that have been loaded.
 * This cache will be rewritten to be populated on compiler startup
 * with files provided from Rust in request message.
 */
const SOURCE_FILE_CACHE: Map<string, SourceFile> = new Map();
/** A map of maps which cache resolved specifier for each import in a file.
 * This cache is used so `resolveModuleNames` ops is called as few times
 * as possible.
 *
 * First map's key is "referrer" URL ("file://a/b/c/mod.ts")
 * Second map's key is "raw" import specifier ("./foo.ts")
 * Second map's value is resolved import URL ("file:///a/b/c/foo.ts")
 */
const RESOLVED_SPECIFIER_CACHE: Map<string, Map<string, string>> = new Map();

function configure(
  defaultOptions: ts.CompilerOptions,
  source: string,
  path: string,
  cwd: string
): ConfigureResponse {
  const { config, error } = ts.parseConfigFileTextToJson(path, source);
  if (error) {
    return { diagnostics: [error], options: defaultOptions };
  }
  const { options, errors } = ts.convertCompilerOptionsFromJson(
    config.compilerOptions,
    cwd
  );
  const ignoredOptions: string[] = [];
  for (const key of Object.keys(options)) {
    if (
      IGNORED_COMPILER_OPTIONS.includes(key) &&
      (!(key in defaultOptions) || options[key] !== defaultOptions[key])
    ) {
      ignoredOptions.push(key);
      delete options[key];
    }
  }
  return {
    options: Object.assign({}, defaultOptions, options),
    ignoredOptions: ignoredOptions.length ? ignoredOptions : undefined,
    diagnostics: errors.length ? errors : undefined,
  };
}

class SourceFile {
  extension!: ts.Extension;
  filename!: string;

  mediaType!: MediaType;
  processed = false;
  sourceCode?: string;
  tsSourceFile?: ts.SourceFile;
  versionHash!: string;
  url!: string;

  constructor(json: SourceFileJson) {
    Object.assign(this, json);
    this.extension = getExtension(this.url, this.mediaType);
  }

  static addToCache(json: SourceFileJson): SourceFile {
    if (SOURCE_FILE_CACHE.has(json.url)) {
      throw new TypeError("SourceFile already exists");
    }
    const sf = new SourceFile(json);
    SOURCE_FILE_CACHE.set(sf.url, sf);
    return sf;
  }

  static getCached(url: string): SourceFile | undefined {
    return SOURCE_FILE_CACHE.get(url);
  }

  static cacheResolvedUrl(
    resolvedUrl: string,
    rawModuleSpecifier: string,
    containingFile?: string
  ): void {
    containingFile = containingFile || "";
    let innerCache = RESOLVED_SPECIFIER_CACHE.get(containingFile);
    if (!innerCache) {
      innerCache = new Map();
      RESOLVED_SPECIFIER_CACHE.set(containingFile, innerCache);
    }
    innerCache.set(rawModuleSpecifier, resolvedUrl);
  }

  static getResolvedUrl(
    moduleSpecifier: string,
    containingFile: string
  ): string | undefined {
    const containingCache = RESOLVED_SPECIFIER_CACHE.get(containingFile);
    if (containingCache) {
      return containingCache.get(moduleSpecifier);
    }
    return undefined;
  }
}

function getAssetInternal(filename: string): SourceFile {
  const lastSegment = filename.split("/").pop()!;
  const url = ts.libMap.has(lastSegment)
    ? ts.libMap.get(lastSegment)!
    : lastSegment;
  const sourceFile = SourceFile.getCached(url);
  if (sourceFile) {
    return sourceFile;
  }
  const name = url.includes(".") ? url : `${url}.d.ts`;
  const sourceCode = getAsset(name);
  return SourceFile.addToCache({
    url,
    filename: `${ASSETS}/${name}`,
    mediaType: MediaType.TypeScript,
    versionHash: "1",
    sourceCode,
  });
}

class Host implements ts.CompilerHost {
  #options = DEFAULT_COMPILE_OPTIONS;
  readonly #target: CompilerHostTarget;
  readonly #writeFile: WriteFileCallback;
  /* Deno specific APIs */

  constructor({
    bundle = false,
    incremental = false,
    target,
    unstable,
    writeFile,
  }: CompilerHostOptions) {
    this.#target = target;
    this.#writeFile = writeFile;
    if (bundle) {
      // options we need to change when we are generating a bundle
      Object.assign(this.#options, DEFAULT_BUNDLER_OPTIONS);
    } else if (incremental) {
      Object.assign(this.#options, DEFAULT_INCREMENTAL_COMPILE_OPTIONS);
    }
    if (unstable) {
      this.#options.lib = [
        target === CompilerHostTarget.Worker
          ? "lib.deno.worker.d.ts"
          : "lib.deno.window.d.ts",
        "lib.deno.unstable.d.ts",
      ];
    }
  }

  get options(): ts.CompilerOptions {
    return this.#options;
  }

  configure(
    cwd: string,
    path: string,
    configurationText: string
  ): HostConfigureResponse {
    log("compiler::host.configure", path);
    const { options, ...result } = configure(
      this.#options,
      configurationText,
      path,
      cwd
    );
    this.#options = options;
    return result;
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
    log("compiler::host.getCompilationSettings()");
    return this.#options;
  }

  getCurrentDirectory(): string {
    return "";
  }

  getDefaultLibFileName(_options: ts.CompilerOptions): string {
    log("compiler::host.getDefaultLibFileName()");
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
    log("compiler::host.getSourceFile", fileName);
    try {
      assert(!shouldCreateNewSourceFile);
      const sourceFile = fileName.startsWith(ASSETS)
        ? getAssetInternal(fileName)
        : SourceFile.getCached(fileName);
      assert(sourceFile != null);
      if (!sourceFile.tsSourceFile) {
        assert(sourceFile.sourceCode != null);
        const tsSourceFileName = fileName.startsWith(ASSETS)
          ? sourceFile.filename
          : fileName;

        sourceFile.tsSourceFile = ts.createSourceFile(
          tsSourceFileName,
          sourceFile.sourceCode,
          languageVersion
        );
        sourceFile.tsSourceFile.version = sourceFile.versionHash;
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
    log("compiler::host.resolveModuleNames", {
      moduleNames,
      containingFile,
    });
    const resolved = moduleNames.map((specifier) => {
      const maybeUrl = SourceFile.getResolvedUrl(specifier, containingFile);

      log("compiler::host.resolveModuleNames maybeUrl", {
        specifier,
        maybeUrl,
      });

      let sourceFile: SourceFile | undefined = undefined;

      if (specifier.startsWith(ASSETS)) {
        sourceFile = getAssetInternal(specifier);
      } else if (typeof maybeUrl !== "undefined") {
        sourceFile = SourceFile.getCached(maybeUrl);
      }

      if (!sourceFile) {
        return undefined;
      }

      return {
        resolvedFileName: sourceFile.url,
        isExternalLibraryImport: specifier.startsWith(ASSETS),
        extension: sourceFile.extension,
      };
    });
    log(resolved);
    return resolved;
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
    log("compiler::host.writeFile", fileName);
    this.#writeFile(fileName, data, sourceFiles);
  }
}

class IncrementalCompileHost extends Host {
  readonly #buildInfo?: string;

  constructor(options: IncrementalCompilerHostOptions) {
    super({ ...options, incremental: true });
    const { buildInfo } = options;
    if (buildInfo) {
      this.#buildInfo = buildInfo;
    }
  }

  readFile(fileName: string): string | undefined {
    if (fileName == TS_BUILD_INFO) {
      return this.#buildInfo;
    }
    throw new Error("unreachable");
  }
}

// NOTE: target doesn't really matter here,
// this is in fact a mock host created just to
// load all type definitions and snapshot them.
let SNAPSHOT_HOST: Host | undefined = new Host({
  target: CompilerHostTarget.Main,
  writeFile(): void {},
});
const SNAPSHOT_COMPILER_OPTIONS = SNAPSHOT_HOST.getCompilationSettings();

// This is a hacky way of adding our libs to the libs available in TypeScript()
// as these are internal APIs of TypeScript which maintain valid libs
ts.libs.push("deno.ns", "deno.window", "deno.worker", "deno.shared_globals");
ts.libMap.set("deno.ns", "lib.deno.ns.d.ts");
ts.libMap.set("deno.window", "lib.deno.window.d.ts");
ts.libMap.set("deno.worker", "lib.deno.worker.d.ts");
ts.libMap.set("deno.shared_globals", "lib.deno.shared_globals.d.ts");
ts.libMap.set("deno.unstable", "lib.deno.unstable.d.ts");

// this pre-populates the cache at snapshot time of our library files, so they
// are available in the future when needed.
SNAPSHOT_HOST.getSourceFile(
  `${ASSETS}/lib.deno.ns.d.ts`,
  ts.ScriptTarget.ESNext
);
SNAPSHOT_HOST.getSourceFile(
  `${ASSETS}/lib.deno.window.d.ts`,
  ts.ScriptTarget.ESNext
);
SNAPSHOT_HOST.getSourceFile(
  `${ASSETS}/lib.deno.worker.d.ts`,
  ts.ScriptTarget.ESNext
);
SNAPSHOT_HOST.getSourceFile(
  `${ASSETS}/lib.deno.shared_globals.d.ts`,
  ts.ScriptTarget.ESNext
);
SNAPSHOT_HOST.getSourceFile(
  `${ASSETS}/lib.deno.unstable.d.ts`,
  ts.ScriptTarget.ESNext
);

// We never use this program; it's only created
// during snapshotting to hydrate and populate
// source file cache with lib declaration files.
const _TS_SNAPSHOT_PROGRAM = ts.createProgram({
  rootNames: [`${ASSETS}/bootstrap.ts`],
  options: SNAPSHOT_COMPILER_OPTIONS,
  host: SNAPSHOT_HOST,
});

// Derference the snapshot host so it can be GCed
SNAPSHOT_HOST = undefined;

// This function is called only during snapshotting process
const SYSTEM_LOADER = getAsset("system_loader.js");
const SYSTEM_LOADER_ES5 = getAsset("system_loader_es5.js");

function buildLocalSourceFileCache(
  sourceFileMap: Record<string, SourceFileMapEntry>
): void {
  for (const entry of Object.values(sourceFileMap)) {
    assert(entry.sourceCode.length > 0);
    SourceFile.addToCache({
      url: entry.url,
      filename: entry.url,
      mediaType: entry.mediaType,
      sourceCode: entry.sourceCode,
      versionHash: entry.versionHash,
    });

    for (const importDesc of entry.imports) {
      let mappedUrl = importDesc.resolvedSpecifier;
      const importedFile = sourceFileMap[importDesc.resolvedSpecifier];
      assert(importedFile);
      const isJsOrJsx =
        importedFile.mediaType === MediaType.JavaScript ||
        importedFile.mediaType === MediaType.JSX;
      // If JS or JSX perform substitution for types if available
      if (isJsOrJsx) {
        if (importedFile.typeHeaders.length > 0) {
          const typeHeaders = importedFile.typeHeaders[0];
          mappedUrl = typeHeaders.resolvedSpecifier;
        } else if (importDesc.resolvedTypeDirective) {
          mappedUrl = importDesc.resolvedTypeDirective;
        } else if (importedFile.typesDirectives.length > 0) {
          const typeDirective = importedFile.typesDirectives[0];
          mappedUrl = typeDirective.resolvedSpecifier;
        }
      }

      mappedUrl = mappedUrl.replace("memory://", "");
      SourceFile.cacheResolvedUrl(mappedUrl, importDesc.specifier, entry.url);
    }
    for (const fileRef of entry.referencedFiles) {
      SourceFile.cacheResolvedUrl(
        fileRef.resolvedSpecifier.replace("memory://", ""),
        fileRef.specifier,
        entry.url
      );
    }
    for (const fileRef of entry.libDirectives) {
      SourceFile.cacheResolvedUrl(
        fileRef.resolvedSpecifier.replace("memory://", ""),
        fileRef.specifier,
        entry.url
      );
    }
  }
}

function buildSourceFileCache(
  sourceFileMap: Record<string, SourceFileMapEntry>
): void {
  for (const entry of Object.values(sourceFileMap)) {
    SourceFile.addToCache({
      url: entry.url,
      filename: entry.url,
      mediaType: entry.mediaType,
      sourceCode: entry.sourceCode,
      versionHash: entry.versionHash,
    });

    for (const importDesc of entry.imports) {
      let mappedUrl = importDesc.resolvedSpecifier;
      const importedFile = sourceFileMap[importDesc.resolvedSpecifier];
      // IMPORTANT: due to HTTP redirects we might end up in situation
      // where URL points to a file with completely different URL.
      // In that case we take value of `redirect` field and cache
      // resolved specifier pointing to the value of the redirect.
      // It's not very elegant solution and should be rethinked.
      assert(importedFile);
      if (importedFile.redirect) {
        mappedUrl = importedFile.redirect;
      }
      const isJsOrJsx =
        importedFile.mediaType === MediaType.JavaScript ||
        importedFile.mediaType === MediaType.JSX;
      // If JS or JSX perform substitution for types if available
      if (isJsOrJsx) {
        if (importedFile.typeHeaders.length > 0) {
          const typeHeaders = importedFile.typeHeaders[0];
          mappedUrl = typeHeaders.resolvedSpecifier;
        } else if (importDesc.resolvedTypeDirective) {
          mappedUrl = importDesc.resolvedTypeDirective;
        } else if (importedFile.typesDirectives.length > 0) {
          const typeDirective = importedFile.typesDirectives[0];
          mappedUrl = typeDirective.resolvedSpecifier;
        }
      }

      SourceFile.cacheResolvedUrl(mappedUrl, importDesc.specifier, entry.url);
    }
    for (const fileRef of entry.referencedFiles) {
      SourceFile.cacheResolvedUrl(
        fileRef.resolvedSpecifier,
        fileRef.specifier,
        entry.url
      );
    }
    for (const fileRef of entry.libDirectives) {
      SourceFile.cacheResolvedUrl(
        fileRef.resolvedSpecifier,
        fileRef.specifier,
        entry.url
      );
    }
  }
}

interface EmittedSource {
  // original filename
  filename: string;
  // compiled contents
  contents: string;
}

type WriteFileCallback = (
  fileName: string,
  data: string,
  sourceFiles?: readonly ts.SourceFile[]
) => void;

interface CompileWriteFileState {
  rootNames: string[];
  emitMap: Record<string, EmittedSource>;
  buildInfo?: string;
}

interface BundleWriteFileState {
  host?: Host;
  bundleOutput: undefined | string;
  rootNames: string[];
}

// Warning! The values in this enum are duplicated in `cli/msg.rs`
// Update carefully!
enum CompilerRequestType {
  Compile = 0,
  Transpile = 1,
  Bundle = 2,
  RuntimeCompile = 3,
  RuntimeBundle = 4,
  RuntimeTranspile = 5,
}

function createBundleWriteFile(state: BundleWriteFileState): WriteFileCallback {
  return function writeFile(
    _fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    assert(sourceFiles != null);
    assert(state.host);
    // we only support single root names for bundles
    assert(state.rootNames.length === 1);
    state.bundleOutput = buildBundle(
      state.rootNames[0],
      data,
      sourceFiles,
      state.host.options.target ?? ts.ScriptTarget.ESNext
    );
  };
}

function createCompileWriteFile(
  state: CompileWriteFileState
): WriteFileCallback {
  return function writeFile(
    fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    const isBuildInfo = fileName === TS_BUILD_INFO;

    if (isBuildInfo) {
      assert(isBuildInfo);
      state.buildInfo = data;
      return;
    }

    assert(sourceFiles);
    assert(sourceFiles.length === 1);
    state.emitMap[fileName] = {
      filename: sourceFiles[0].fileName,
      contents: data,
    };
  };
}

function createRuntimeCompileWriteFile(
  state: CompileWriteFileState
): WriteFileCallback {
  return function writeFile(
    fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    assert(sourceFiles);
    assert(sourceFiles.length === 1);
    state.emitMap[fileName] = {
      filename: sourceFiles[0].fileName,
      contents: data,
    };
  };
}
interface ConvertCompilerOptionsResult {
  files?: string[];
  options: ts.CompilerOptions;
}

function convertCompilerOptions(str: string): ConvertCompilerOptionsResult {
  const options: CompilerOptions = JSON.parse(str);
  const out: Record<string, unknown> = {};
  const keys = Object.keys(options) as Array<keyof CompilerOptions>;
  const files: string[] = [];
  for (const key of keys) {
    switch (key) {
      case "jsx":
        const value = options[key];
        if (value === "preserve") {
          out[key] = ts.JsxEmit.Preserve;
        } else if (value === "react") {
          out[key] = ts.JsxEmit.React;
        } else {
          out[key] = ts.JsxEmit.ReactNative;
        }
        break;
      case "module":
        switch (options[key]) {
          case "amd":
            out[key] = ts.ModuleKind.AMD;
            break;
          case "commonjs":
            out[key] = ts.ModuleKind.CommonJS;
            break;
          case "es2015":
          case "es6":
            out[key] = ts.ModuleKind.ES2015;
            break;
          case "esnext":
            out[key] = ts.ModuleKind.ESNext;
            break;
          case "none":
            out[key] = ts.ModuleKind.None;
            break;
          case "system":
            out[key] = ts.ModuleKind.System;
            break;
          case "umd":
            out[key] = ts.ModuleKind.UMD;
            break;
          default:
            throw new TypeError("Unexpected module type");
        }
        break;
      case "target":
        switch (options[key]) {
          case "es3":
            out[key] = ts.ScriptTarget.ES3;
            break;
          case "es5":
            out[key] = ts.ScriptTarget.ES5;
            break;
          case "es6":
          case "es2015":
            out[key] = ts.ScriptTarget.ES2015;
            break;
          case "es2016":
            out[key] = ts.ScriptTarget.ES2016;
            break;
          case "es2017":
            out[key] = ts.ScriptTarget.ES2017;
            break;
          case "es2018":
            out[key] = ts.ScriptTarget.ES2018;
            break;
          case "es2019":
            out[key] = ts.ScriptTarget.ES2019;
            break;
          case "es2020":
            out[key] = ts.ScriptTarget.ES2020;
            break;
          case "esnext":
            out[key] = ts.ScriptTarget.ESNext;
            break;
          default:
            throw new TypeError("Unexpected emit target.");
        }
        break;
      case "types":
        const types = options[key];
        assert(types);
        files.push(...types);
        break;
      default:
        out[key] = options[key];
    }
  }
  return {
    options: out as ts.CompilerOptions,
    files: files.length ? files : undefined,
  };
}

const ignoredDiagnostics = [
  // TS2306: File 'file:///Users/rld/src/deno/cli/tests/subdir/amd_like.js' is
  // not a module.
  2306,
  // TS1375: 'await' expressions are only allowed at the top level of a file
  // when that file is a module, but this file has no imports or exports.
  // Consider adding an empty 'export {}' to make this file a module.
  1375,
  // TS1103: 'for-await-of' statement is only allowed within an async function
  // or async generator.
  1103,
  // TS2691: An import path cannot end with a '.ts' extension. Consider
  // importing 'bad-module' instead.
  2691,
  // TS5009: Cannot find the common subdirectory path for the input files.
  5009,
  // TS5055: Cannot write file
  // 'http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js'
  // because it would overwrite input file.
  5055,
  // TypeScript is overly opinionated that only CommonJS modules kinds can
  // support JSON imports.  Allegedly this was fixed in
  // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
  // so we will ignore complaints about this compiler setting.
  5070,
  // TS7016: Could not find a declaration file for module '...'. '...'
  // implicitly has an 'any' type.  This is due to `allowJs` being off by
  // default but importing of a JavaScript module.
  7016,
];

type Stats = Array<{ key: string; value: number }>;

const stats: Stats = [];
let statsStart = 0;

function performanceStart(): void {
  stats.length = 0;
  // TODO(kitsonk) replace with performance.mark() when landed
  statsStart = performance.now();
  ts.performance.enable();
}

function performanceProgram({
  program,
  fileCount,
}: {
  program?: ts.Program | ts.BuilderProgram;
  fileCount?: number;
}): void {
  if (program) {
    if ("getProgram" in program) {
      program = program.getProgram();
    }
    stats.push({ key: "Files", value: program.getSourceFiles().length });
    stats.push({ key: "Nodes", value: program.getNodeCount() });
    stats.push({ key: "Identifiers", value: program.getIdentifierCount() });
    stats.push({ key: "Symbols", value: program.getSymbolCount() });
    stats.push({ key: "Types", value: program.getTypeCount() });
    stats.push({
      key: "Instantiations",
      value: program.getInstantiationCount(),
    });
  } else if (fileCount != null) {
    stats.push({ key: "Files", value: fileCount });
  }
  const programTime = ts.performance.getDuration("Program");
  const bindTime = ts.performance.getDuration("Bind");
  const checkTime = ts.performance.getDuration("Check");
  const emitTime = ts.performance.getDuration("Emit");
  stats.push({ key: "Parse time", value: programTime });
  stats.push({ key: "Bind time", value: bindTime });
  stats.push({ key: "Check time", value: checkTime });
  stats.push({ key: "Emit time", value: emitTime });
  stats.push({
    key: "Total TS time",
    value: programTime + bindTime + checkTime + emitTime,
  });
}

function performanceEnd(): Stats {
  // TODO(kitsonk) replace with performance.measure() when landed
  const duration = performance.now() - statsStart;
  stats.push({ key: "Compile time", value: duration });
  return stats;
}

// TODO(Bartlomieju): this check should be done in Rust; there should be no
function processConfigureResponse(
  configResult: HostConfigureResponse,
  configPath: string
): ts.Diagnostic[] | undefined {
  const { ignoredOptions, diagnostics } = configResult;
  if (ignoredOptions) {
    console.warn(
      yellow(`Unsupported compiler options in "${configPath}"\n`) +
        cyan(`  The following options were ignored:\n`) +
        `    ${ignoredOptions.map((value): string => bold(value)).join(", ")}`
    );
  }
  return diagnostics;
}

function normalizeString(path: string): string {
  let res = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let code: number;
  for (let i = 0, len = path.length; i <= len; ++i) {
    if (i < len) code = path.charCodeAt(i);
    else if (code! === CHAR_FORWARD_SLASH) break;
    else code = CHAR_FORWARD_SLASH;

    if (code === CHAR_FORWARD_SLASH) {
      if (lastSlash === i - 1 || dots === 1) {
        // NOOP
      } else if (lastSlash !== i - 1 && dots === 2) {
        if (
          res.length < 2 ||
          lastSegmentLength !== 2 ||
          res.charCodeAt(res.length - 1) !== CHAR_DOT ||
          res.charCodeAt(res.length - 2) !== CHAR_DOT
        ) {
          if (res.length > 2) {
            const lastSlashIndex = res.lastIndexOf("/");
            if (lastSlashIndex === -1) {
              res = "";
              lastSegmentLength = 0;
            } else {
              res = res.slice(0, lastSlashIndex);
              lastSegmentLength = res.length - 1 - res.lastIndexOf("/");
            }
            lastSlash = i;
            dots = 0;
            continue;
          } else if (res.length === 2 || res.length === 1) {
            res = "";
            lastSegmentLength = 0;
            lastSlash = i;
            dots = 0;
            continue;
          }
        }
      } else {
        if (res.length > 0) res += "/" + path.slice(lastSlash + 1, i);
        else res = path.slice(lastSlash + 1, i);
        lastSegmentLength = i - lastSlash - 1;
      }
      lastSlash = i;
      dots = 0;
    } else if (code === CHAR_DOT && dots !== -1) {
      ++dots;
    } else {
      dots = -1;
    }
  }
  return res;
}

function commonPath(paths: string[], sep = "/"): string {
  const [first = "", ...remaining] = paths;
  if (first === "" || remaining.length === 0) {
    return first.substring(0, first.lastIndexOf(sep) + 1);
  }
  const parts = first.split(sep);

  let endOfPrefix = parts.length;
  for (const path of remaining) {
    const compare = path.split(sep);
    for (let i = 0; i < endOfPrefix; i++) {
      if (compare[i] !== parts[i]) {
        endOfPrefix = i;
      }
    }

    if (endOfPrefix === 0) {
      return "";
    }
  }
  const prefix = parts.slice(0, endOfPrefix).join(sep);
  return prefix.endsWith(sep) ? prefix : `${prefix}${sep}`;
}

let rootExports: string[] | undefined;

function normalizeUrl(rootName: string): string {
  const match = /^(\S+:\/{2,3})(.+)$/.exec(rootName);
  if (match) {
    const [, protocol, path] = match;
    return `${protocol}${normalizeString(path)}`;
  } else {
    return rootName;
  }
}

function buildBundle(
  rootName: string,
  data: string,
  sourceFiles: readonly ts.SourceFile[],
  target: ts.ScriptTarget
): string {
  // when outputting to AMD and a single outfile, TypeScript makes up the module
  // specifiers which are used to define the modules, and doesn't expose them
  // publicly, so we have to try to replicate
  const sources = sourceFiles.map((sf) => sf.fileName);
  const sharedPath = commonPath(sources);
  rootName = normalizeUrl(rootName)
    .replace(sharedPath, "")
    .replace(/\.\w+$/i, "");
  // If one of the modules requires support for top-level-await, TypeScript will
  // emit the execute function as an async function.  When this is the case we
  // need to bubble up the TLA to the instantiation, otherwise we instantiate
  // synchronously.
  const hasTla = data.match(/execute:\sasync\sfunction\s/);
  let instantiate: string;
  if (rootExports && rootExports.length) {
    instantiate = hasTla
      ? `const __exp = await __instantiate("${rootName}", true);\n`
      : `const __exp = __instantiate("${rootName}", false);\n`;
    for (const rootExport of rootExports) {
      if (rootExport === "default") {
        instantiate += `export default __exp["${rootExport}"];\n`;
      } else {
        instantiate += `export const ${rootExport} = __exp["${rootExport}"];\n`;
      }
    }
  } else {
    instantiate = hasTla
      ? `await __instantiate("${rootName}", true);\n`
      : `__instantiate("${rootName}", false);\n`;
  }
  const es5Bundle =
    target === ts.ScriptTarget.ES3 ||
    target === ts.ScriptTarget.ES5 ||
    target === ts.ScriptTarget.ES2015 ||
    target === ts.ScriptTarget.ES2016;
  return `${
    es5Bundle ? SYSTEM_LOADER_ES5 : SYSTEM_LOADER
  }\n${data}\n${instantiate}`;
}

function setRootExports(program: ts.Program, rootModule: string): void {
  // get a reference to the type checker, this will let us find symbols from
  // the AST.
  const checker = program.getTypeChecker();
  // get a reference to the main source file for the bundle
  const mainSourceFile = program.getSourceFile(rootModule);
  assert(mainSourceFile);
  // retrieve the internal TypeScript symbol for this AST node
  const mainSymbol = checker.getSymbolAtLocation(mainSourceFile);
  if (!mainSymbol) {
    return;
  }
  rootExports = checker
    .getExportsOfModule(mainSymbol)
    // .getExportsOfModule includes type only symbols which are exported from
    // the module, so we need to try to filter those out.  While not critical
    // someone looking at the bundle would think there is runtime code behind
    // that when there isn't.  There appears to be no clean way of figuring that
    // out, so inspecting SymbolFlags that might be present that are type only
    .filter(
      (sym) =>
        sym.flags & ts.SymbolFlags.Class ||
        !(
          sym.flags & ts.SymbolFlags.Interface ||
          sym.flags & ts.SymbolFlags.TypeLiteral ||
          sym.flags & ts.SymbolFlags.Signature ||
          sym.flags & ts.SymbolFlags.TypeParameter ||
          sym.flags & ts.SymbolFlags.TypeAlias ||
          sym.flags & ts.SymbolFlags.Type ||
          sym.flags & ts.SymbolFlags.Namespace ||
          sym.flags & ts.SymbolFlags.InterfaceExcludes ||
          sym.flags & ts.SymbolFlags.TypeParameterExcludes ||
          sym.flags & ts.SymbolFlags.TypeAliasExcludes
        )
    )
    .map((sym) => sym.getName());
}

interface ImportDescriptor {
  specifier: string;
  resolvedSpecifier: string;
  typeDirective?: string;
  resolvedTypeDirective?: string;
}

interface ReferenceDescriptor {
  specifier: string;
  resolvedSpecifier: string;
}

interface SourceFileMapEntry {
  // fully resolved URL
  url: string;
  sourceCode: string;
  mediaType: MediaType;
  redirect?: string;
  imports: ImportDescriptor[];
  referencedFiles: ReferenceDescriptor[];
  libDirectives: ReferenceDescriptor[];
  typesDirectives: ReferenceDescriptor[];
  typeHeaders: ReferenceDescriptor[];
  versionHash: string;
}

/** Used when "deno run" is invoked */
interface CompileRequest {
  type: CompilerRequestType.Compile;
  allowJs: boolean;
  target: CompilerHostTarget;
  rootNames: string[];
  configPath?: string;
  config?: string;
  unstable: boolean;
  performance: boolean;
  cwd: string;
  // key value is fully resolved URL
  sourceFileMap: Record<string, SourceFileMapEntry>;
  buildInfo?: string;
}

interface TranspileRequest {
  type: CompilerRequestType.Transpile;
  config?: string;
  configPath?: string;
  cwd?: string;
  performance: boolean;
  sourceFiles: TranspileSourceFile[];
}

interface TranspileSourceFile {
  sourceCode: string;
  fileName: string;
}

/** Used when "deno bundle" is invoked */
interface BundleRequest {
  type: CompilerRequestType.Bundle;
  target: CompilerHostTarget;
  rootNames: string[];
  configPath?: string;
  config?: string;
  unstable: boolean;
  performance: boolean;
  cwd: string;
  // key value is fully resolved URL
  sourceFileMap: Record<string, SourceFileMapEntry>;
}

/** Used when "Deno.compile()" API is called */
interface RuntimeCompileRequest {
  type: CompilerRequestType.RuntimeCompile;
  target: CompilerHostTarget;
  rootNames: string[];
  sourceFileMap: Record<string, SourceFileMapEntry>;
  unstable?: boolean;
  options?: string;
}

/** Used when "Deno.bundle()" API is called */
interface RuntimeBundleRequest {
  type: CompilerRequestType.RuntimeBundle;
  target: CompilerHostTarget;
  rootNames: string[];
  sourceFileMap: Record<string, SourceFileMapEntry>;
  unstable?: boolean;
  options?: string;
}

/** Used when "Deno.transpileOnly()" API is called */
interface RuntimeTranspileRequest {
  type: CompilerRequestType.RuntimeTranspile;
  sources: Record<string, string>;
  options?: string;
}

type CompilerRequest =
  | CompileRequest
  | TranspileRequest
  | BundleRequest
  | RuntimeCompileRequest
  | RuntimeBundleRequest
  | RuntimeTranspileRequest;

interface CompileResponse {
  emitMap: Record<string, EmittedSource>;
  diagnostics: Diagnostic;
  buildInfo?: string;
  stats?: Stats;
}

interface TranspileResponse {
  emitMap: Record<string, EmittedSource>;
  diagnostics: Diagnostic;
  stats?: Stats;
}

interface BundleResponse {
  bundleOutput?: string;
  diagnostics: Diagnostic;
  stats?: Stats;
}

interface RuntimeCompileResponse {
  emitMap: Record<string, EmittedSource>;
  diagnostics: DiagnosticItem[];
}

interface RuntimeBundleResponse {
  output?: string;
  diagnostics: DiagnosticItem[];
}

function compile({
  allowJs,
  buildInfo,
  config,
  configPath,
  rootNames,
  target,
  unstable,
  cwd,
  sourceFileMap,
  type,
  performance,
}: CompileRequest): CompileResponse {
  if (performance) {
    performanceStart();
  }
  log(">>> compile start", { rootNames, type: CompilerRequestType[type] });

  // When a programme is emitted, TypeScript will call `writeFile` with
  // each file that needs to be emitted.  The Deno compiler host delegates
  // this, to make it easier to perform the right actions, which vary
  // based a lot on the request.
  const state: CompileWriteFileState = {
    rootNames,
    emitMap: {},
  };
  const host = new IncrementalCompileHost({
    bundle: false,
    target,
    unstable,
    writeFile: createCompileWriteFile(state),
    rootNames,
    buildInfo,
  });
  let diagnostics: readonly ts.Diagnostic[] = [];

  host.mergeOptions({ allowJs });

  // if there is a configuration supplied, we need to parse that
  if (config && config.length && configPath) {
    const configResult = host.configure(cwd, configPath, config);
    diagnostics = processConfigureResponse(configResult, configPath) || [];
  }

  buildSourceFileCache(sourceFileMap);
  // if there was a configuration and no diagnostics with it, we will continue
  // to generate the program and possibly emit it.
  if (diagnostics.length === 0) {
    const options = host.getCompilationSettings();
    const program = ts.createIncrementalProgram({
      rootNames,
      options,
      host,
    });

    // TODO(bartlomieju): check if this is ok
    diagnostics = [
      ...program.getConfigFileParsingDiagnostics(),
      ...program.getSyntacticDiagnostics(),
      ...program.getOptionsDiagnostics(),
      ...program.getGlobalDiagnostics(),
      ...program.getSemanticDiagnostics(),
    ];
    diagnostics = diagnostics.filter(
      ({ code }) => !ignoredDiagnostics.includes(code)
    );

    // We will only proceed with the emit if there are no diagnostics.
    if (diagnostics.length === 0) {
      const emitResult = program.emit();
      // If `checkJs` is off we still might be compiling entry point JavaScript file
      // (if it has `.ts` imports), but it won't be emitted. In that case we skip
      // assertion.
      if (options.checkJs) {
        assert(
          emitResult.emitSkipped === false,
          "Unexpected skip of the emit."
        );
      }
      // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
      // without casting.
      diagnostics = emitResult.diagnostics;
    }
    performanceProgram({ program });
  }

  log("<<< compile end", { rootNames, type: CompilerRequestType[type] });
  const stats = performance ? performanceEnd() : undefined;

  return {
    emitMap: state.emitMap,
    buildInfo: state.buildInfo,
    diagnostics: fromTypeScriptDiagnostic(diagnostics),
    stats,
  };
}

function transpile({
  config: configText,
  configPath,
  cwd,
  performance,
  sourceFiles,
}: TranspileRequest): TranspileResponse {
  if (performance) {
    performanceStart();
  }
  log(">>> transpile start");
  let compilerOptions: ts.CompilerOptions;
  if (configText && configPath && cwd) {
    const { options, ...response } = configure(
      DEFAULT_TRANSPILE_OPTIONS,
      configText,
      configPath,
      cwd
    );
    const diagnostics = processConfigureResponse(response, configPath);
    if (diagnostics && diagnostics.length) {
      return {
        diagnostics: fromTypeScriptDiagnostic(diagnostics),
        emitMap: {},
      };
    }
    compilerOptions = options;
  } else {
    compilerOptions = Object.assign({}, DEFAULT_TRANSPILE_OPTIONS);
  }
  const emitMap: Record<string, EmittedSource> = {};
  let diagnostics: ts.Diagnostic[] = [];
  for (const { sourceCode, fileName } of sourceFiles) {
    const {
      outputText,
      sourceMapText,
      diagnostics: diags,
    } = ts.transpileModule(sourceCode, {
      fileName,
      compilerOptions,
      reportDiagnostics: true,
    });
    if (diags) {
      diagnostics = diagnostics.concat(...diags);
    }
    emitMap[`${fileName}.js`] = { filename: fileName, contents: outputText };
    // currently we inline source maps, but this is good logic to have if this
    // ever changes
    if (sourceMapText) {
      emitMap[`${fileName}.map`] = {
        filename: fileName,
        contents: sourceMapText,
      };
    }
  }
  performanceProgram({ fileCount: sourceFiles.length });
  const stats = performance ? performanceEnd() : undefined;
  log("<<< transpile end");
  return { diagnostics: fromTypeScriptDiagnostic(diagnostics), emitMap, stats };
}

function bundle({
  config,
  configPath,
  rootNames,
  target,
  unstable,
  cwd,
  sourceFileMap,
  type,
}: BundleRequest): BundleResponse {
  if (performance) {
    performanceStart();
  }
  log(">>> bundle start", {
    rootNames,
    type: CompilerRequestType[type],
  });

  // When a programme is emitted, TypeScript will call `writeFile` with
  // each file that needs to be emitted.  The Deno compiler host delegates
  // this, to make it easier to perform the right actions, which vary
  // based a lot on the request.
  const state: BundleWriteFileState = {
    rootNames,
    bundleOutput: undefined,
  };
  const host = new Host({
    bundle: true,
    target,
    unstable,
    writeFile: createBundleWriteFile(state),
  });
  state.host = host;
  let diagnostics: readonly ts.Diagnostic[] = [];

  // if there is a configuration supplied, we need to parse that
  if (config && config.length && configPath) {
    const configResult = host.configure(cwd, configPath, config);
    diagnostics = processConfigureResponse(configResult, configPath) || [];
  }

  buildSourceFileCache(sourceFileMap);
  // if there was a configuration and no diagnostics with it, we will continue
  // to generate the program and possibly emit it.
  if (diagnostics.length === 0) {
    const options = host.getCompilationSettings();
    const program = ts.createProgram({
      rootNames,
      options,
      host,
    });

    diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !ignoredDiagnostics.includes(code));

    // We will only proceed with the emit if there are no diagnostics.
    if (diagnostics.length === 0) {
      // we only support a single root module when bundling
      assert(rootNames.length === 1);
      setRootExports(program, rootNames[0]);
      const emitResult = program.emit();
      assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");
      // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
      // without casting.
      diagnostics = emitResult.diagnostics;
    }
    if (performance) {
      performanceProgram({ program });
    }
  }

  let bundleOutput;

  if (diagnostics.length === 0) {
    assert(state.bundleOutput);
    bundleOutput = state.bundleOutput;
  }

  const stats = performance ? performanceEnd() : undefined;

  const result: BundleResponse = {
    bundleOutput,
    diagnostics: fromTypeScriptDiagnostic(diagnostics),
    stats,
  };

  log("<<< bundle end", {
    rootNames,
    type: CompilerRequestType[type],
  });

  return result;
}

function runtimeCompile(
  request: RuntimeCompileRequest
): RuntimeCompileResponse {
  const { options, rootNames, target, unstable, sourceFileMap } = request;

  log(">>> runtime compile start", {
    rootNames,
  });

  // if there are options, convert them into TypeScript compiler options,
  // and resolve any external file references
  let convertedOptions: ts.CompilerOptions | undefined;
  if (options) {
    const result = convertCompilerOptions(options);
    convertedOptions = result.options;
  }

  buildLocalSourceFileCache(sourceFileMap);

  const state: CompileWriteFileState = {
    rootNames,
    emitMap: {},
  };
  const host = new Host({
    bundle: false,
    target,
    writeFile: createRuntimeCompileWriteFile(state),
  });
  const compilerOptions = [DEFAULT_RUNTIME_COMPILE_OPTIONS];
  if (convertedOptions) {
    compilerOptions.push(convertedOptions);
  }
  if (unstable) {
    compilerOptions.push({
      lib: [
        "deno.unstable",
        ...((convertedOptions && convertedOptions.lib) || ["deno.window"]),
      ],
    });
  }

  host.mergeOptions(...compilerOptions);

  const program = ts.createProgram({
    rootNames,
    options: host.getCompilationSettings(),
    host,
  });

  const diagnostics = ts
    .getPreEmitDiagnostics(program)
    .filter(({ code }) => !ignoredDiagnostics.includes(code));

  const emitResult = program.emit();

  assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

  log("<<< runtime compile finish", {
    rootNames,
    emitMap: Object.keys(state.emitMap),
  });

  const maybeDiagnostics = diagnostics.length
    ? fromTypeScriptDiagnostic(diagnostics).items
    : [];

  return {
    diagnostics: maybeDiagnostics,
    emitMap: state.emitMap,
  };
}

function runtimeBundle(request: RuntimeBundleRequest): RuntimeBundleResponse {
  const { options, rootNames, target, unstable, sourceFileMap } = request;

  log(">>> runtime bundle start", {
    rootNames,
  });

  // if there are options, convert them into TypeScript compiler options,
  // and resolve any external file references
  let convertedOptions: ts.CompilerOptions | undefined;
  if (options) {
    const result = convertCompilerOptions(options);
    convertedOptions = result.options;
  }

  buildLocalSourceFileCache(sourceFileMap);

  const state: BundleWriteFileState = {
    rootNames,
    bundleOutput: undefined,
  };
  const host = new Host({
    bundle: true,
    target,
    writeFile: createBundleWriteFile(state),
  });
  state.host = host;

  const compilerOptions = [DEFAULT_RUNTIME_COMPILE_OPTIONS];
  if (convertedOptions) {
    compilerOptions.push(convertedOptions);
  }
  if (unstable) {
    compilerOptions.push({
      lib: [
        "deno.unstable",
        ...((convertedOptions && convertedOptions.lib) || ["deno.window"]),
      ],
    });
  }
  compilerOptions.push(DEFAULT_BUNDLER_OPTIONS);
  host.mergeOptions(...compilerOptions);

  const program = ts.createProgram({
    rootNames,
    options: host.getCompilationSettings(),
    host,
  });

  setRootExports(program, rootNames[0]);
  const diagnostics = ts
    .getPreEmitDiagnostics(program)
    .filter(({ code }) => !ignoredDiagnostics.includes(code));

  const emitResult = program.emit();

  assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

  log("<<< runtime bundle finish", {
    rootNames,
  });

  const maybeDiagnostics = diagnostics.length
    ? fromTypeScriptDiagnostic(diagnostics).items
    : [];

  return {
    diagnostics: maybeDiagnostics,
    output: state.bundleOutput,
  };
}

function runtimeTranspile(
  request: RuntimeTranspileRequest
): Promise<Record<string, TranspileOnlyResult>> {
  const result: Record<string, TranspileOnlyResult> = {};
  const { sources, options } = request;
  const compilerOptions = options
    ? Object.assign(
        {},
        DEFAULT_RUNTIME_TRANSPILE_OPTIONS,
        convertCompilerOptions(options).options
      )
    : DEFAULT_RUNTIME_TRANSPILE_OPTIONS;

  for (const [fileName, inputText] of Object.entries(sources)) {
    const { outputText: source, sourceMapText: map } = ts.transpileModule(
      inputText,
      {
        fileName,
        compilerOptions,
      }
    );
    result[fileName] = { source, map };
  }
  return Promise.resolve(result);
}

async function tsCompilerOnMessage({
  data: request,
}: {
  data: CompilerRequest;
}): Promise<void> {
  switch (request.type) {
    case CompilerRequestType.Compile: {
      const result = compile(request);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.Transpile: {
      const result = transpile(request);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.Bundle: {
      const result = bundle(request);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeCompile: {
      const result = runtimeCompile(request);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeBundle: {
      const result = runtimeBundle(request);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeTranspile: {
      const result = await runtimeTranspile(request);
      globalThis.postMessage(result);
      break;
    }
    default:
      log(
        `!!! unhandled CompilerRequestType: ${
          (request as CompilerRequest).type
        } (${CompilerRequestType[(request as CompilerRequest).type]})`
      );
  }
  // Shutdown after single request
  globalThis.close();
}

function bootstrapTsCompilerRuntime(): void {
  bootstrapWorkerRuntime("TS", false);
  globalThis.onmessage = tsCompilerOnMessage;
}

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete (Object.prototype as any).__proto__;

Object.defineProperties(globalThis, {
  bootstrap: {
    value: {
      ...globalThis.bootstrap,
      tsCompilerRuntime: bootstrapTsCompilerRuntime,
    },
    configurable: true,
    writable: true,
  },
});
