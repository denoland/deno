// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Glossary
// outputCode = generated javascript code
// sourceCode = typescript code (or input javascript code)
// moduleName = a resolved module name
// fileName = an unresolved raw fileName.
//            for http modules , its the path to the locally downloaded
//            version.

import * as ts from "typescript";
import * as util from "./util";
import { log } from "./util";
import { assetSourceCode } from "./assets";
import * as os from "./os";
import * as sourceMaps from "./v8_source_maps";
import { libdeno, window, globalEval } from "./globals";
import * as deno from "./deno";
import { RawSourceMap } from "./types";

const EOL = "\n";
const ASSETS = "/$asset$/";

// tslint:disable-next-line:no-any
export type AmdFactory = (...args: any[]) => undefined | object;
export type AmdDefine = (deps: string[], factory: AmdFactory) => void;

// Uncaught exceptions are sent to window.onerror by the privlaged binding.
window.onerror = (
  message: string,
  source: string,
  lineno: number,
  colno: number,
  error: Error
) => {
  // TODO Currently there is a bug in v8_source_maps.ts that causes a segfault
  // if it is used within window.onerror. To workaround we uninstall the
  // Error.prepareStackTrace handler. Users will get unmapped stack traces on
  // uncaught exceptions until this issue is fixed.
  //Error.prepareStackTrace = null;
  console.log(error.stack);
  os.exit(1);
};

export function setup(): void {
  sourceMaps.install({
    installPrepareStackTrace: true,
    getGeneratedContents: (filename: string): string | RawSourceMap => {
      util.log("getGeneratedContents", filename);
      if (filename === "gen/bundle/main.js") {
        util.assert(libdeno.mainSource.length > 0);
        return libdeno.mainSource;
      } else if (filename === "main.js.map") {
        return libdeno.mainSourceMap;
      } else if (filename === "deno_main.js") {
        return "";
      } else {
        const mod = FileModule.load(filename);
        if (!mod) {
          util.log("getGeneratedContents cannot find", filename);
          return "";
        }
        return mod.outputCode;
      }
    }
  });
}

// This class represents a module. We call it FileModule to make it explicit
// that each module represents a single file.
// Access to FileModule instances should only be done thru the static method
// FileModule.load(). FileModules are NOT executed upon first load, only when
// compileAndRun is called.
export class FileModule {
  scriptVersion = "";
  readonly exports = {};

  private static readonly map = new Map<string, FileModule>();
  constructor(
    readonly fileName: string,
    readonly sourceCode = "",
    public outputCode = ""
  ) {
    util.assert(
      !FileModule.map.has(fileName),
      `FileModule.map already has ${fileName}`
    );
    FileModule.map.set(fileName, this);
    if (outputCode !== "") {
      this.scriptVersion = "1";
    }
  }

  compileAndRun(): void {
    util.log("compileAndRun", this.sourceCode);
    if (!this.outputCode) {
      // If there is no cached outputCode, then compile the code.
      util.assert(
        this.sourceCode != null && this.sourceCode.length > 0,
        `Have no source code from ${this.fileName}`
      );
      const compiler = Compiler.instance();
      this.outputCode = compiler.compile(this.fileName);
      os.codeCache(this.fileName, this.sourceCode, this.outputCode);
    }
    execute(this.fileName, this.outputCode);
  }

  static load(fileName: string): FileModule | undefined {
    return this.map.get(fileName);
  }

  static getScriptsWithSourceCode(): string[] {
    const out = [];
    for (const fn of this.map.keys()) {
      const m = this.map.get(fn);
      if (m && m.sourceCode) {
        out.push(fn);
      }
    }
    return out;
  }
}

export function makeDefine(fileName: string): AmdDefine {
  const localDefine = (deps: string[], factory: AmdFactory): void => {
    const localRequire = (x: string) => {
      log("localRequire", x);
    };
    const currentModule = FileModule.load(fileName);
    util.assert(currentModule != null);
    const localExports = currentModule!.exports;
    log("localDefine", fileName, deps, localExports);
    const args = deps.map(dep => {
      if (dep === "require") {
        return localRequire;
      } else if (dep === "exports") {
        return localExports;
      } else if (dep === "typescript") {
        return ts;
      } else if (dep === "deno") {
        return deno;
      } else {
        const resolved = resolveModuleName(dep, fileName);
        util.assert(resolved != null);
        const depModule = FileModule.load(resolved!);
        if (depModule) {
          depModule.compileAndRun();
          return depModule.exports;
        }
        return undefined;
      }
    });
    factory(...args);
  };
  return localDefine;
}

export function resolveModule(
  moduleSpecifier: string,
  containingFile: string
): null | FileModule {
  util.log("resolveModule", { moduleSpecifier, containingFile });
  util.assert(moduleSpecifier != null && moduleSpecifier.length > 0);
  let filename: string | null;
  let sourceCode: string | null;
  let outputCode: string | null;
  if (moduleSpecifier.startsWith(ASSETS) || containingFile.startsWith(ASSETS)) {
    // Assets are compiled into the runtime javascript bundle.
    // we _know_ `.pop()` will return a string, but TypeScript doesn't so
    // not null assertion
    const moduleId = moduleSpecifier.split("/").pop()!;
    const assetName = moduleId.includes(".") ? moduleId : `${moduleId}.d.ts`;
    util.assert(assetName in assetSourceCode, `No such asset "${assetName}"`);
    sourceCode = assetSourceCode[assetName];
    filename = ASSETS + assetName;
  } else {
    // We query Rust with a CodeFetch message. It will load the sourceCode, and
    // if there is any outputCode cached, will return that as well.
    const fetchResponse = os.codeFetch(moduleSpecifier, containingFile);
    filename = fetchResponse.filename;
    sourceCode = fetchResponse.sourceCode;
    outputCode = fetchResponse.outputCode;
  }
  if (sourceCode == null || sourceCode.length === 0 || filename == null) {
    return null;
  }
  util.log("resolveModule sourceCode length ", sourceCode.length);
  const m = FileModule.load(filename);
  if (m != null) {
    return m;
  } else {
    // null and undefined are incompatible in strict mode, but outputCode being
    // null here has no runtime behavior impact, therefore not null assertion
    return new FileModule(filename, sourceCode, outputCode!);
  }
}

function resolveModuleName(
  moduleSpecifier: string,
  containingFile: string
): string | undefined {
  const mod = resolveModule(moduleSpecifier, containingFile);
  if (mod) {
    return mod.fileName;
  } else {
    return undefined;
  }
}

function execute(fileName: string, outputCode: string): void {
  util.assert(outputCode != null && outputCode.length > 0);
  window["define"] = makeDefine(fileName);
  outputCode += `\n//# sourceURL=${fileName}`;
  globalEval(outputCode);
  window["define"] = null;
}

// This is a singleton class. Use Compiler.instance() to access.
class Compiler {
  options: ts.CompilerOptions = {
    allowJs: true,
    module: ts.ModuleKind.AMD,
    outDir: "$deno$",
    inlineSourceMap: true,
    inlineSources: true,
    target: ts.ScriptTarget.ESNext
  };
  /*
  allowJs: true,
  module: ts.ModuleKind.AMD,
  noEmit: false,
  outDir: '$deno$',
  */
  private service: ts.LanguageService;

  private constructor() {
    const host = new TypeScriptHost(this.options);
    this.service = ts.createLanguageService(host);
  }

  private static _instance: Compiler;
  static instance(): Compiler {
    return this._instance || (this._instance = new this());
  }

  compile(fileName: string): string {
    const output = this.service.getEmitOutput(fileName);

    // Get the relevant diagnostics - this is 3x faster than
    // `getPreEmitDiagnostics`.
    const diagnostics = this.service
      .getCompilerOptionsDiagnostics()
      .concat(this.service.getSyntacticDiagnostics(fileName))
      .concat(this.service.getSemanticDiagnostics(fileName));
    if (diagnostics.length > 0) {
      const errMsg = ts.formatDiagnosticsWithColorAndContext(
        diagnostics,
        formatDiagnosticsHost
      );
      console.log(errMsg);
      os.exit(1);
    }

    util.assert(!output.emitSkipped);

    const outputCode = output.outputFiles[0].text;
    // let sourceMapCode = output.outputFiles[0].text;
    return outputCode;
  }
}

// Create the compiler host for type checking.
class TypeScriptHost implements ts.LanguageServiceHost {
  constructor(readonly options: ts.CompilerOptions) {}

  getScriptFileNames(): string[] {
    const keys = FileModule.getScriptsWithSourceCode();
    util.log("getScriptFileNames", keys);
    return keys;
  }

  getScriptVersion(fileName: string): string {
    util.log("getScriptVersion", fileName);
    const m = FileModule.load(fileName);
    return (m && m.scriptVersion) || "";
  }

  getScriptSnapshot(fileName: string): ts.IScriptSnapshot | undefined {
    util.log("getScriptSnapshot", fileName);
    const m = resolveModule(fileName, ".");
    if (m == null) {
      util.log("getScriptSnapshot", fileName, "NOT FOUND");
      return undefined;
    }
    //const m = resolveModule(fileName, ".");
    util.assert(m.sourceCode.length > 0);
    return ts.ScriptSnapshot.fromString(m.sourceCode);
  }

  fileExists(fileName: string): boolean {
    const m = resolveModule(fileName, ".");
    const exists = m != null;
    util.log("fileExist", fileName, exists);
    return exists;
  }

  readFile(path: string, encoding?: string): string | undefined {
    util.log("readFile", path);
    return util.notImplemented();
  }

  getNewLine() {
    return EOL;
  }

  getCurrentDirectory() {
    util.log("getCurrentDirectory");
    return ".";
  }

  getCompilationSettings() {
    util.log("getCompilationSettings");
    return this.options;
  }

  getDefaultLibFileName(options: ts.CompilerOptions): string {
    const fn = "lib.globals.d.ts"; // ts.getDefaultLibFileName(options);
    util.log("getDefaultLibFileName", fn);
    const m = resolveModule(fn, ASSETS);
    util.assert(m != null);
    // TypeScript cannot track assertions, therefore not null assertion
    return m!.fileName;
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string
  ): ts.ResolvedModule[] {
    //util.log("resolveModuleNames", { moduleNames, reusedNames });
    return moduleNames.map(name => {
      let resolvedFileName;
      if (name === "deno") {
        resolvedFileName = resolveModuleName("deno.d.ts", ASSETS);
      } else if (name === "typescript") {
        resolvedFileName = resolveModuleName("typescript.d.ts", ASSETS);
      } else {
        resolvedFileName = resolveModuleName(name, containingFile);
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
}

const formatDiagnosticsHost: ts.FormatDiagnosticsHost = {
  getCurrentDirectory(): string {
    return ".";
  },
  getCanonicalFileName(fileName: string): string {
    return fileName;
  },
  getNewLine(): string {
    return EOL;
  }
};
