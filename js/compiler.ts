// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as ts from "typescript";
import { assetSourceCode } from "./assets";
import * as deno from "./deno";
import { libdeno, window, globalEval } from "./globals";
import * as os from "./os";
import { RawSourceMap } from "./types";
import { assert, log, notImplemented } from "./util";
import * as sourceMaps from "./v8_source_maps";

const EOL = "\n";
const ASSETS = "$asset$";

// tslint:disable:no-any
type AmdCallback = (...args: any[]) => void;
type AmdErrback = (err: any) => void;
export type AmdFactory = (...args: any[]) => object | void;
// tslint:enable:no-any
export type AmdDefine = (deps: string[], factory: AmdFactory) => void;
type AmdRequire = (
  deps: string[],
  callback: AmdCallback,
  errback?: AmdErrback
) => void;

// The location that a module is being loaded from. This could be a directory,
// like ".", or it could be a module specifier like
// "http://gist.github.com/somefile.ts"
type ContainingFile = string;
// The internal local filename of a compiled module. It will often be something
// like "/home/ry/.deno/gen/f7b4605dfbc4d3bb356e98fda6ceb1481e4a8df5.js"
type ModuleFileName = string;
// The external name of a module - could be a URL or could be a relative path.
// Examples "http://gist.github.com/somefile.ts" or "./somefile.ts"
type ModuleSpecifier = string;
// The compiled source code which is cached in .deno/gen/
type OutputCode = string;

/**
 * Abstraction of the APIs required from the `os` module so they can be
 * easily mocked.
 */
export interface Os {
  codeCache: typeof os.codeCache;
  codeFetch: typeof os.codeFetch;
  exit: typeof os.exit;
}

/**
 * Abstraction of the APIs required from the `typescript` module so they can
 * be easily mocked.
 */
export interface Ts {
  createLanguageService: typeof ts.createLanguageService;
  /* tslint:disable-next-line:max-line-length */
  formatDiagnosticsWithColorAndContext: typeof ts.formatDiagnosticsWithColorAndContext;
}

/**
 * A simple object structure for caching resolved modules and their contents.
 *
 * Named `ModuleMetaData` to clarify it is just a representation of meta data of
 * the module, not the actual module instance.
 */
export class ModuleMetaData {
  public readonly exports = {};
  public scriptSnapshot?: ts.IScriptSnapshot;
  public scriptVersion = "";

  constructor(
    public readonly fileName: string,
    public readonly sourceCode = "",
    public outputCode = ""
  ) {
    if (outputCode !== "" || fileName.endsWith(".d.ts")) {
      this.scriptVersion = "1";
    }
  }
}

/**
 * The required minimal API to allow formatting of TypeScript compiler
 * diagnostics.
 */
const formatDiagnosticsHost: ts.FormatDiagnosticsHost = {
  getCurrentDirectory: () => ".",
  getCanonicalFileName: (fileName: string) => fileName,
  getNewLine: () => EOL
};

/**
 * Throw a module resolution error, when a module is unsuccessfully resolved.
 */
function throwResolutionError(
  message: string,
  moduleSpecifier: ModuleSpecifier,
  containingFile: ContainingFile
): never {
  throw new Error(
    // tslint:disable-next-line:max-line-length
    `Cannot resolve module "${moduleSpecifier}" from "${containingFile}".\n  ${message}`
  );
}

// ts.ScriptKind is not available at runtime, so local enum definition
enum ScriptKind {
  JS = 1,
  TS = 3,
  JSON = 6
}

/**
 * A singleton class that combines the TypeScript Language Service host API
 * with Deno specific APIs to provide an interface for compiling and running
 * TypeScript and JavaScript modules.
 */
export class DenoCompiler implements ts.LanguageServiceHost {
  // Modules are usually referenced by their ModuleSpecifier and ContainingFile,
  // and keeping a map of the resolved module file name allows more efficient
  // future resolution
  private readonly _fileNamesMap = new Map<
    ContainingFile,
    Map<ModuleSpecifier, ModuleFileName>
  >();
  // A reference to global eval, so it can be monkey patched during testing
  private _globalEval = globalEval;
  // A reference to the log utility, so it can be monkey patched during testing
  private _log = log;
  // A map of module file names to module meta data
  private readonly _moduleMetaDataMap = new Map<
    ModuleFileName,
    ModuleMetaData
  >();
  // TODO ideally this are not static and can be influenced by command line
  // arguments
  private readonly _options: Readonly<ts.CompilerOptions> = {
    allowJs: true,
    module: ts.ModuleKind.AMD,
    outDir: "$deno$",
    // TODO https://github.com/denoland/deno/issues/23
    inlineSourceMap: true,
    inlineSources: true,
    stripComments: true,
    target: ts.ScriptTarget.ESNext
  };
  // A reference to the `./os.ts` module, so it can be monkey patched during
  // testing
  private _os: Os = os;
  // Used to contain the script file we are currently running
  private _scriptFileNames: string[] = [];
  // A reference to the TypeScript LanguageService instance so it can be
  // monkey patched during testing
  private _service: ts.LanguageService;
  // A reference to `typescript` module so it can be monkey patched during
  // testing
  private _ts: Ts = ts;
  // A reference to the global scope so it can be monkey patched during
  // testing
  private _window = window;

  /**
   * The TypeScript language service often refers to the resolved fileName of
   * a module, this is a shortcut to avoid unnecessary module resolution logic
   * for modules that may have been initially resolved by a `moduleSpecifier`
   * and `containingFile`.  Also, `resolveModule()` throws when the module
   * cannot be resolved, which isn't always valid when dealing with the
   * TypeScript compiler, but the TypeScript compiler shouldn't be asking about
   * external modules that we haven't told it about yet.
   */
  private _getModuleMetaData(
    fileName: ModuleFileName
  ): ModuleMetaData | undefined {
    return this._moduleMetaDataMap.has(fileName)
      ? this._moduleMetaDataMap.get(fileName)
      : fileName.startsWith(ASSETS)
        ? this.resolveModule(fileName, "")
        : undefined;
  }

  /**
   * Setup being able to map back source references back to their source
   *
   * TODO is this the best place for this?  It is tightly coupled to how the
   * compiler works, but it is also tightly coupled to how the whole runtime
   * environment is bootstrapped.  It also needs efficient access to the
   * `outputCode` of the module information, which exists inside of the
   * compiler instance.
   */
  private _setupSourceMaps(): void {
    sourceMaps.install({
      installPrepareStackTrace: true,
      getGeneratedContents: (fileName: string): string | RawSourceMap => {
        this._log("getGeneratedContents", fileName);
        if (fileName === "gen/bundle/main.js") {
          assert(libdeno.mainSource.length > 0);
          return libdeno.mainSource;
        } else if (fileName === "main.js.map") {
          return libdeno.mainSourceMap;
        } else if (fileName === "deno_main.js") {
          return "";
        } else {
          const moduleMetaData = this._moduleMetaDataMap.get(fileName);
          if (!moduleMetaData) {
            this._log("getGeneratedContents cannot find", fileName);
            return "";
          }
          return moduleMetaData.outputCode;
        }
      }
    });
  }

  private constructor() {
    if (DenoCompiler._instance) {
      throw new TypeError("Attempt to create an additional compiler.");
    }
    this._service = this._ts.createLanguageService(this);
    this._setupSourceMaps();
  }

  // Deno specific compiler API

  /**
   * Retrieve the output of the TypeScript compiler for a given `fileName`.
   */
  compile(fileName: ModuleFileName): OutputCode {
    const service = this._service;
    const output = service.getEmitOutput(fileName);

    // Get the relevant diagnostics - this is 3x faster than
    // `getPreEmitDiagnostics`.
    const diagnostics = [
      ...service.getCompilerOptionsDiagnostics(),
      ...service.getSyntacticDiagnostics(fileName),
      ...service.getSemanticDiagnostics(fileName)
    ];
    if (diagnostics.length > 0) {
      const errMsg = this._ts.formatDiagnosticsWithColorAndContext(
        diagnostics,
        formatDiagnosticsHost
      );
      console.log(errMsg);
      // All TypeScript errors are terminal for deno
      this._os.exit(1);
    }

    assert(!output.emitSkipped, "The emit was skipped for an unknown reason.");

    // Currently we are inlining source maps, there should be only 1 output file
    // See: https://github.com/denoland/deno/issues/23
    assert(
      output.outputFiles.length === 1,
      "Only single file should be output."
    );

    const [outputFile] = output.outputFiles;
    return outputFile.text;
  }

  /**
   * Create a localized AMD `define` function and return it.
   */
  makeDefine(moduleMetaData: ModuleMetaData): AmdDefine {
    const localDefine = (deps: string[], factory: AmdFactory): void => {
      // TypeScript will emit a local require dependency when doing dynamic
      // `import()`
      const localRequire: AmdRequire = (
        deps: string[],
        callback: AmdCallback,
        errback?: AmdErrback
      ): void => {
        this._log("localRequire", deps);
        try {
          const args = deps.map(dep => {
            if (dep in DenoCompiler._builtins) {
              return DenoCompiler._builtins[dep];
            } else {
              const depModuleMetaData = this.run(dep, moduleMetaData.fileName);
              return depModuleMetaData.exports;
            }
          });
          callback(...args);
        } catch (e) {
          if (errback) {
            errback(e);
          } else {
            throw e;
          }
        }
      };
      const localExports = moduleMetaData.exports;
      this._log("localDefine", moduleMetaData.fileName, deps, localExports);
      const args = deps.map(dep => {
        if (dep === "require") {
          return localRequire;
        } else if (dep === "exports") {
          return localExports;
        } else if (dep in DenoCompiler._builtins) {
          return DenoCompiler._builtins[dep];
        } else {
          const depModuleMetaData = this.run(dep, moduleMetaData.fileName);
          return depModuleMetaData.exports;
        }
      });
      factory(...args);
    };
    return localDefine;
  }

  /**
   * Given a `moduleSpecifier` and `containingFile` retrieve the cached
   * `fileName` for a given module.  If the module has yet to be resolved
   * this will return `undefined`.
   */
  resolveFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    this._log("resolveFileName", { moduleSpecifier, containingFile });
    const innerMap = this._fileNamesMap.get(containingFile);
    if (innerMap) {
      return innerMap.get(moduleSpecifier);
    }
    return undefined;
  }

  /**
   * Given a `moduleSpecifier` and `containingFile`, resolve the module and
   * return the `ModuleMetaData`.
   */
  resolveModule(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("resolveModule", { moduleSpecifier, containingFile });
    assert(moduleSpecifier != null && moduleSpecifier.length > 0);
    let fileName = this.resolveFileName(moduleSpecifier, containingFile);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    let sourceCode: string | undefined;
    let outputCode: string | undefined;
    if (
      moduleSpecifier.startsWith(ASSETS) ||
      containingFile.startsWith(ASSETS)
    ) {
      // Assets are compiled into the runtime javascript bundle.
      // we _know_ `.pop()` will return a string, but TypeScript doesn't so
      // not null assertion
      const moduleId = moduleSpecifier.split("/").pop()!;
      const assetName = moduleId.includes(".") ? moduleId : `${moduleId}.d.ts`;
      assert(assetName in assetSourceCode, `No such asset "${assetName}"`);
      sourceCode = assetSourceCode[assetName];
      fileName = `${ASSETS}/${assetName}`;
    } else {
      // We query Rust with a CodeFetch message. It will load the sourceCode,
      // and if there is any outputCode cached, will return that as well.
      let fetchResponse;
      try {
        fetchResponse = this._os.codeFetch(moduleSpecifier, containingFile);
      } catch (e) {
        return throwResolutionError(
          `os.codeFetch message: ${e.message}`,
          moduleSpecifier,
          containingFile
        );
      }
      fileName = fetchResponse.filename || undefined;
      sourceCode = fetchResponse.sourceCode || undefined;
      outputCode = fetchResponse.outputCode || undefined;
    }
    if (!sourceCode || sourceCode.length === 0 || !fileName) {
      return throwResolutionError(
        "Invalid source code or file name.",
        moduleSpecifier,
        containingFile
      );
    }
    this._log("resolveModule sourceCode length ", sourceCode.length);
    this.setFileName(moduleSpecifier, containingFile, fileName);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    const moduleMetaData = new ModuleMetaData(fileName, sourceCode, outputCode);
    this._moduleMetaDataMap.set(fileName, moduleMetaData);
    return moduleMetaData;
  }

  /**
   * Resolve the `fileName` for a given `moduleSpecifier` and `containingFile`
   */
  resolveModuleName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    const moduleMetaData = this.resolveModule(moduleSpecifier, containingFile);
    return moduleMetaData ? moduleMetaData.fileName : undefined;
  }

  /* tslint:disable-next-line:no-any */
  /**
   * Execute a module based on the `moduleSpecifier` and the `containingFile`
   * and return the resulting `FileModule`.
   */
  run(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("run", { moduleSpecifier, containingFile });
    const moduleMetaData = this.resolveModule(moduleSpecifier, containingFile);
    const fileName = moduleMetaData.fileName;
    this._scriptFileNames = [fileName];
    const sourceCode = moduleMetaData.sourceCode;
    let outputCode = moduleMetaData.outputCode;
    if (!outputCode) {
      outputCode = moduleMetaData.outputCode = `${this.compile(
        fileName
      )}\n//# sourceURL=${fileName}`;
      moduleMetaData!.scriptVersion = "1";
      this._os.codeCache(fileName, sourceCode, outputCode);
    }
    this._window.define = this.makeDefine(moduleMetaData);
    this._globalEval(moduleMetaData.outputCode);
    this._window.define = undefined;
    return moduleMetaData!;
  }

  /**
   * Caches the resolved `fileName` in relationship to the `moduleSpecifier`
   * and `containingFile` in order to reduce calls to the privileged side
   * to retrieve the contents of a module.
   */
  setFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile,
    fileName: ModuleFileName
  ): void {
    this._log("setFileName", { moduleSpecifier, containingFile });
    let innerMap = this._fileNamesMap.get(containingFile);
    if (!innerMap) {
      innerMap = new Map();
      this._fileNamesMap.set(containingFile, innerMap);
    }
    innerMap.set(moduleSpecifier, fileName);
  }

  // TypeScript Language Service API

  getCompilationSettings(): ts.CompilerOptions {
    this._log("getCompilationSettings()");
    return this._options;
  }

  getNewLine(): string {
    return EOL;
  }

  getScriptFileNames(): string[] {
    // This is equal to `"files"` in the `tsconfig.json`, therefore we only need
    // to include the actual base source files we are evaluating at the moment,
    // which would be what is set during the `.run()`
    return this._scriptFileNames;
  }

  getScriptKind(fileName: ModuleFileName): ts.ScriptKind {
    this._log("getScriptKind()", fileName);
    const suffix = fileName.substr(fileName.lastIndexOf(".") + 1);
    switch (suffix) {
      case "ts":
        return ScriptKind.TS;
      case "js":
        return ScriptKind.JS;
      case "json":
        return ScriptKind.JSON;
      default:
        return this._options.allowJs ? ScriptKind.JS : ScriptKind.TS;
    }
  }

  getScriptVersion(fileName: ModuleFileName): string {
    this._log("getScriptVersion()", fileName);
    const moduleMetaData = this._getModuleMetaData(fileName);
    return (moduleMetaData && moduleMetaData.scriptVersion) || "";
  }

  getScriptSnapshot(fileName: ModuleFileName): ts.IScriptSnapshot | undefined {
    this._log("getScriptSnapshot()", fileName);
    const moduleMetaData = this._getModuleMetaData(fileName);
    if (moduleMetaData) {
      return (
        moduleMetaData.scriptSnapshot ||
        (moduleMetaData.scriptSnapshot = {
          getText(start, end) {
            return moduleMetaData.sourceCode.substring(start, end);
          },
          getLength() {
            return moduleMetaData.sourceCode.length;
          },
          getChangeRange() {
            return undefined;
          }
        })
      );
    } else {
      return undefined;
    }
  }

  getCurrentDirectory(): string {
    this._log("getCurrentDirectory()");
    return "";
  }

  getDefaultLibFileName(): string {
    this._log("getDefaultLibFileName()");
    const moduleSpecifier = "lib.globals.d.ts";
    const moduleMetaData = this.resolveModule(moduleSpecifier, ASSETS);
    return moduleMetaData.fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    this._log("useCaseSensitiveFileNames");
    return true;
  }

  readFile(path: string): string | undefined {
    this._log("readFile", path);
    return notImplemented();
  }

  fileExists(fileName: string): boolean {
    const moduleMetaData = this._getModuleMetaData(fileName);
    const exists = moduleMetaData != null;
    this._log("fileExists", fileName, exists);
    return exists;
  }

  resolveModuleNames(
    moduleNames: ModuleSpecifier[],
    containingFile: ContainingFile
  ): ts.ResolvedModule[] {
    this._log("resolveModuleNames", { moduleNames, containingFile });
    return moduleNames.map(name => {
      let resolvedFileName;
      if (name === "deno") {
        resolvedFileName = this.resolveModuleName("deno.d.ts", ASSETS);
      } else if (name === "compiler") {
        resolvedFileName = this.resolveModuleName("compiler.d.ts", ASSETS);
      } else if (name === "typescript") {
        resolvedFileName = this.resolveModuleName("typescript.d.ts", ASSETS);
      } else {
        resolvedFileName = this.resolveModuleName(name, containingFile);
      }
      // According to the interface we shouldn't return `undefined` but if we
      // fail to return the same length of modules to those we cannot resolve
      // then TypeScript fails on an assertion that the lengths can't be
      // different, so we have to return an "empty" resolved module
      // TODO: all this does is push the problem downstream, and TypeScript
      // will complain it can't identify the type of the file and throw
      // a runtime exception, so we need to handle missing modules better
      resolvedFileName = resolvedFileName || "";
      // This flags to the compiler to not go looking to transpile functional
      // code, anything that is in `/$asset$/` is just library code
      const isExternalLibraryImport = resolvedFileName.startsWith(ASSETS);
      // TODO: we should be returning a ts.ResolveModuleFull
      return { resolvedFileName, isExternalLibraryImport };
    });
  }

  // Deno specific static properties and methods

  /**
   * Built in modules which can be returned to external modules
   *
   * Placed as a private static otherwise we get use before
   * declared with the `DenoCompiler`
   */
  // tslint:disable-next-line:no-any
  private static _builtins: { [mid: string]: any } = {
    typescript: ts,
    deno,
    compiler: { DenoCompiler, ModuleMetaData }
  };

  private static _instance: DenoCompiler | undefined;

  /**
   * Returns the instance of `DenoCompiler` or creates a new instance.
   */
  static instance(): DenoCompiler {
    return (
      DenoCompiler._instance || (DenoCompiler._instance = new DenoCompiler())
    );
  }
}
