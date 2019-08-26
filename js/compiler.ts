// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as ts from "typescript";
import { assetSourceCode } from "./assets";
import { bold, cyan, yellow } from "./colors";
import { Console } from "./console";
import { core } from "./core";
import { Diagnostic, fromTypeScriptDiagnostic } from "./diagnostics";
import { cwd } from "./dir";
import * as dispatch from "./dispatch";
import { sendSync } from "./dispatch_json";
import * as os from "./os";
import { TextEncoder } from "./text_encoding";
import { getMappedModuleName, parseTypeDirectives } from "./type_directives";
import { assert, notImplemented } from "./util";
import * as util from "./util";
import { window } from "./window";
import { postMessage, workerClose, workerMain } from "./workers";
import { writeFileSync } from "./write_file";

// Warning! The values in this enum are duplicated in cli/msg.rs
// Update carefully!
enum MediaType {
  JavaScript = 0,
  TypeScript = 1,
  Json = 2,
  Unknown = 3
}

// Startup boilerplate. This is necessary because the compiler has its own
// snapshot. (It would be great if we could remove these things or centralize
// them somewhere else.)
const console = new Console(core.print);
window.console = console;
window.workerMain = workerMain;
export default function denoMain(): void {
  os.start(true, "TS");
}

const ASSETS = "$asset$";
const OUT_DIR = "$deno$";

/** The format of the work message payload coming from the privileged side */
interface CompilerReq {
  rootNames: string[];
  bundle?: string;
  // TODO(ry) add compiler config to this interface.
  // options: ts.CompilerOptions;
  configPath?: string;
  config?: string;
}

interface ConfigureResponse {
  ignoredOptions?: string[];
  diagnostics?: ts.Diagnostic[];
}

/** Options that either do nothing in Deno, or would cause undesired behavior
 * if modified. */
const ignoredCompilerOptions: ReadonlyArray<string> = [
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

interface SourceFile {
  moduleName: string | undefined;
  filename: string | undefined;
  mediaType: MediaType;
  sourceCode: string | undefined;
  typeDirectives?: Record<string, string>;
}

interface EmitResult {
  emitSkipped: boolean;
  diagnostics?: Diagnostic;
}

/** Ops to Rust to resolve and fetch a modules meta data. */
function fetchSourceFile(specifier: string, referrer: string): SourceFile {
  util.log("compiler.fetchSourceFile", { specifier, referrer });
  const res = sendSync(dispatch.OP_FETCH_SOURCE_FILE, {
    specifier,
    referrer
  });

  return {
    ...res,
    typeDirectives: parseTypeDirectives(res.sourceCode)
  };
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
  sendSync(dispatch.OP_CACHE, { extension, moduleId, contents });
}

const encoder = new TextEncoder();

/** Given a fileName and the data, emit the file to the file system. */
function emitBundle(fileName: string, data: string): void {
  // For internal purposes, when trying to emit to `$deno$` just no-op
  if (fileName.startsWith("$deno$")) {
    console.warn("skipping emitBundle", fileName);
    return;
  }
  const encodedData = encoder.encode(data);
  console.log(`Emitting bundle to "${fileName}"`);
  writeFileSync(fileName, encodedData);
  console.log(`${humanFileSize(encodedData.length)} emitted.`);
}

/** Returns the TypeScript Extension enum for a given media type. */
function getExtension(fileName: string, mediaType: MediaType): ts.Extension {
  switch (mediaType) {
    case MediaType.JavaScript:
      return ts.Extension.Js;
    case MediaType.TypeScript:
      return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
    case MediaType.Json:
      return ts.Extension.Json;
    case MediaType.Unknown:
    default:
      throw TypeError("Cannot resolve extension.");
  }
}

class Host implements ts.CompilerHost {
  private _extensionCache: Record<string, ts.Extension> = {};

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
    target: ts.ScriptTarget.ESNext
  };

  private _sourceFileCache: Record<string, SourceFile> = {};

  private _resolveModule(specifier: string, referrer: string): SourceFile {
    util.log("host._resolveModule", { specifier, referrer });
    // Handle built-in assets specially.
    if (specifier.startsWith(ASSETS)) {
      const moduleName = specifier.split("/").pop()!;
      if (moduleName in this._sourceFileCache) {
        return this._sourceFileCache[moduleName];
      }
      const assetName = moduleName.includes(".")
        ? moduleName
        : `${moduleName}.d.ts`;
      assert(assetName in assetSourceCode, `No such asset "${assetName}"`);
      const sourceCode = assetSourceCode[assetName];
      const sourceFile = {
        moduleName,
        filename: specifier,
        mediaType: MediaType.TypeScript,
        sourceCode
      };
      this._sourceFileCache[moduleName] = sourceFile;
      return sourceFile;
    }
    const sourceFile = fetchSourceFile(specifier, referrer);
    assert(sourceFile.moduleName != null);
    const { moduleName } = sourceFile;
    if (!(moduleName! in this._sourceFileCache)) {
      this._sourceFileCache[moduleName!] = sourceFile;
    }
    return sourceFile;
  }

  /* Deno specific APIs */

  /** Provides the `ts.HostCompiler` interface for Deno.
   *
   * @param _bundle Set to a string value to configure the host to write out a
   *   bundle instead of caching individual files.
   */
  constructor(private _bundle?: string) {
    if (this._bundle) {
      // options we need to change when we are generating a bundle
      const bundlerOptions: ts.CompilerOptions = {
        module: ts.ModuleKind.AMD,
        inlineSourceMap: true,
        outDir: undefined,
        outFile: `${OUT_DIR}/bundle.js`,
        sourceMap: false
      };
      Object.assign(this._options, bundlerOptions);
    }
  }

  /** Take a configuration string, parse it, and use it to merge with the
   * compiler's configuration options.  The method returns an array of compiler
   * options which were ignored, or `undefined`.
   */
  configure(path: string, configurationText: string): ConfigureResponse {
    util.log("host.configure", path);
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

  fileExists(fileName: string): boolean {
    if (fileName.endsWith("package.json")) {
      throw new TypeError("Automatic type resolution not supported");
    }
    return notImplemented();
  }

  getCanonicalFileName(fileName: string): string {
    // console.log("getCanonicalFileName", fileName);
    return fileName;
  }

  getCompilationSettings(): ts.CompilerOptions {
    util.log("getCompilationSettings()");
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
    assert(!shouldCreateNewSourceFile);
    util.log("getSourceFile", fileName);
    const sourceFile =
      fileName in this._sourceFileCache
        ? this._sourceFileCache[fileName]
        : this._resolveModule(fileName, ".");
    assert(sourceFile != null);
    if (!sourceFile.sourceCode) {
      return undefined;
    }
    return ts.createSourceFile(
      fileName,
      sourceFile.sourceCode,
      languageVersion
    );
  }

  readFile(_fileName: string): string | undefined {
    return notImplemented();
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string
  ): Array<ts.ResolvedModuleFull | undefined> {
    util.log("resolveModuleNames()", { moduleNames, containingFile });
    const typeDirectives: Record<string, string> | undefined =
      containingFile in this._sourceFileCache
        ? this._sourceFileCache[containingFile].typeDirectives
        : undefined;
    return moduleNames.map(
      (moduleName): ts.ResolvedModuleFull | undefined => {
        const mappedModuleName = getMappedModuleName(
          moduleName,
          containingFile,
          typeDirectives
        );
        const sourceFile = this._resolveModule(
          mappedModuleName,
          containingFile
        );
        if (sourceFile.moduleName) {
          const resolvedFileName = sourceFile.moduleName;
          // This flags to the compiler to not go looking to transpile functional
          // code, anything that is in `/$asset$/` is just library code
          const isExternalLibraryImport = moduleName.startsWith(ASSETS);
          const extension = getExtension(
            resolvedFileName,
            sourceFile.mediaType
          );
          this._extensionCache[resolvedFileName] = extension;

          return {
            resolvedFileName,
            isExternalLibraryImport,
            extension
          };
        } else {
          return undefined;
        }
      }
    );
  }

  useCaseSensitiveFileNames(): boolean {
    return true;
  }

  writeFile(
    fileName: string,
    data: string,
    writeByteOrderMark: boolean,
    onError?: (message: string) => void,
    sourceFiles?: ReadonlyArray<ts.SourceFile>
  ): void {
    util.log("writeFile", fileName);
    try {
      if (this._bundle) {
        emitBundle(this._bundle, data);
      } else {
        assert(sourceFiles != null && sourceFiles.length == 1);
        const sourceFileName = sourceFiles![0].fileName;
        const maybeExtension = this._extensionCache[sourceFileName];

        if (maybeExtension) {
          // NOTE: If it's a `.json` file we don't want to write it to disk.
          // JSON files are loaded and used by TS compiler to check types, but we don't want
          // to emit them to disk because output file is the same as input file.
          if (maybeExtension === ts.Extension.Json) {
            return;
          }

          // NOTE: JavaScript files are only emitted to disk if `checkJs` option in on
          if (maybeExtension === ts.Extension.Js && !this._options.checkJs) {
            return;
          }
        }

        if (fileName.endsWith(".map")) {
          // Source Map
          cache(".map", sourceFileName, data);
        } else if (fileName.endsWith(".js") || fileName.endsWith(".json")) {
          // Compiled JavaScript
          cache(".js", sourceFileName, data);
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
  window.onmessage = ({ data }: { data: CompilerReq }): void => {
    const { rootNames, configPath, config, bundle } = data;
    const host = new Host(bundle);

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
        if (bundle) {
          console.log(`Bundling "${bundle}"`);
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

    // The compiler isolate exits after a single message.
    workerClose();
  };
};
