// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO(ry) Combine this implementation with //deno_typescript/compiler_main.js

import "./globals.ts";
import "./ts_global.d.ts";

import { bold, cyan, yellow } from "./colors.ts";
import { Console } from "./console.ts";
import { core } from "./core.ts";
import { Diagnostic, fromTypeScriptDiagnostic } from "./diagnostics.ts";
import { cwd } from "./dir.ts";
import * as dispatch from "./dispatch.ts";
import { sendAsync, sendSync } from "./dispatch_json.ts";
import * as os from "./os.ts";
import { TextEncoder } from "./text_encoding.ts";
import { getMappedModuleName, parseTypeDirectives } from "./type_directives.ts";
import { assert, notImplemented } from "./util.ts";
import * as util from "./util.ts";
import { window } from "./window.ts";
import { postMessage, workerClose, workerMain } from "./workers.ts";
import { writeFileSync } from "./write_file.ts";

// Warning! The values in this enum are duplicated in cli/msg.rs
// Update carefully!
enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  TSX = 3,
  Json = 4,
  Unknown = 5
}

// Warning! The values in this enum are duplicated in cli/msg.rs
// Update carefully!
enum CompilerRequestType {
  Compile = 0,
  Bundle = 1
}

// Startup boilerplate. This is necessary because the compiler has its own
// snapshot. (It would be great if we could remove these things or centralize
// them somewhere else.)
const console = new Console(core.print);
window.console = console;
window.workerMain = workerMain;
function denoMain(): void {
  os.start(true, "TS");
}
window["denoMain"] = denoMain;

const ASSETS = "$asset$";
const OUT_DIR = "$deno$";
const BUNDLE_LOADER = "bundle_loader.js";

/** The format of the work message payload coming from the privileged side */
type CompilerRequest = {
  rootNames: string[];
  // TODO(ry) add compiler config to this interface.
  // options: ts.CompilerOptions;
  configPath?: string;
  config?: string;
} & (
  | {
      type: CompilerRequestType.Compile;
    }
  | {
      type: CompilerRequestType.Bundle;
      outFile?: string;
    });

interface ConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
}

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
  "lib",
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

/** The shape of the SourceFile that comes from the privileged side */
interface SourceFileJson {
  url: string;
  filename: string;
  mediaType: MediaType;
  sourceCode: string;
}

/** A self registering abstraction of source files. */
class SourceFile {
  extension!: ts.Extension;
  filename!: string;

  /** An array of tuples which represent the imports for the source file.  The
   * first element is the one that will be requested at compile time, the
   * second is the one that should be actually resolved.  This provides the
   * feature of type directives for Deno. */
  importedFiles?: Array<[string, string]>;

  mediaType!: MediaType;
  processed = false;
  sourceCode!: string;
  tsSourceFile?: ts.SourceFile;
  url!: string;

  constructor(json: SourceFileJson) {
    if (SourceFile._moduleCache.has(json.url)) {
      throw new TypeError("SourceFile already exists");
    }
    Object.assign(this, json);
    this.extension = getExtension(this.url, this.mediaType);
    SourceFile._moduleCache.set(this.url, this);
  }

  /** Cache the source file to be able to be retrieved by `moduleSpecifier` and
   * `containingFile`. */
  cache(moduleSpecifier: string, containingFile: string): void {
    let innerCache = SourceFile._specifierCache.get(containingFile);
    if (!innerCache) {
      innerCache = new Map();
      SourceFile._specifierCache.set(containingFile, innerCache);
    }
    innerCache.set(moduleSpecifier, this);
  }

  /** Process the imports for the file and return them. */
  imports(): Array<[string, string]> {
    if (this.processed) {
      throw new Error("SourceFile has already been processed.");
    }
    assert(this.sourceCode != null);
    const preProcessedFileInfo = ts.preProcessFile(
      this.sourceCode!,
      true,
      true
    );
    this.processed = true;
    const files = (this.importedFiles = [] as Array<[string, string]>);

    function process(references: ts.FileReference[]): void {
      for (const { fileName } of references) {
        files.push([fileName, fileName]);
      }
    }

    const {
      importedFiles,
      referencedFiles,
      libReferenceDirectives,
      typeReferenceDirectives
    } = preProcessedFileInfo;
    const typeDirectives = parseTypeDirectives(this.sourceCode);
    if (typeDirectives) {
      for (const importedFile of importedFiles) {
        files.push([
          importedFile.fileName,
          getMappedModuleName(importedFile, typeDirectives)
        ]);
      }
    } else {
      process(importedFiles);
    }
    process(referencedFiles);
    process(libReferenceDirectives);
    process(typeReferenceDirectives);
    return files;
  }

  /** A cache of all the source files which have been loaded indexed by the
   * url. */
  private static _moduleCache: Map<string, SourceFile> = new Map();

  /** A cache of source files based on module specifiers and containing files
   * which is used by the TypeScript compiler to resolve the url */
  private static _specifierCache: Map<
    string,
    Map<string, SourceFile>
  > = new Map();

  /** Retrieve a `SourceFile` based on a `moduleSpecifier` and `containingFile`
   * or return `undefined` if not preset. */
  static getUrl(
    moduleSpecifier: string,
    containingFile: string
  ): string | undefined {
    const containingCache = this._specifierCache.get(containingFile);
    if (containingCache) {
      const sourceFile = containingCache.get(moduleSpecifier);
      return sourceFile && sourceFile.url;
    }
    return undefined;
  }

  /** Retrieve a `SourceFile` based on a `url` */
  static get(url: string): SourceFile | undefined {
    return this._moduleCache.get(url);
  }
}

interface EmitResult {
  emitSkipped: boolean;
  diagnostics?: Diagnostic;
}

/** Ops to Rust to resolve special static assets. */
function fetchAsset(name: string): string {
  return sendSync(dispatch.OP_FETCH_ASSET, { name });
}

/** Ops to Rust to resolve and fetch modules meta data. */
function fetchSourceFiles(
  specifiers: string[],
  referrer: string
): Promise<SourceFileJson[]> {
  util.log("compiler::fetchSourceFiles", { specifiers, referrer });
  return sendAsync(dispatch.OP_FETCH_SOURCE_FILES, {
    specifiers,
    referrer
  });
}

/** Recursively process the imports of modules, generating `SourceFile`s of any
 * imported files.
 *
 * Specifiers are supplied in an array of tupples where the first is the
 * specifier that will be requested in the code and the second is the specifier
 * that should be actually resolved. */
async function processImports(
  specifiers: Array<[string, string]>,
  referrer = ""
): Promise<SourceFileJson[]> {
  if (!specifiers.length) {
    return;
  }
  const sources = specifiers.map(([, moduleSpecifier]) => moduleSpecifier);
  const sourceFiles = await fetchSourceFiles(sources, referrer);
  assert(sourceFiles.length === specifiers.length);
  for (let i = 0; i < sourceFiles.length; i++) {
    const sourceFileJson = sourceFiles[i];
    const sourceFile =
      SourceFile.get(sourceFileJson.url) || new SourceFile(sourceFileJson);
    sourceFile.cache(specifiers[i][0], referrer);
    if (!sourceFile.processed) {
      await processImports(sourceFile.imports(), sourceFile.url);
    }
  }
  return sourceFiles;
}

/** Utility function to turn the number of bytes into a human readable
 * unit */
function humanFileSize(bytes: number): string {
  const thresh = 1000;
  if (Math.abs(bytes) < thresh) {
    return bytes + " B";
  }
  const units = ["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  let u = -1;
  do {
    bytes /= thresh;
    ++u;
  } while (Math.abs(bytes) >= thresh && u < units.length - 1);
  return `${bytes.toFixed(1)} ${units[u]}`;
}

/** Ops to rest for caching source map and compiled js */
function cache(extension: string, moduleId: string, contents: string): void {
  util.log("compiler::cache", { extension, moduleId });
  sendSync(dispatch.OP_CACHE, { extension, moduleId, contents });
}

const encoder = new TextEncoder();

/** Given a fileName and the data, emit the file to the file system. */
function emitBundle(
  rootNames: string[],
  fileName: string | undefined,
  data: string,
  sourceFiles: readonly ts.SourceFile[]
): void {
  // For internal purposes, when trying to emit to `$deno$` just no-op
  if (fileName && fileName.startsWith("$deno$")) {
    console.warn("skipping emitBundle", fileName);
    return;
  }
  const loader = fetchAsset(BUNDLE_LOADER);
  // when outputting to AMD and a single outfile, TypeScript makes up the module
  // specifiers which are used to define the modules, and doesn't expose them
  // publicly, so we have to try to replicate
  const sources = sourceFiles.map(sf => sf.fileName);
  const sharedPath = util.commonPath(sources);
  rootNames = rootNames.map(id =>
    id.replace(sharedPath, "").replace(/\.\w+$/i, "")
  );
  const instantiate = `instantiate(${JSON.stringify(rootNames)});\n`;
  const bundle = `${loader}\n${data}\n${instantiate}`;
  if (fileName) {
    const encodedData = encoder.encode(bundle);
    console.warn(`Emitting bundle to "${fileName}"`);
    writeFileSync(fileName, encodedData);
    console.warn(`${humanFileSize(encodedData.length)} emitted.`);
  } else {
    console.log(bundle);
  }
}

/** Returns the TypeScript Extension enum for a given media type. */
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
      return ts.Extension.Json;
    case MediaType.Unknown:
    default:
      throw TypeError("Cannot resolve extension.");
  }
}

class Host implements ts.CompilerHost {
  private readonly _options: ts.CompilerOptions = {
    allowJs: true,
    allowNonTsExtensions: true,
    checkJs: false,
    esModuleInterop: true,
    module: ts.ModuleKind.ESNext,
    outDir: OUT_DIR,
    resolveJsonModule: true,
    sourceMap: true,
    stripComments: true,
    target: ts.ScriptTarget.ESNext,
    jsx: ts.JsxEmit.React
  };

  private _getAsset(filename: string): SourceFile {
    const sourceFile = SourceFile.get(filename);
    if (sourceFile) {
      return sourceFile;
    }
    const url = filename.split("/").pop()!;
    const assetName = url.includes(".") ? url : `${url}.d.ts`;
    const sourceCode = fetchAsset(assetName);
    return new SourceFile({
      url,
      filename,
      mediaType: MediaType.TypeScript,
      sourceCode
    });
  }

  /* Deno specific APIs */

  /** Provides the `ts.HostCompiler` interface for Deno.
   *
   * @param _rootNames A set of modules that are the ones that should be
   *   instantiated first.  Used when generating a bundle.
   * @param _bundle Set to a string value to configure the host to write out a
   *   bundle instead of caching individual files.
   */
  constructor(
    private _requestType: CompilerRequestType,
    private _rootNames: string[],
    private _outFile?: string
  ) {
    if (this._requestType === CompilerRequestType.Bundle) {
      // options we need to change when we are generating a bundle
      const bundlerOptions: ts.CompilerOptions = {
        module: ts.ModuleKind.AMD,
        outDir: undefined,
        outFile: `${OUT_DIR}/bundle.js`,
        // disabled until we have effective way to modify source maps
        sourceMap: false
      };
      Object.assign(this._options, bundlerOptions);
    }
  }

  /** Take a configuration string, parse it, and use it to merge with the
   * compiler's configuration options.  The method returns an array of compiler
   * options which were ignored, or `undefined`. */
  configure(path: string, configurationText: string): ConfigureResponse {
    util.log("compiler::host.configure", path);
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
    return ASSETS + "/lib.deno_runtime.d.ts";
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
      if (!sourceFile!.tsSourceFile) {
        sourceFile!.tsSourceFile = ts.createSourceFile(
          fileName,
          sourceFile!.sourceCode,
          languageVersion
        );
      }
      return sourceFile!.tsSourceFile;
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
    onError?: (message: string) => void,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    util.log("compiler::host.writeFile", fileName);
    try {
      assert(sourceFiles != null);
      if (this._requestType === CompilerRequestType.Bundle) {
        emitBundle(this._rootNames, this._outFile, data, sourceFiles!);
      } else {
        assert(sourceFiles.length == 1);
        const url = sourceFiles![0].fileName;
        const sourceFile = SourceFile.get(url);

        if (sourceFile) {
          // NOTE: If it's a `.json` file we don't want to write it to disk.
          // JSON files are loaded and used by TS compiler to check types, but we don't want
          // to emit them to disk because output file is the same as input file.
          if (sourceFile.extension === ts.Extension.Json) {
            return;
          }

          // NOTE: JavaScript files are only emitted to disk if `checkJs` option in on
          if (
            sourceFile.extension === ts.Extension.Js &&
            !this._options.checkJs
          ) {
            return;
          }
        }

        if (fileName.endsWith(".map")) {
          // Source Map
          cache(".map", url, data);
        } else if (fileName.endsWith(".js") || fileName.endsWith(".json")) {
          // Compiled JavaScript
          cache(".js", url, data);
        } else {
          assert(false, "Trying to cache unhandled file type " + fileName);
        }
      }
    } catch (e) {
      if (onError) {
        onError(String(e));
      } else {
        throw e;
      }
    }
  }
}

// provide the "main" function that will be called by the privileged side when
// lazy instantiating the compiler web worker
window.compilerMain = function compilerMain(): void {
  // workerMain should have already been called since a compiler is a worker.
  window.onmessage = async ({
    data: request
  }: {
    data: CompilerRequest;
  }): Promise<void> => {
    const { rootNames, configPath, config } = request;
    util.log(">>> compile start", {
      rootNames,
      type: CompilerRequestType[request.type]
    });

    // This will recursively analyse all the code for other imports, requesting
    // those from the privileged side, populating the in memory cache which
    // will be used by the host, before resolving.
    const resolvedRootModules = (await processImports(
      rootNames.map(rootName => [rootName, rootName])
    )).map(info => info.url);

    const host = new Host(
      request.type,
      resolvedRootModules,
      request.type === CompilerRequestType.Bundle ? request.outFile : undefined
    );
    let emitSkipped = true;
    let diagnostics: ts.Diagnostic[] | undefined;

    // if there is a configuration supplied, we need to parse that
    if (config && config.length && configPath) {
      const configResult = host.configure(configPath, config);
      const ignoredOptions = configResult.ignoredOptions;
      diagnostics = configResult.diagnostics;
      if (ignoredOptions) {
        console.warn(
          yellow(`Unsupported compiler options in "${configPath}"\n`) +
            cyan(`  The following options were ignored:\n`) +
            `    ${ignoredOptions
              .map((value): string => bold(value))
              .join(", ")}`
        );
      }
    }

    // if there was a configuration and no diagnostics with it, we will continue
    // to generate the program and possibly emit it.
    if (!diagnostics || (diagnostics && diagnostics.length === 0)) {
      const options = host.getCompilationSettings();
      const program = ts.createProgram(rootNames, options, host);

      diagnostics = ts.getPreEmitDiagnostics(program).filter(
        ({ code }): boolean => {
          // TS1103: 'for-await-of' statement is only allowed within an async
          // function or async generator.
          if (code === 1103) return false;
          // TS1308: 'await' expression is only allowed within an async
          // function.
          if (code === 1308) return false;
          // TS2691: An import path cannot end with a '.ts' extension. Consider
          // importing 'bad-module' instead.
          if (code === 2691) return false;
          // TS5009: Cannot find the common subdirectory path for the input files.
          if (code === 5009) return false;
          // TS5055: Cannot write file
          // 'http://localhost:4545/tests/subdir/mt_application_x_javascript.j4.js'
          // because it would overwrite input file.
          if (code === 5055) return false;
          // TypeScript is overly opinionated that only CommonJS modules kinds can
          // support JSON imports.  Allegedly this was fixed in
          // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
          // so we will ignore complaints about this compiler setting.
          if (code === 5070) return false;
          return true;
        }
      );

      // We will only proceed with the emit if there are no diagnostics.
      if (diagnostics && diagnostics.length === 0) {
        if (request.type === CompilerRequestType.Bundle) {
          // warning so it goes to stderr instead of stdout
          console.warn(`Bundling "${resolvedRootModules.join(`", "`)}"`);
        }
        const emitResult = program.emit();
        emitSkipped = emitResult.emitSkipped;
        // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
        // without casting.
        diagnostics = emitResult.diagnostics as ts.Diagnostic[];
      }
    }

    const result: EmitResult = {
      emitSkipped,
      diagnostics: diagnostics.length
        ? fromTypeScriptDiagnostic(diagnostics)
        : undefined
    };

    postMessage(result);

    util.log("<<< compile end", {
      rootNames,
      type: CompilerRequestType[request.type]
    });

    // The compiler isolate exits after a single message.
    workerClose();
  };
};
