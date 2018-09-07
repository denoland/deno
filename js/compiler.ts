// Copyright 2018 the Deno authors. All rights reserved. MIT license.
/// <amd-module name="compiler"/>
import * as ts from "typescript";
import { assetSourceCode } from "./assets";
import * as deno from "./deno";
import { globalEval } from "./global-eval";
import { libdeno } from "./libdeno";
import { window } from "./globals";
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
export type AmdDefine = (deps: ModuleSpecifier[], factory: AmdFactory) => void;
type AMDRequire = (
  deps: ModuleSpecifier[],
  callback: AmdCallback,
  errback: AmdErrback
) => void;

/**
 * The location that a module is being loaded from. This could be a directory,
 * like `.`, or it could be a module specifier like
 * `http://gist.github.com/somefile.ts`
 */
type ContainingFile = string;
/**
 * The internal local filename of a compiled module. It will often be something
 * like `/home/ry/.deno/gen/f7b4605dfbc4d3bb356e98fda6ceb1481e4a8df5.js`
 */
type ModuleFileName = string;
/**
 * The original resolved resource name.
 * Path to cached module file or URL from which dependency was retrieved
 */
type ModuleId = string;
/**
 * The external name of a module - could be a URL or could be a relative path.
 * Examples `http://gist.github.com/somefile.ts` or `./somefile.ts`
 */
type ModuleSpecifier = string;
/**
 * The compiled source code which is cached in `.deno/gen/`
 */
type OutputCode = string;
/**
 * The original source code
 */
type SourceCode = string;

/**
 * Abstraction of the APIs required from the `os` module so they can be
 * easily mocked.
 * @internal
 */
export interface Os {
  codeCache: typeof os.codeCache;
  codeFetch: typeof os.codeFetch;
  exit: typeof os.exit;
}

/**
 * Abstraction of the APIs required from the `typescript` module so they can
 * be easily mocked.
 * @internal
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
export class ModuleMetaData implements ts.IScriptSnapshot {
  public deps?: ModuleFileName[];
  public readonly exports = {};
  public factory?: AmdFactory;
  public gatheringDeps = false;
  public hasRun = false;
  public scriptVersion = "";

  constructor(
    public readonly moduleId: ModuleId,
    public readonly fileName: ModuleFileName,
    public readonly sourceCode: SourceCode = "",
    public outputCode: OutputCode = ""
  ) {
    if (outputCode !== "" || fileName.endsWith(".d.ts")) {
      this.scriptVersion = "1";
    }
  }

  public getText(start: number, end: number): string {
    return this.sourceCode.substring(start, end);
  }

  public getLength(): number {
    return this.sourceCode.length;
  }

  public getChangeRange(): undefined {
    // Required `IScriptSnapshot` API, but not implemented/needed in deno
    return undefined;
  }
}

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

/**
 * A singleton class that combines the TypeScript Language Service host API
 * with Deno specific APIs to provide an interface for compiling and running
 * TypeScript and JavaScript modules.
 */
export class DenoCompiler
  implements ts.LanguageServiceHost, ts.FormatDiagnosticsHost {
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
  // Contains a queue of modules that have been resolved, but not yet
  // run
  private _runQueue: ModuleMetaData[] = [];
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
   * Drain the run queue, retrieving the arguments for the module
   * factory and calling the module's factory.
   */
  private _drainRunQueue(): void {
    this._log(
      "compiler._drainRunQueue",
      this._runQueue.map(metaData => metaData.fileName)
    );
    let moduleMetaData: ModuleMetaData | undefined;
    while ((moduleMetaData = this._runQueue.shift())) {
      assert(
        moduleMetaData.factory != null,
        "Cannot run module without factory."
      );
      assert(moduleMetaData.hasRun === false, "Module has already been run.");
      // asserts not tracked by TypeScripts, so using not null operator
      moduleMetaData.factory!(...this._getFactoryArguments(moduleMetaData));
      moduleMetaData.hasRun = true;
    }
  }

  /**
   * Get the dependencies for a given module, but don't run the module,
   * just add the module factory to the run queue.
   */
  private _gatherDependencies(moduleMetaData: ModuleMetaData): void {
    this._log("compiler._resolveDependencies", moduleMetaData.fileName);

    // if the module has already run, we can short circuit.
    // it is intentional though that if we have already resolved dependencies,
    // we won't short circuit, as something may have changed, or we might have
    // only collected the dependencies to be able to able to obtain the graph of
    // dependencies
    if (moduleMetaData.hasRun) {
      return;
    }

    this._window.define = this._makeDefine(moduleMetaData);
    this._globalEval(this.compile(moduleMetaData));
    this._window.define = undefined;
  }

  /**
   * Retrieve the arguments to pass a module's factory function.
   */
  // tslint:disable-next-line:no-any
  private _getFactoryArguments(moduleMetaData: ModuleMetaData): any[] {
    if (!moduleMetaData.deps) {
      throw new Error("Cannot get arguments until dependencies resolved.");
    }
    return moduleMetaData.deps.map(dep => {
      if (dep === "require") {
        return this._makeLocalRequire(moduleMetaData);
      }
      if (dep === "exports") {
        return moduleMetaData.exports;
      }
      if (dep in DenoCompiler._builtins) {
        return DenoCompiler._builtins[dep];
      }
      const dependencyMetaData = this._getModuleMetaData(dep);
      assert(dependencyMetaData != null, `Missing dependency "${dep}".`);
      // TypeScript does not track assert, therefore using not null operator
      return dependencyMetaData!.exports;
    });
  }

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
   * Create a localized AMD `define` function and return it.
   */
  private _makeDefine(moduleMetaData: ModuleMetaData): AmdDefine {
    return (deps: ModuleSpecifier[], factory: AmdFactory): void => {
      this._log("compiler.localDefine", moduleMetaData.fileName);
      moduleMetaData.factory = factory;
      // when there are circular dependencies, we need to skip recursing the
      // dependencies
      moduleMetaData.gatheringDeps = true;
      // we will recursively resolve the dependencies for any modules
      moduleMetaData.deps = deps.map(dep => {
        if (
          dep === "require" ||
          dep === "exports" ||
          dep in DenoCompiler._builtins
        ) {
          return dep;
        }
        const dependencyMetaData = this.resolveModule(
          dep,
          moduleMetaData.fileName
        );
        if (!dependencyMetaData.gatheringDeps) {
          this._gatherDependencies(dependencyMetaData);
        }
        return dependencyMetaData.fileName;
      });
      moduleMetaData.gatheringDeps = false;
      if (!this._runQueue.includes(moduleMetaData)) {
        this._runQueue.push(moduleMetaData);
      }
    };
  }

  /**
   * Returns a require that specifically handles the resolution of a transpiled
   * emit of a dynamic ES `import()` from TypeScript.
   */
  private _makeLocalRequire(moduleMetaData: ModuleMetaData): AMDRequire {
    return (
      deps: ModuleSpecifier[],
      callback: AmdCallback,
      errback: AmdErrback
    ): void => {
      log("localRequire", deps);
      assert(
        deps.length === 1,
        "Local require requires exactly one dependency."
      );
      const [moduleSpecifier] = deps;
      try {
        const requiredMetaData = this.run(
          moduleSpecifier,
          moduleMetaData.fileName
        );
        callback(requiredMetaData.exports);
      } catch (e) {
        errback(e);
      }
    };
  }

  /**
   * Given a `moduleSpecifier` and `containingFile` retrieve the cached
   * `fileName` for a given module.  If the module has yet to be resolved
   * this will return `undefined`.
   */
  private _resolveFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    this._log("compiler.resolveFileName", { moduleSpecifier, containingFile });
    const innerMap = this._fileNamesMap.get(containingFile);
    if (innerMap) {
      return innerMap.get(moduleSpecifier);
    }
    return undefined;
  }

  /**
   * Resolve the `fileName` for a given `moduleSpecifier` and `containingFile`
   */
  private _resolveModuleName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    const moduleMetaData = this.resolveModule(moduleSpecifier, containingFile);
    return moduleMetaData ? moduleMetaData.fileName : undefined;
  }

  /**
   * Caches the resolved `fileName` in relationship to the `moduleSpecifier`
   * and `containingFile` in order to reduce calls to the privileged side
   * to retrieve the contents of a module.
   */
  private _setFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile,
    fileName: ModuleFileName
  ): void {
    this._log("compiler.setFileName", { moduleSpecifier, containingFile });
    let innerMap = this._fileNamesMap.get(containingFile);
    if (!innerMap) {
      innerMap = new Map();
      this._fileNamesMap.set(containingFile, innerMap);
    }
    innerMap.set(moduleSpecifier, fileName);
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
        this._log("compiler.getGeneratedContents", fileName);
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
            this._log("compiler.getGeneratedContents cannot find", fileName);
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
   * Retrieve the output of the TypeScript compiler for a given module and
   * cache the result.
   */
  compile(moduleMetaData: ModuleMetaData): OutputCode {
    this._log("compiler.compile", moduleMetaData.fileName);
    if (moduleMetaData.outputCode) {
      return moduleMetaData.outputCode;
    }
    const { fileName, sourceCode } = moduleMetaData;
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
        this
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
    const outputCode = (moduleMetaData.outputCode = `${
      outputFile.text
    }\n//# sourceURL=${fileName}`);
    moduleMetaData.scriptVersion = "1";
    this._os.codeCache(fileName, sourceCode, outputCode);
    return moduleMetaData.outputCode;
  }

  /**
   * For a given module specifier and containing file, return a list of absolute
   * identifiers for dependent modules that are required by this module.
   */
  getModuleDependencies(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName[] {
    assert(
      this._runQueue.length === 0,
      "Cannot get dependencies with modules queued to be run."
    );
    const moduleMetaData = this.resolveModule(moduleSpecifier, containingFile);
    assert(
      !moduleMetaData.hasRun,
      "Cannot get dependencies for a module that has already been run."
    );
    this._gatherDependencies(moduleMetaData);
    const dependencies = this._runQueue.map(
      moduleMetaData => moduleMetaData.moduleId
    );
    // empty the run queue, to free up references to factories we have collected
    // and to ensure that if there is a further invocation of `.run()` the
    // factories don't get called
    this._runQueue = [];
    return dependencies;
  }

  /**
   * Given a `moduleSpecifier` and `containingFile`, resolve the module and
   * return the `ModuleMetaData`.
   */
  resolveModule(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("compiler.resolveModule", { moduleSpecifier, containingFile });
    assert(moduleSpecifier != null && moduleSpecifier.length > 0);
    let fileName = this._resolveFileName(moduleSpecifier, containingFile);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    let moduleId: ModuleId = "";
    let sourceCode: SourceCode | undefined;
    let outputCode: OutputCode | undefined;
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
      moduleId = fetchResponse.moduleName || "";
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
    this._log("resolveModule sourceCode length:", sourceCode.length);
    this._log("resolveModule has outputCode:", !!outputCode);
    this._setFileName(moduleSpecifier, containingFile, fileName);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    const moduleMetaData = new ModuleMetaData(
      moduleId,
      fileName,
      sourceCode,
      outputCode
    );
    this._moduleMetaDataMap.set(fileName, moduleMetaData);
    return moduleMetaData;
  }

  /**
   * Load and run a module and all of its dependencies based on a module
   * specifier and a containing file
   */
  run(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("compiler.run", { moduleSpecifier, containingFile });
    const moduleMetaData = this.resolveModule(moduleSpecifier, containingFile);
    this._scriptFileNames = [moduleMetaData.fileName];
    if (!moduleMetaData.deps) {
      this._gatherDependencies(moduleMetaData);
    }
    this._drainRunQueue();
    return moduleMetaData;
  }

  // TypeScript Language Service and Format Diagnostic Host API

  getCanonicalFileName(fileName: string): string {
    this._log("getCanonicalFileName", fileName);
    return fileName;
  }

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
        return ts.ScriptKind.TS;
      case "js":
        return ts.ScriptKind.JS;
      case "json":
        return ts.ScriptKind.JSON;
      default:
        return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
    }
  }

  getScriptVersion(fileName: ModuleFileName): string {
    this._log("getScriptVersion()", fileName);
    const moduleMetaData = this._getModuleMetaData(fileName);
    return (moduleMetaData && moduleMetaData.scriptVersion) || "";
  }

  getScriptSnapshot(fileName: ModuleFileName): ts.IScriptSnapshot | undefined {
    this._log("getScriptSnapshot()", fileName);
    return this._getModuleMetaData(fileName);
  }

  getCurrentDirectory(): string {
    this._log("getCurrentDirectory()");
    return "";
  }

  getDefaultLibFileName(): string {
    this._log("getDefaultLibFileName()");
    const moduleSpecifier = "globals.d.ts";
    const moduleMetaData = this.resolveModule(moduleSpecifier, ASSETS);
    return moduleMetaData.fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    this._log("useCaseSensitiveFileNames()");
    return true;
  }

  readFile(path: string): string | undefined {
    this._log("readFile()", path);
    return notImplemented();
  }

  fileExists(fileName: string): boolean {
    const moduleMetaData = this._getModuleMetaData(fileName);
    const exists = moduleMetaData != null;
    this._log("fileExists()", fileName, exists);
    return exists;
  }

  resolveModuleNames(
    moduleNames: ModuleSpecifier[],
    containingFile: ContainingFile
  ): ts.ResolvedModule[] {
    this._log("resolveModuleNames()", { moduleNames, containingFile });
    return moduleNames.map(name => {
      let resolvedFileName;
      if (name === "deno" || name === "compiler") {
        // builtin modules are part of `globals.d.ts`
        resolvedFileName = this._resolveModuleName("globals.d.ts", ASSETS);
      } else if (name === "typescript") {
        resolvedFileName = this._resolveModuleName("typescript.d.ts", ASSETS);
      } else {
        resolvedFileName = this._resolveModuleName(name, containingFile);
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
