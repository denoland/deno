// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as ts from "typescript";
import { MediaType } from "gen/msg_generated";

import { assetSourceCode } from "./assets";
import { libdeno } from "./libdeno";
import * as os from "./os";
import { CodeProvider } from "./runner";
import { RawSourceMap } from "./types";
import { assert, log, notImplemented } from "./util";

const EOL = "\n";
const ASSETS = "$asset$";
const LIB_RUNTIME = "lib.deno_runtime.d.ts";

/** The location that a module is being loaded from. This could be a directory,
 * like `.`, or it could be a module specifier like
 * `http://gist.github.com/somefile.ts`
 */
type ContainingFile = string;
/** The internal local filename of a compiled module. It will often be something
 * like `/home/ry/.deno/gen/f7b4605dfbc4d3bb356e98fda6ceb1481e4a8df5.js`
 */
type ModuleFileName = string;
/** The original resolved resource name.
 * Path to cached module file or URL from which dependency was retrieved
 */
type ModuleId = string;
/** The external name of a module - could be a URL or could be a relative path.
 * Examples `http://gist.github.com/somefile.ts` or `./somefile.ts`
 */
type ModuleSpecifier = string;
/** The compiled source code which is cached in `.deno/gen/` */
type OutputCode = string;
/** The original source code */
type SourceCode = string;
/** The output source map */
type SourceMap = string;

/** Abstraction of the APIs required from the `os` module so they can be
 * easily mocked.
 * @internal
 */
export interface Os {
  codeCache: typeof os.codeCache;
  codeFetch: typeof os.codeFetch;
  exit: typeof os.exit;
}

/** Abstraction of the APIs required from the `typescript` module so they can
 * be easily mocked.
 * @internal
 */
export interface Ts {
  createLanguageService: typeof ts.createLanguageService;
  /* tslint:disable-next-line:max-line-length */
  formatDiagnosticsWithColorAndContext: typeof ts.formatDiagnosticsWithColorAndContext;
}

/** A simple object structure for caching resolved modules and their contents.
 *
 * Named `ModuleMetaData` to clarify it is just a representation of meta data of
 * the module, not the actual module instance.
 */
export class ModuleMetaData implements ts.IScriptSnapshot {
  public scriptVersion = "";

  constructor(
    public readonly moduleId: ModuleId,
    public readonly fileName: ModuleFileName,
    public readonly mediaType: MediaType,
    public readonly sourceCode: SourceCode = "",
    public outputCode: OutputCode = "",
    public sourceMap: SourceMap | RawSourceMap = ""
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

function getExtension(
  fileName: ModuleFileName,
  mediaType: MediaType
): ts.Extension | undefined {
  switch (mediaType) {
    case MediaType.JavaScript:
      return ts.Extension.Js;
    case MediaType.TypeScript:
      return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
    case MediaType.Json:
      return ts.Extension.Json;
    case MediaType.Unknown:
    default:
      return undefined;
  }
}

/** Generate output code for a provided JSON string along with its source. */
export function jsonAmdTemplate(
  jsonString: string,
  sourceFileName: string
): OutputCode {
  // tslint:disable-next-line:max-line-length
  return `define([], function() { return JSON.parse(\`${jsonString}\`); });\n//# sourceURL=${sourceFileName}`;
}

/** A singleton class that combines the TypeScript Language Service host API
 * with Deno specific APIs to provide an interface for compiling and running
 * TypeScript and JavaScript modules.
 */
export class Compiler
  implements ts.LanguageServiceHost, ts.FormatDiagnosticsHost, CodeProvider {
  // Modules are usually referenced by their ModuleSpecifier and ContainingFile,
  // and keeping a map of the resolved module file name allows more efficient
  // future resolution
  private readonly _fileNamesMap = new Map<
    ContainingFile,
    Map<ModuleSpecifier, ModuleFileName>
  >();
  // Keep track of state of the last module requested via `getGeneratedContents`
  private _lastModule: ModuleMetaData | undefined;
  // A reference to the log utility, so it can be monkey patched during testing
  private _log = log;
  // A map of module file names to module meta data
  private readonly _moduleMetaDataMap = new Map<
    ModuleFileName,
    ModuleMetaData
  >();
  // TODO ideally this are not static and can be influenced by command line
  // arguments
  private readonly _options: ts.CompilerOptions = {
    allowJs: true,
    checkJs: true,
    module: ts.ModuleKind.AMD,
    outDir: "$deno$",
    resolveJsonModule: true,
    sourceMap: true,
    stripComments: true,
    target: ts.ScriptTarget.ESNext
  };
  // A reference to the `./os.ts` module, so it can be monkey patched during
  // testing
  private _os: Os = os;
  // Used to contain the script file we are currently compiling
  private _scriptFileNames: string[] = [];
  // A reference to the TypeScript LanguageService instance so it can be
  // monkey patched during testing
  private _service: ts.LanguageService;
  // A reference to `typescript` module so it can be monkey patched during
  // testing
  private _ts: Ts = ts;
  // Flags forcing recompilation of TS code
  public recompile = false;

  /** The TypeScript language service often refers to the resolved fileName of
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
      ? this._resolveModule(fileName, "")
      : undefined;
  }

  /** Given a `moduleSpecifier` and `containingFile` retrieve the cached
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

  /** Given a `moduleSpecifier` and `containingFile`, resolve the module and
   * return the `ModuleMetaData`.
   */
  private _resolveModule(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("compiler.resolveModule", { moduleSpecifier, containingFile });
    assert(moduleSpecifier != null && moduleSpecifier.length > 0);
    let fileName = this._resolveFileName(moduleSpecifier, containingFile);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    let moduleId: ModuleId | undefined;
    let mediaType = MediaType.Unknown;
    let sourceCode: SourceCode | undefined;
    let outputCode: OutputCode | undefined;
    let sourceMap: SourceMap | undefined;
    if (
      moduleSpecifier.startsWith(ASSETS) ||
      containingFile.startsWith(ASSETS)
    ) {
      // Assets are compiled into the runtime javascript bundle.
      // we _know_ `.pop()` will return a string, but TypeScript doesn't so
      // not null assertion
      moduleId = moduleSpecifier.split("/").pop()!;
      const assetName = moduleId.includes(".") ? moduleId : `${moduleId}.d.ts`;
      assert(assetName in assetSourceCode, `No such asset "${assetName}"`);
      mediaType = MediaType.TypeScript;
      sourceCode = assetSourceCode[assetName];
      fileName = `${ASSETS}/${assetName}`;
      outputCode = "";
      sourceMap = "";
    } else {
      // We query Rust with a CodeFetch message. It will load the sourceCode,
      // and if there is any outputCode cached, will return that as well.
      const fetchResponse = this._os.codeFetch(moduleSpecifier, containingFile);
      moduleId = fetchResponse.moduleName;
      fileName = fetchResponse.filename;
      mediaType = fetchResponse.mediaType;
      sourceCode = fetchResponse.sourceCode;
      outputCode = fetchResponse.outputCode;
      sourceMap =
        fetchResponse.sourceMap && JSON.parse(fetchResponse.sourceMap);
    }
    assert(moduleId != null, "No module ID.");
    assert(fileName != null, "No file name.");
    assert(sourceCode ? sourceCode.length > 0 : false, "No source code.");
    assert(
      mediaType !== MediaType.Unknown,
      `Unknown media type for: "${moduleSpecifier}" from "${containingFile}".`
    );
    this._log(
      "resolveModule sourceCode length:",
      sourceCode && sourceCode.length
    );
    this._log("resolveModule has outputCode:", outputCode != null);
    this._log("resolveModule has source map:", sourceMap != null);
    this._log("resolveModule has media type:", MediaType[mediaType]);
    // fileName is asserted above, but TypeScript does not track so not null
    this._setFileName(moduleSpecifier, containingFile, fileName!);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    const moduleMetaData = new ModuleMetaData(
      moduleId!,
      fileName!,
      mediaType,
      sourceCode,
      outputCode,
      sourceMap
    );
    this._moduleMetaDataMap.set(fileName!, moduleMetaData);
    return moduleMetaData;
  }

  /** Caches the resolved `fileName` in relationship to the `moduleSpecifier`
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

  private constructor() {
    if (Compiler._instance) {
      throw new TypeError("Attempt to create an additional compiler.");
    }
    this._service = this._ts.createLanguageService(this);
  }

  // Deno specific compiler API

  /** Retrieve the output of the TypeScript compiler for a given module and
   * cache the result. Re-compilation can be forced using '--recompile' flag.
   */
  compile(moduleMetaData: ModuleMetaData): OutputCode {
    const recompile = !!this.recompile;
    if (!recompile && moduleMetaData.outputCode) {
      return moduleMetaData.outputCode;
    }
    const { fileName, sourceCode, mediaType, moduleId } = moduleMetaData;
    console.warn("Compiling", moduleId);
    // Instead of using TypeScript to transpile JSON modules, we will just do
    // it directly.
    if (mediaType === MediaType.Json) {
      moduleMetaData.outputCode = jsonAmdTemplate(sourceCode, fileName);
    } else {
      const service = this._service;
      assert(
        mediaType === MediaType.TypeScript || mediaType === MediaType.JavaScript
      );
      const output = service.getEmitOutput(fileName);

      // Get the relevant diagnostics - this is 3x faster than
      // `getPreEmitDiagnostics`.
      const diagnostics = [
        // TypeScript is overly opinionated that only CommonJS modules kinds can
        // support JSON imports.  Allegedly this was fixed in
        // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
        // so we will ignore complaints about this compiler setting.
        ...service
          .getCompilerOptionsDiagnostics()
          .filter(diagnostic => diagnostic.code !== 5070),
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

      assert(
        !output.emitSkipped,
        "The emit was skipped for an unknown reason."
      );

      assert(
        output.outputFiles.length === 2,
        `Expected 2 files to be emitted, got ${output.outputFiles.length}.`
      );

      const [sourceMapFile, outputFile] = output.outputFiles;
      assert(
        sourceMapFile.name.endsWith(".map"),
        "Expected first emitted file to be a source map"
      );
      assert(
        outputFile.name.endsWith(".js"),
        "Expected second emitted file to be JavaScript"
      );
      moduleMetaData.outputCode = `${
        outputFile.text
      }\n//# sourceURL=${fileName}`;
      moduleMetaData.sourceMap = JSON.parse(sourceMapFile.text);
    }

    moduleMetaData.scriptVersion = "1";
    const sourceMap =
      moduleMetaData.sourceMap === "string"
        ? moduleMetaData.sourceMap
        : JSON.stringify(moduleMetaData.sourceMap);
    this._os.codeCache(
      fileName,
      sourceCode,
      moduleMetaData.outputCode,
      sourceMap
    );
    return moduleMetaData.outputCode;
  }

  /** Given a module specifier and a containing file, return the filename of the
   * module.  If the module is not resolvable, the method will throw.
   */
  getFilename(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName {
    const moduleMetaData = this._resolveModule(moduleSpecifier, containingFile);
    return moduleMetaData.fileName;
  }

  /** Given a fileName, return what was generated by the compiler. */
  getGeneratedContents = (fileName: string): string | RawSourceMap => {
    this._log("compiler.getGeneratedContents", fileName);
    if (fileName === "gen/bundle/main.js") {
      assert(libdeno.mainSource.length > 0);
      return libdeno.mainSource;
    } else if (fileName === "main.js.map") {
      return libdeno.mainSourceMap;
    } else if (fileName === "deno_main.js") {
      return "";
    } else if (!fileName.endsWith(".map")) {
      const moduleMetaData = this._moduleMetaDataMap.get(fileName);
      if (!moduleMetaData) {
        this._lastModule = undefined;
        return "";
      }
      this._lastModule = moduleMetaData;
      return moduleMetaData.outputCode;
    } else {
      if (this._lastModule && this._lastModule.sourceMap) {
        // Assuming the the map will always be asked for after the source
        // code.
        const { sourceMap } = this._lastModule;
        this._lastModule = undefined;
        return sourceMap;
      } else {
        // Errors thrown here are caught by source-map.
        throw new Error(`Unable to find source map: "${fileName}"`);
      }
    }
  };

  /** Get the output code for a module based on its filename. A call to
   * `.getFilename()` should occur before attempting to get the output code as
   * this ensures the module is loaded.
   */
  getOutput(filename: ModuleFileName): OutputCode {
    const moduleMetaData = this._getModuleMetaData(filename)!;
    assert(moduleMetaData != null, `Module not loaded: "${filename}"`);
    this._scriptFileNames = [moduleMetaData.fileName];
    return this.compile(moduleMetaData);
  }

  /** Get the source code for a module based on its filename.  A call to
   * `.getFilename()` should occur before attempting to get the output code as
   * this ensures the module is loaded.
   */
  getSource(filename: ModuleFileName): SourceCode {
    const moduleMetaData = this._getModuleMetaData(filename)!;
    assert(moduleMetaData != null, `Module not loaded: "${filename}"`);
    return moduleMetaData.sourceCode;
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
    const moduleMetaData = this._getModuleMetaData(fileName);
    if (moduleMetaData) {
      switch (moduleMetaData.mediaType) {
        case MediaType.TypeScript:
          return ts.ScriptKind.TS;
        case MediaType.JavaScript:
          return ts.ScriptKind.JS;
        case MediaType.Json:
          return ts.ScriptKind.JSON;
        default:
          return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
      }
    } else {
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
    const moduleSpecifier = LIB_RUNTIME;
    const moduleMetaData = this._resolveModule(moduleSpecifier, ASSETS);
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
  ): Array<ts.ResolvedModuleFull | ts.ResolvedModule> {
    this._log("resolveModuleNames()", { moduleNames, containingFile });
    return moduleNames.map(name => {
      let moduleMetaData: ModuleMetaData;
      if (name === "deno") {
        // builtin modules are part of the runtime lib
        moduleMetaData = this._resolveModule(LIB_RUNTIME, ASSETS);
      } else if (name === "typescript") {
        moduleMetaData = this._resolveModule("typescript.d.ts", ASSETS);
      } else {
        moduleMetaData = this._resolveModule(name, containingFile);
      }
      // According to the interface we shouldn't return `undefined` but if we
      // fail to return the same length of modules to those we cannot resolve
      // then TypeScript fails on an assertion that the lengths can't be
      // different, so we have to return an "empty" resolved module
      // TODO: all this does is push the problem downstream, and TypeScript
      // will complain it can't identify the type of the file and throw
      // a runtime exception, so we need to handle missing modules better
      const resolvedFileName = moduleMetaData.fileName || "";
      // This flags to the compiler to not go looking to transpile functional
      // code, anything that is in `/$asset$/` is just library code
      const isExternalLibraryImport = resolvedFileName.startsWith(ASSETS);
      return {
        resolvedFileName,
        isExternalLibraryImport,
        extension: getExtension(resolvedFileName, moduleMetaData.mediaType)
      };
    });
  }

  // Deno specific static properties and methods

  private static _instance: Compiler | undefined;

  /** Returns the instance of `DenoCompiler` or creates a new instance. */
  static instance(): Compiler {
    return Compiler._instance || (Compiler._instance = new Compiler());
  }
}
