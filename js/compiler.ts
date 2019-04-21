// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as ts from "typescript";
import * as msg from "gen/cli/msg_generated";
import { window } from "./window";
import { assetSourceCode } from "./assets";
import { Console } from "./console";
import { core } from "./core";
import * as os from "./os";
import { TextDecoder, TextEncoder } from "./text_encoding";
import { clearTimer, setTimeout } from "./timers";
import { postMessage, workerClose, workerMain } from "./workers";
import { assert, log, notImplemented } from "./util";

const EOL = "\n";
const ASSETS = "$asset$";
const LIB_RUNTIME = `${ASSETS}/lib.deno_runtime.d.ts`;

// An instance of console
const console = new Console(core.print);

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

/** The format of the work message payload coming from the privileged side */
interface CompilerLookup {
  specifier: ModuleSpecifier;
  referrer: ContainingFile;
  cmdId: number;
}

/** Abstraction of the APIs required from the `os` module so they can be
 * easily mocked.
 */
interface Os {
  fetchModuleMetaData: typeof os.fetchModuleMetaData;
  exit: typeof os.exit;
}

/** Abstraction of the APIs required from the `typescript` module so they can
 * be easily mocked.
 */
interface Ts {
  createLanguageService: typeof ts.createLanguageService;
  formatDiagnosticsWithColorAndContext: typeof ts.formatDiagnosticsWithColorAndContext;
  formatDiagnostics: typeof ts.formatDiagnostics;
}

/** A simple object structure for caching resolved modules and their contents.
 *
 * Named `ModuleMetaData` to clarify it is just a representation of meta data of
 * the module, not the actual module instance.
 */
class ModuleMetaData implements ts.IScriptSnapshot {
  public scriptVersion = "";

  constructor(
    public readonly moduleId: ModuleId,
    public readonly fileName: ModuleFileName,
    public readonly mediaType: msg.MediaType,
    public readonly sourceCode: SourceCode = "",
    public outputCode: OutputCode = "",
    public sourceMap: SourceMap = ""
  ) {
    if (outputCode !== "" || fileName.endsWith(".d.ts")) {
      this.scriptVersion = "1";
    }
  }

  /** TypeScript IScriptSnapshot Interface */

  public getText(start: number, end: number): string {
    return start === 0 && end === this.sourceCode.length
      ? this.sourceCode
      : this.sourceCode.substring(start, end);
  }

  public getLength(): number {
    return this.sourceCode.length;
  }

  public getChangeRange(): undefined {
    // Required `IScriptSnapshot` API, but not implemented/needed in deno
    return undefined;
  }
}

/** Returns the TypeScript Extension enum for a given media type. */
function getExtension(
  fileName: ModuleFileName,
  mediaType: msg.MediaType
): ts.Extension {
  switch (mediaType) {
    case msg.MediaType.JavaScript:
      return ts.Extension.Js;
    case msg.MediaType.TypeScript:
      return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
    case msg.MediaType.Json:
      return ts.Extension.Json;
    case msg.MediaType.Unknown:
    default:
      throw TypeError("Cannot resolve extension.");
  }
}

/** Generate output code for a provided JSON string along with its source. */
function jsonEsmTemplate(
  jsonString: string,
  sourceFileName: string
): OutputCode {
  return (
    `const _json = JSON.parse(\`${jsonString}\`);\n` +
    `export default _json;\n` +
    `//# sourceURL=${sourceFileName}\n`
  );
}

/** A singleton class that combines the TypeScript Language Service host API
 * with Deno specific APIs to provide an interface for compiling and running
 * TypeScript and JavaScript modules.
 */
class Compiler implements ts.LanguageServiceHost, ts.FormatDiagnosticsHost {
  // Modules are usually referenced by their ModuleSpecifier and ContainingFile,
  // and keeping a map of the resolved module file name allows more efficient
  // future resolution
  private readonly _fileNamesMap = new Map<
    ContainingFile,
    Map<ModuleSpecifier, ModuleFileName>
  >();
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
    allowNonTsExtensions: true,
    checkJs: true,
    esModuleInterop: true,
    module: ts.ModuleKind.ESNext,
    outDir: "$deno$",
    resolveJsonModule: true,
    sourceMap: true,
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

  private readonly _assetsSourceCode: { [key: string]: string };

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
    return (
      this._moduleMetaDataMap.get(fileName) ||
      (fileName.startsWith(ASSETS)
        ? this._resolveModule(fileName, "")
        : undefined)
    );
  }

  /** Given a `moduleSpecifier` and `containingFile` retrieve the cached
   * `fileName` for a given module.  If the module has yet to be resolved
   * this will return `undefined`.
   */
  private _resolveFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    this._log("compiler._resolveFileName", { moduleSpecifier, containingFile });
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
    this._log("compiler._resolveModule", { moduleSpecifier, containingFile });
    assert(moduleSpecifier != null && moduleSpecifier.length > 0);
    let fileName = this._resolveFileName(moduleSpecifier, containingFile);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    let moduleId: ModuleId | undefined;
    let mediaType = msg.MediaType.Unknown;
    let sourceCode: SourceCode | undefined;
    if (
      moduleSpecifier.startsWith(ASSETS) ||
      containingFile.startsWith(ASSETS)
    ) {
      // Assets are compiled into the runtime javascript bundle.
      // we _know_ `.pop()` will return a string, but TypeScript doesn't so
      // not null assertion
      moduleId = moduleSpecifier.split("/").pop()!;
      const assetName = moduleId.includes(".") ? moduleId : `${moduleId}.d.ts`;
      assert(
        assetName in this._assetsSourceCode,
        `No such asset "${assetName}"`
      );
      mediaType = msg.MediaType.TypeScript;
      sourceCode = this._assetsSourceCode[assetName];
      fileName = `${ASSETS}/${assetName}`;
    } else {
      // We query Rust with a CodeFetch message. It will load the sourceCode,
      // and if there is any outputCode cached, will return that as well.
      const fetchResponse = this._os.fetchModuleMetaData(
        moduleSpecifier,
        containingFile
      );
      moduleId = fetchResponse.moduleName;
      fileName = fetchResponse.filename;
      mediaType = fetchResponse.mediaType;
      sourceCode = fetchResponse.sourceCode;
    }
    assert(moduleId != null, "No module ID.");
    assert(fileName != null, "No file name.");
    assert(
      mediaType !== msg.MediaType.Unknown,
      `Unknown media type for: "${moduleSpecifier}" from "${containingFile}".`
    );
    this._log(
      "resolveModule sourceCode length:",
      sourceCode && sourceCode.length
    );
    this._log("resolveModule has media type:", msg.MediaType[mediaType]);
    // fileName is asserted above, but TypeScript does not track so not null
    this._setFileName(moduleSpecifier, containingFile, fileName!);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    const moduleMetaData = new ModuleMetaData(
      moduleId!,
      fileName!,
      mediaType,
      sourceCode
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
    this._log("compiler._setFileName", { moduleSpecifier, containingFile });
    let innerMap = this._fileNamesMap.get(containingFile);
    if (!innerMap) {
      innerMap = new Map();
      this._fileNamesMap.set(containingFile, innerMap);
    }
    innerMap.set(moduleSpecifier, fileName);
  }

  constructor(assetsSourceCode: { [key: string]: string }) {
    this._assetsSourceCode = assetsSourceCode;
    this._service = this._ts.createLanguageService(this);
  }

  // Deno specific compiler API

  /** Retrieve the output of the TypeScript compiler for a given module.
   */
  compile(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): { outputCode: OutputCode; sourceMap: SourceMap } {
    this._log("compiler.compile", { moduleSpecifier, containingFile });
    const moduleMetaData = this._resolveModule(moduleSpecifier, containingFile);
    const { fileName, mediaType, moduleId, sourceCode } = moduleMetaData;
    this._scriptFileNames = [fileName];
    console.warn("Compiling", moduleId);
    let outputCode: string;
    let sourceMap = "";
    // Instead of using TypeScript to transpile JSON modules, we will just do
    // it directly.
    if (mediaType === msg.MediaType.Json) {
      outputCode = moduleMetaData.outputCode = jsonEsmTemplate(
        sourceCode,
        fileName
      );
    } else {
      const service = this._service;
      assert(
        mediaType === msg.MediaType.TypeScript ||
          mediaType === msg.MediaType.JavaScript
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
          .filter((diagnostic): boolean => diagnostic.code !== 5070),
        ...service.getSyntacticDiagnostics(fileName),
        ...service.getSemanticDiagnostics(fileName)
      ];
      if (diagnostics.length > 0) {
        const errMsg = os.noColor
          ? this._ts.formatDiagnostics(diagnostics, this)
          : this._ts.formatDiagnosticsWithColorAndContext(diagnostics, this);

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
      outputCode = moduleMetaData.outputCode = `${
        outputFile.text
      }\n//# sourceURL=${fileName}`;
      sourceMap = moduleMetaData.sourceMap = sourceMapFile.text;
    }

    moduleMetaData.scriptVersion = "1";
    return { outputCode, sourceMap };
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
    // which would be what is set during the `.compile()`
    return this._scriptFileNames;
  }

  getScriptKind(fileName: ModuleFileName): ts.ScriptKind {
    this._log("getScriptKind()", fileName);
    const moduleMetaData = this._getModuleMetaData(fileName);
    if (moduleMetaData) {
      switch (moduleMetaData.mediaType) {
        case msg.MediaType.TypeScript:
          return ts.ScriptKind.TS;
        case msg.MediaType.JavaScript:
          return ts.ScriptKind.JS;
        case msg.MediaType.Json:
          return ts.ScriptKind.JSON;
        default:
          return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
      }
    } else {
      return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
    }
  }

  getScriptVersion(fileName: ModuleFileName): string {
    const moduleMetaData = this._getModuleMetaData(fileName);
    const version = (moduleMetaData && moduleMetaData.scriptVersion) || "";
    this._log("getScriptVersion()", fileName, version);
    return version;
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
    const moduleMetaData = this._getModuleMetaData(moduleSpecifier);
    assert(moduleMetaData != null);
    return moduleMetaData!.fileName;
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
    const resolvedModuleNames: ts.ResolvedModuleFull[] = [];
    for (const moduleName of moduleNames) {
      const moduleMetaData = this._resolveModule(moduleName, containingFile);
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
      resolvedModuleNames.push({
        resolvedFileName,
        isExternalLibraryImport,
        extension: getExtension(resolvedFileName, moduleMetaData.mediaType)
      });
    }
    return resolvedModuleNames;
  }
}

const compiler = new Compiler(assetSourceCode);

// set global objects for compiler web worker
window.clearTimeout = clearTimer;
window.console = console;
window.postMessage = postMessage;
window.setTimeout = setTimeout;
window.workerMain = workerMain;
window.close = workerClose;
window.TextDecoder = TextDecoder;
window.TextEncoder = TextEncoder;

// provide the "main" function that will be called by the privileged side when
// lazy instantiating the compiler web worker
window.compilerMain = function compilerMain(): void {
  // workerMain should have already been called since a compiler is a worker.
  window.onmessage = ({ data }: { data: CompilerLookup }): void => {
    const { specifier, referrer, cmdId } = data;

    try {
      const result = compiler.compile(specifier, referrer);
      postMessage({
        success: true,
        cmdId,
        data: result
      });
    } catch (e) {
      postMessage({
        success: false,
        cmdId,
        data: JSON.parse(core.errorToJSON(e))
      });
    }
  };
};

export default function denoMain(): void {
  os.start("TS");
}
