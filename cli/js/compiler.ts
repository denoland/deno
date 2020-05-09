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
import { CompilerOptions } from "./compiler_options.ts";
import { Diagnostic, DiagnosticItem } from "./diagnostics.ts";
import { fromTypeScriptDiagnostic } from "./diagnostics_util.ts";
import { TranspileOnlyResult } from "./ops/runtime_compiler.ts";
import { sendAsync, sendSync } from "./ops/dispatch_json.ts";
import { bootstrapWorkerRuntime } from "./runtime_worker.ts";
import { assert, log } from "./util.ts";
import * as util from "./util.ts";
import { TextDecoder, TextEncoder } from "./web/text_encoding.ts";
import { core } from "./core.ts";

export function resolveModules(
  specifiers: string[],
  referrer?: string
): string[] {
  util.log("compiler::resolveModules", { specifiers, referrer });
  return sendSync("op_resolve_modules", { specifiers, referrer });
}

export function fetchSourceFiles(
  specifiers: string[],
  referrer?: string
): Promise<
  Array<{
    url: string;
    filename: string;
    mediaType: number;
    sourceCode: string;
  }>
> {
  util.log("compiler::fetchSourceFiles", { specifiers, referrer });
  return sendAsync("op_fetch_source_files", {
    specifiers,
    referrer,
  });
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function getAsset(name: string): string {
  const opId = core.ops()["op_fetch_asset"];
  // We really don't want to depend on JSON dispatch during snapshotting, so
  // this op exchanges strings with Rust as raw byte arrays.
  const sourceCodeBytes = core.dispatch(opId, encoder.encode(name));
  return decoder.decode(sourceCodeBytes!);
}

// Constants used by `normalizeString` and `resolvePath`
const CHAR_DOT = 46; /* . */
const CHAR_FORWARD_SLASH = 47; /* / */
const ASSETS = "$asset$";
const OUT_DIR = "$deno$";

// TODO(Bartlomieju): this check should be done in Rust
const IGNORED_COMPILER_OPTIONS: readonly string[] = [
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

const DEFAULT_BUNDLER_OPTIONS: ts.CompilerOptions = {
  allowJs: true,
  inlineSourceMap: false,
  module: ts.ModuleKind.System,
  outDir: undefined,
  outFile: `${OUT_DIR}/bundle.js`,
  // disabled until we have effective way to modify source maps
  sourceMap: false,
};

const DEFAULT_COMPILE_OPTIONS: ts.CompilerOptions = {
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
}

interface ConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
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

/** Because we support providing types for JS files as well as X-TypeScript-Types
 * header we might be feeding TS compiler with different files than import specifiers
 * suggest. To accomplish that we keep track of two different specifiers:
 *  - original - the one in import statement (import "./foo.js")
 *  - mapped - if there's no type directive it's the same as original, otherwise
 *             it's unresolved specifier for type directive (/// @deno-types="./foo.d.ts")
 */
interface SourceFileSpecifierMap {
  original: string;
  mapped: string;
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

class SourceFile {
  extension!: ts.Extension;
  filename!: string;

  mediaType!: MediaType;
  processed = false;
  sourceCode?: string;
  tsSourceFile?: ts.SourceFile;
  url!: string;

  constructor(json: SourceFileJson) {
    Object.assign(this, json);
    this.extension = getExtension(this.url, this.mediaType);
  }

  imports(processJsImports: boolean): SourceFileSpecifierMap[] {
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

    const readImportFiles = true;
    const isJsOrJsx =
      this.mediaType === MediaType.JavaScript ||
      this.mediaType === MediaType.JSX;
    const detectJsImports = isJsOrJsx;

    const preProcessedFileInfo = ts.preProcessFile(
      this.sourceCode,
      readImportFiles,
      detectJsImports
    );
    this.processed = true;
    const files: SourceFileSpecifierMap[] = [];

    function process(references: Array<{ fileName: string }>): void {
      for (const { fileName } of references) {
        files.push({ original: fileName, mapped: fileName });
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
        // If there's a type directive for current processed file; then we provide
        // different `mapped` specifier.
        const mappedModuleName = getMappedModuleName(
          importedFile,
          typeDirectives
        );
        files.push({
          original: importedFile.fileName,
          mapped: mappedModuleName ?? importedFile.fileName,
        });
      }
    } else if (processJsImports || !isJsOrJsx) {
      process(importedFiles);
    }
    process(referencedFiles);
    // built in libs comes across as `"dom"` for example, and should be filtered
    // out during pre-processing as they are either already cached or they will
    // be lazily fetched by the compiler host.  Ones that contain full files are
    // not filtered out and will be fetched as normal.
    const filteredLibs = libReferenceDirectives.filter(
      ({ fileName }) => !ts.libMap.has(fileName.toLowerCase())
    );
    process(filteredLibs);
    process(typeReferenceDirectives);
    return files;
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
      const resolvedUrl = containingCache.get(moduleSpecifier);
      return resolvedUrl;
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
    sourceCode,
  });
}

class Host implements ts.CompilerHost {
  readonly #options = DEFAULT_COMPILE_OPTIONS;
  #target: CompilerHostTarget;
  #writeFile: WriteFileCallback;

  /* Deno specific APIs */

  constructor({
    bundle = false,
    target,
    unstable,
    writeFile,
  }: CompilerHostOptions) {
    this.#target = target;
    this.#writeFile = writeFile;
    if (bundle) {
      // options we need to change when we are generating a bundle
      Object.assign(this.#options, DEFAULT_BUNDLER_OPTIONS);
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

  configure(
    cwd: string,
    path: string,
    configurationText: string
  ): ConfigureResponse {
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
      cwd
    );
    const ignoredOptions: string[] = [];
    for (const key of Object.keys(options)) {
      if (
        IGNORED_COMPILER_OPTIONS.includes(key) &&
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
    return util.notImplemented();
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
    return util.notImplemented();
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
      const maybeUrl = SourceFile.getResolvedUrl(specifier, containingFile);

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

// NOTE: target doesn't really matter here,
// this is in fact a mock host created just to
// load all type definitions and snapshot them.
const SNAPSHOT_HOST = new Host({
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

const TS_SNAPSHOT_PROGRAM = ts.createProgram({
  rootNames: [`${ASSETS}/bootstrap.ts`],
  options: SNAPSHOT_COMPILER_OPTIONS,
  host: SNAPSHOT_HOST,
});

// This function is called only during snapshotting process
const SYSTEM_LOADER = getAsset("system_loader.js");

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

function processLocalImports(
  sources: Record<string, string>,
  specifiers: SourceFileSpecifierMap[],
  referrer?: string,
  processJsImports = false
): string[] {
  if (!specifiers.length) {
    return [];
  }
  const moduleNames = specifiers.map((specifierMap) => {
    if (referrer) {
      return resolveSpecifier(specifierMap.mapped, referrer);
    } else {
      return specifierMap.mapped;
    }
  });

  for (let i = 0; i < moduleNames.length; i++) {
    const moduleName = moduleNames[i];
    const specifierMap = specifiers[i];
    assert(moduleName in sources, `Missing module in sources: "${moduleName}"`);
    let sourceFile = SourceFile.getCached(moduleName);
    if (typeof sourceFile === "undefined") {
      sourceFile = SourceFile.addToCache({
        url: moduleName,
        filename: moduleName,
        sourceCode: sources[moduleName],
        mediaType: getMediaType(moduleName),
      });
    }
    assert(sourceFile);
    SourceFile.cacheResolvedUrl(
      sourceFile.url,
      specifierMap.original,
      referrer
    );
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

async function processImports(
  specifiers: SourceFileSpecifierMap[],
  referrer?: string,
  processJsImports = false
): Promise<string[]> {
  if (!specifiers.length) {
    return [];
  }
  const sources = specifiers.map(({ mapped }) => mapped);
  const resolvedSources = resolveModules(sources, referrer);
  const sourceFiles = await fetchSourceFiles(resolvedSources, referrer);
  assert(sourceFiles.length === specifiers.length);
  for (let i = 0; i < sourceFiles.length; i++) {
    const specifierMap = specifiers[i];
    const sourceFileJson = sourceFiles[i];
    let sourceFile = SourceFile.getCached(sourceFileJson.url);
    if (typeof sourceFile === "undefined") {
      sourceFile = SourceFile.addToCache(sourceFileJson);
    }
    assert(sourceFile);
    SourceFile.cacheResolvedUrl(
      sourceFile.url,
      specifierMap.original,
      referrer
    );
    if (!sourceFile.processed) {
      const sourceFileImports = sourceFile.imports(processJsImports);
      await processImports(sourceFileImports, sourceFile.url, processJsImports);
    }
  }
  return resolvedSources;
}

interface FileReference {
  fileName: string;
  pos: number;
  end: number;
}

function getMappedModuleName(
  source: FileReference,
  typeDirectives: Map<FileReference, string>
): string | undefined {
  const { fileName: sourceFileName, pos: sourcePos } = source;
  for (const [{ fileName, pos }, value] of typeDirectives.entries()) {
    if (sourceFileName === fileName && sourcePos === pos) {
      return value;
    }
  }
  return undefined;
}

const typeDirectiveRegEx = /@deno-types\s*=\s*(["'])((?:(?=(\\?))\3.)*?)\1/gi;

const importExportRegEx = /(?:import|export)(?:\s+|\s+[\s\S]*?from\s+)?(["'])((?:(?=(\\?))\3.)*?)\1/;

function parseTypeDirectives(
  sourceCode: string | undefined
): Map<FileReference, string> | undefined {
  if (!sourceCode) {
    return;
  }

  // collect all the directives in the file and their start and end positions
  const directives: FileReference[] = [];
  let maybeMatch: RegExpExecArray | null = null;
  while ((maybeMatch = typeDirectiveRegEx.exec(sourceCode))) {
    const [matchString, , fileName] = maybeMatch;
    const { index: pos } = maybeMatch;
    directives.push({
      fileName,
      pos,
      end: pos + matchString.length,
    });
  }
  if (!directives.length) {
    return;
  }

  // work from the last directive backwards for the next `import`/`export`
  // statement
  directives.reverse();
  const results = new Map<FileReference, string>();
  for (const { end, fileName, pos } of directives) {
    const searchString = sourceCode.substring(end);
    const maybeMatch = importExportRegEx.exec(searchString);
    if (maybeMatch) {
      const [matchString, , targetFileName] = maybeMatch;
      const targetPos =
        end + maybeMatch.index + matchString.indexOf(targetFileName) - 1;
      const target: FileReference = {
        fileName: targetFileName,
        pos: targetPos,
        end: targetPos + targetFileName.length,
      };
      results.set(target, fileName);
    }
    sourceCode = sourceCode.substring(0, pos);
  }

  return results;
}

interface EmmitedSource {
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

interface WriteFileState {
  type: CompilerRequestType;
  bundle?: boolean;
  bundleOutput?: string;
  host?: Host;
  rootNames: string[];
  emitMap?: Record<string, EmmitedSource>;
  sources?: Record<string, string>;
}

// Warning! The values in this enum are duplicated in `cli/msg.rs`
// Update carefully!
enum CompilerRequestType {
  Compile = 0,
  RuntimeCompile = 1,
  RuntimeTranspile = 2,
}

// TODO(bartlomieju): probably could be defined inline?
function createBundleWriteFile(state: WriteFileState): WriteFileCallback {
  return function writeFile(
    _fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    assert(sourceFiles != null);
    assert(state.host);
    assert(state.emitMap);
    assert(state.bundle);
    // we only support single root names for bundles
    assert(state.rootNames.length === 1);
    state.bundleOutput = buildBundle(state.rootNames[0], data, sourceFiles);
  };
}

// TODO(bartlomieju): probably could be defined inline?
function createCompileWriteFile(state: WriteFileState): WriteFileCallback {
  return function writeFile(
    fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    assert(sourceFiles != null);
    assert(state.host);
    assert(state.emitMap);
    assert(!state.bundle);
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

// TODO(Bartlomieju): this check should be done in Rust; there should be no
// console.log here
function processConfigureResponse(
  configResult: ConfigureResponse,
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
  sourceFiles: readonly ts.SourceFile[]
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
      ? `const __exp = await __instantiateAsync("${rootName}");\n`
      : `const __exp = __instantiate("${rootName}");\n`;
    for (const rootExport of rootExports) {
      if (rootExport === "default") {
        instantiate += `export default __exp["${rootExport}"];\n`;
      } else {
        instantiate += `export const ${rootExport} = __exp["${rootExport}"];\n`;
      }
    }
  } else {
    instantiate = hasTla
      ? `await __instantiateAsync("${rootName}");\n`
      : `__instantiate("${rootName}");\n`;
  }
  return `${SYSTEM_LOADER}\n${data}\n${instantiate}`;
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

interface CompilerRequestCompile {
  type: CompilerRequestType.Compile;
  target: CompilerHostTarget;
  rootNames: string[];
  // TODO(ry) add compiler config to this interface.
  // options: ts.CompilerOptions;
  configPath?: string;
  config?: string;
  unstable: boolean;
  bundle: boolean;
  cwd: string;
}

interface CompilerRequestRuntimeCompile {
  type: CompilerRequestType.RuntimeCompile;
  target: CompilerHostTarget;
  rootName: string;
  sources?: Record<string, string>;
  unstable?: boolean;
  bundle?: boolean;
  options?: string;
}

interface CompilerRequestRuntimeTranspile {
  type: CompilerRequestType.RuntimeTranspile;
  sources: Record<string, string>;
  options?: string;
}

type CompilerRequest =
  | CompilerRequestCompile
  | CompilerRequestRuntimeCompile
  | CompilerRequestRuntimeTranspile;

interface CompileResult {
  emitMap?: Record<string, EmmitedSource>;
  bundleOutput?: string;
  diagnostics: Diagnostic;
}

interface RuntimeCompileResult {
  emitMap: Record<string, EmmitedSource>;
  diagnostics: DiagnosticItem[];
}

interface RuntimeBundleResult {
  output: string;
  diagnostics: DiagnosticItem[];
}

async function compile(
  request: CompilerRequestCompile
): Promise<CompileResult> {
  const {
    bundle,
    config,
    configPath,
    rootNames,
    target,
    unstable,
    cwd,
  } = request;
  util.log(">>> compile start", {
    rootNames,
    type: CompilerRequestType[request.type],
  });

  // When a programme is emitted, TypeScript will call `writeFile` with
  // each file that needs to be emitted.  The Deno compiler host delegates
  // this, to make it easier to perform the right actions, which vary
  // based a lot on the request.
  const state: WriteFileState = {
    type: request.type,
    emitMap: {},
    bundle,
    host: undefined,
    rootNames,
  };
  let writeFile: WriteFileCallback;
  if (bundle) {
    writeFile = createBundleWriteFile(state);
  } else {
    writeFile = createCompileWriteFile(state);
  }
  const host = (state.host = new Host({
    bundle,
    target,
    writeFile,
    unstable,
  }));
  let diagnostics: readonly ts.Diagnostic[] = [];

  // if there is a configuration supplied, we need to parse that
  if (config && config.length && configPath) {
    const configResult = host.configure(cwd, configPath, config);
    diagnostics = processConfigureResponse(configResult, configPath) || [];
  }

  // This will recursively analyse all the code for other imports,
  // requesting those from the privileged side, populating the in memory
  // cache which will be used by the host, before resolving.
  const specifiers = rootNames.map((rootName) => {
    return { original: rootName, mapped: rootName };
  });
  const resolvedRootModules = await processImports(
    specifiers,
    undefined,
    bundle || host.getCompilationSettings().checkJs
  );

  // if there was a configuration and no diagnostics with it, we will continue
  // to generate the program and possibly emit it.
  if (diagnostics.length === 0) {
    const options = host.getCompilationSettings();
    const program = ts.createProgram({
      rootNames,
      options,
      host,
      oldProgram: TS_SNAPSHOT_PROGRAM,
    });

    diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !ignoredDiagnostics.includes(code));

    // We will only proceed with the emit if there are no diagnostics.
    if (diagnostics && diagnostics.length === 0) {
      if (bundle) {
        // we only support a single root module when bundling
        assert(resolvedRootModules.length === 1);
        setRootExports(program, resolvedRootModules[0]);
      }
      const emitResult = program.emit();
      assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");
      // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
      // without casting.
      diagnostics = emitResult.diagnostics;
    }
  }

  let bundleOutput = undefined;

  if (bundle) {
    assert(state.bundleOutput);
    bundleOutput = state.bundleOutput;
  }

  assert(state.emitMap);
  const result: CompileResult = {
    emitMap: state.emitMap,
    bundleOutput,
    diagnostics: fromTypeScriptDiagnostic(diagnostics),
  };

  util.log("<<< compile end", {
    rootNames,
    type: CompilerRequestType[request.type],
  });

  return result;
}

async function runtimeCompile(
  request: CompilerRequestRuntimeCompile
): Promise<RuntimeCompileResult | RuntimeBundleResult> {
  const { bundle, options, rootName, sources, target, unstable } = request;

  util.log(">>> runtime compile start", {
    rootName,
    bundle,
    sources: sources ? Object.keys(sources) : undefined,
  });

  // resolve the root name, if there are sources, the root name does not
  // get resolved
  const resolvedRootName = sources ? rootName : resolveModules([rootName])[0];

  // if there are options, convert them into TypeScript compiler options,
  // and resolve any external file references
  let convertedOptions: ts.CompilerOptions | undefined;
  let additionalFiles: string[] | undefined;
  if (options) {
    const result = convertCompilerOptions(options);
    convertedOptions = result.options;
    additionalFiles = result.files;
  }

  const checkJsImports =
    bundle || (convertedOptions && convertedOptions.checkJs);

  // recursively process imports, loading each file into memory.  If there
  // are sources, these files are pulled out of the there, otherwise the
  // files are retrieved from the privileged side
  const specifiers = [
    {
      original: resolvedRootName,
      mapped: resolvedRootName,
    },
  ];
  const rootNames = sources
    ? processLocalImports(sources, specifiers, undefined, checkJsImports)
    : await processImports(specifiers, undefined, checkJsImports);

  if (additionalFiles) {
    // any files supplied in the configuration are resolved externally,
    // even if sources are provided
    const resolvedNames = resolveModules(additionalFiles);
    const resolvedSpecifiers = resolvedNames.map((rn) => {
      return {
        original: rn,
        mapped: rn,
      };
    });
    const additionalImports = await processImports(
      resolvedSpecifiers,
      undefined,
      checkJsImports
    );
    rootNames.push(...additionalImports);
  }

  const state: WriteFileState = {
    type: request.type,
    bundle,
    host: undefined,
    rootNames,
    sources,
    emitMap: {},
    bundleOutput: undefined,
  };
  let writeFile: WriteFileCallback;
  if (bundle) {
    writeFile = createBundleWriteFile(state);
  } else {
    writeFile = createCompileWriteFile(state);
  }

  const host = (state.host = new Host({
    bundle,
    target,
    writeFile,
  }));
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
  if (bundle) {
    compilerOptions.push(DEFAULT_BUNDLER_OPTIONS);
  }
  host.mergeOptions(...compilerOptions);

  const program = ts.createProgram({
    rootNames,
    options: host.getCompilationSettings(),
    host,
    oldProgram: TS_SNAPSHOT_PROGRAM,
  });

  if (bundle) {
    setRootExports(program, rootNames[0]);
  }

  const diagnostics = ts
    .getPreEmitDiagnostics(program)
    .filter(({ code }) => !ignoredDiagnostics.includes(code));

  const emitResult = program.emit();

  assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

  assert(state.emitMap);
  util.log("<<< runtime compile finish", {
    rootName,
    sources: sources ? Object.keys(sources) : undefined,
    bundle,
    emitMap: Object.keys(state.emitMap),
  });

  const maybeDiagnostics = diagnostics.length
    ? fromTypeScriptDiagnostic(diagnostics).items
    : [];

  if (bundle) {
    return {
      diagnostics: maybeDiagnostics,
      output: state.bundleOutput,
    } as RuntimeBundleResult;
  } else {
    return {
      diagnostics: maybeDiagnostics,
      emitMap: state.emitMap,
    } as RuntimeCompileResult;
  }
}

function runtimeTranspile(
  request: CompilerRequestRuntimeTranspile
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
      const result = await compile(request as CompilerRequestCompile);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeCompile: {
      const result = await runtimeCompile(
        request as CompilerRequestRuntimeCompile
      );
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeTranspile: {
      const result = await runtimeTranspile(
        request as CompilerRequestRuntimeTranspile
      );
      globalThis.postMessage(result);
      break;
    }
    default:
      util.log(
        `!!! unhandled CompilerRequestType: ${
          (request as CompilerRequest).type
        } (${CompilerRequestType[(request as CompilerRequest).type]})`
      );
  }
  // Currently Rust shuts down worker after single request
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
