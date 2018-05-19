// Glossary
// outputCode = generated javascript code
// sourceCode = typescript code (or input javascript code)
// fileName = an unresolved raw fileName.
// moduleName = a resolved module name

import * as ts from "typescript";
import * as path from "path";
import * as util from "./util";
import { log } from "./util";
import * as os from "./os";
import "./url";

const EOL = "\n";

// This class represents a module. We call it FileModule to make it explicit
// that each module represents a single file.
// Access to FileModule instances should only be done thru the static method
// FileModule.load(). FileModules are executed upon first load.
export class FileModule {
  scriptVersion: string = undefined;
  sourceCode: string;
  outputCode: string;
  readonly exports = {};

  private static readonly map = new Map<string, FileModule>();
  private constructor(readonly fileName: string) {
    FileModule.map.set(fileName, this);

    // Load typescript code (sourceCode) and maybe load compiled javascript
    // (outputCode) from cache. If cache is empty, outputCode will be null.
    const { sourceCode, outputCode } = os.sourceCodeFetch(this.fileName);
    this.sourceCode = sourceCode;
    this.outputCode = outputCode;
    this.scriptVersion = "1";
  }

  compileAndRun() {
    if (!this.outputCode) {
      // If there is no cached outputCode, the compile the code.
      util.assert(this.sourceCode && this.sourceCode.length > 0);
      const compiler = Compiler.instance();
      this.outputCode = compiler.compile(this.fileName);
      os.sourceCodeCache(this.fileName, this.sourceCode, this.outputCode);
    }
    util.log("compileAndRun", this.sourceCode);
    execute(this.fileName, this.outputCode);
  }

  static load(fileName: string): FileModule {
    let m = this.map.get(fileName);
    if (m == null) {
      m = new this(fileName);
      util.assert(this.map.has(fileName));
    }
    return m;
  }

  static getScriptsWithSourceCode(): string[] {
    const out = [];
    for (const fn of this.map.keys()) {
      const m = this.map.get(fn);
      if (m.sourceCode) {
        out.push(fn);
      }
    }
    return out;
  }
}

// tslint:disable-next-line:no-any
type AmdFactory = (...args: any[]) => undefined | object;
type AmdDefine = (deps: string[], factory: AmdFactory) => void;

export function makeDefine(fileName: string): AmdDefine {
  const localDefine = (deps: string[], factory: AmdFactory): void => {
    const localRequire = (x: string) => {
      log("localRequire", x);
    };
    const currentModule = FileModule.load(fileName);
    const localExports = currentModule.exports;
    log("localDefine", fileName, deps, localExports);
    const args = deps.map(dep => {
      if (dep === "require") {
        return localRequire;
      } else if (dep === "exports") {
        return localExports;
      } else {
        const resolved = resolveModuleName(dep, fileName);
        const depModule = FileModule.load(resolved);
        depModule.compileAndRun();
        return depModule.exports;
      }
    });
    factory(...args);
  };
  return localDefine;
}

function resolveModuleName(moduleName: string, containingFile: string): string {
  if (isUrl(moduleName)) {
    // Remove the "http://" from the start of the string.
    const u = new URL(moduleName);
    const withoutProtocol = u.toString().replace(u.protocol + "//", "");
    const name2 = "/$remote$/" + withoutProtocol;
    return name2;
  } else if (moduleName.startsWith("/")) {
    throw Error("Absolute paths not supported");
  } else {
    // Relative import.
    const containingDir = path.dirname(containingFile);
    const resolvedFileName = path.join(containingDir, moduleName);
    util.log("relative import", {
      containingFile,
      moduleName,
      resolvedFileName
    });
    return resolvedFileName;
  }
}

function execute(fileName: string, outputCode: string): void {
  util.assert(outputCode && outputCode.length > 0);
  util._global["define"] = makeDefine(fileName);
  util.globalEval(outputCode);
  util._global["define"] = null;
}

// This is a singleton class. Use Compiler.instance() to access.
class Compiler {
  options: ts.CompilerOptions = {
    allowJs: true,
    module: ts.ModuleKind.AMD,
    outDir: "$deno$"
  };
  /*
  allowJs: true,
  inlineSourceMap: true,
  inlineSources: true,
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

    util.log("compile output", output);
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
    return m.scriptVersion;
  }

  getScriptSnapshot(fileName: string): ts.IScriptSnapshot | undefined {
    util.log("getScriptSnapshot", fileName);
    const m = FileModule.load(fileName);
    if (m.sourceCode) {
      return ts.ScriptSnapshot.fromString(m.sourceCode);
    } else {
      return undefined;
    }
  }

  fileExists(fileName: string): boolean {
    throw Error("not implemented");
  }

  readFile(path: string, encoding?: string): string | undefined {
    util.log("readFile", path);
    throw Error("not implemented");
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
    util.log("getDefaultLibFileName");
    return ts.getDefaultLibFileName(options);
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string,
    reusedNames?: string[]
  ): Array<ts.ResolvedModule | undefined> {
    util.log("resolveModuleNames", { moduleNames, reusedNames });
    return moduleNames.map((name: string) => {
      const resolvedFileName = resolveModuleName(name, containingFile);
      const isExternalLibraryImport = false;
      return { resolvedFileName, isExternalLibraryImport };
    });
  }
}

function isUrl(p: string): boolean {
  return (
    p.startsWith("//") || p.startsWith("http://") || p.startsWith("https://")
  );
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
