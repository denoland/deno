import * as ts from "typescript";
import { log, assert, globalEval, _global } from "./util";
import * as os from "./os";
import * as path from "path";
import * as amd from "./amd";

/*
export function makeCacheDir(): string {
  let cacheDir = path.join(env.HOME, ".deno/cache")
  os.mkdirp(cacheDir);
  return cacheDir
}
*/

export function compile(cwd: string, inputFn: string): void {
  const options: ts.CompilerOptions = {
    allowJs: true,
    module: ts.ModuleKind.AMD,
    outDir: "/" // Will be placed in ~/.deno/compile
  };
  const host = new CompilerHost();

  const inputExt = path.extname(inputFn);
  if (!EXTENSIONS.includes(inputExt)) {
    console.error(`Bad file name extension for input "${inputFn}"`);
    os.exit(1);
  }

  const program = ts.createProgram([inputFn], options, host);
  //let sourceFiles = program.getSourceFiles();
  //log("rootFileNames", program.getRootFileNames());

  // Print compilation errors, if any.
  const diagnostics = getDiagnostics(program);
  if (diagnostics.length > 0) {
    const errorMessages = diagnostics.map(d => formatDiagnostic(d, cwd));
    for (const msg of errorMessages) {
      console.error(msg);
    }
    os.exit(2);
  }

  const emitResult = program.emit();
  assert(!emitResult.emitSkipped);
  log("emitResult", emitResult);

  amd.executeQueueDrain();
}

/**
 * Format a diagnostic object into a string.
 * Adapted from TS-Node https://github.com/TypeStrong/ts-node
 * which uses the same MIT license as this file but is
 * Copyright (c) 2014 Blake Embrey (hello@blakeembrey.com)
 */
export function formatDiagnostic(
  diagnostic: ts.Diagnostic,
  cwd: string,
  lineOffset = 0
): string {
  const messageText = ts.flattenDiagnosticMessageText(
    diagnostic.messageText,
    "\n"
  );
  const { code } = diagnostic;
  if (diagnostic.file) {
    const fn = path.relative(cwd, diagnostic.file.fileName);
    if (diagnostic.start) {
      const { line, character } = diagnostic.file.getLineAndCharacterOfPosition(
        diagnostic.start
      );
      const r = Number(line) + 1 + lineOffset;
      const c = Number(character) + 1;
      return `${fn} (${r},${c}): ${messageText} (${code})`;
    }
    return `${fn}: ${messageText} (${code})`;
  }
  return `${messageText} (${code})`;
}

function getDiagnostics(program: ts.Program): ReadonlyArray<ts.Diagnostic> {
  return program
    .getOptionsDiagnostics()
    .concat(
      program.getGlobalDiagnostics(),
      program.getSyntacticDiagnostics(),
      program.getSemanticDiagnostics(),
      program.getDeclarationDiagnostics()
    );
}

const EXTENSIONS = [".ts", ".js"];

export class CompilerHost {
  constructor() {}

  getSourceFile(
    fileName: string,
    languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void,
    shouldCreateNewSourceFile?: boolean
  ): ts.SourceFile | undefined {
    let sourceText: string;
    if (fileName === "lib.d.ts") {
      // TODO This should be compiled into the bindata.
      sourceText = os.readFileSync("node_modules/typescript/lib/lib.d.ts");
    } else {
      sourceText = os.readFileSync(fileName);
    }
    // fileName = fileName.replace(/\.\w+$/, ""); // Remove extension.
    if (sourceText) {
      log("getSourceFile", { fileName });
      return ts.createSourceFile(fileName, sourceText, languageVersion);
    } else {
      log("getSourceFile NOT FOUND", { fileName });
      return undefined;
    }
  }

  getSourceFileByPath?(
    fileName: string,
    path: ts.Path,
    languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void,
    shouldCreateNewSourceFile?: boolean
  ): ts.SourceFile | undefined {
    console.log("getSourceFileByPath", fileName);
    return undefined;
  }

  // getCancellationToken?(): CancellationToken;
  getDefaultLibFileName(options: ts.CompilerOptions): string {
    return ts.getDefaultLibFileName(options);
  }

  getDefaultLibLocation(): string {
    return "/blah/";
  }

  writeFile(
    fileName: string,
    data: string,
    writeByteOrderMark: boolean,
    onError: ((message: string) => void) | undefined,
    sourceFiles: ReadonlyArray<ts.SourceFile>
  ): void {
    //log("writeFile", { fileName, data });

    os.compileOutput(data, fileName);

    _global["define"] = amd.makeDefine(fileName);
    globalEval(data);
    _global["define"] = null;
  }

  getCurrentDirectory(): string {
    log("getCurrentDirectory", ".");
    return ".";
  }

  getDirectories(path: string): string[] {
    log("getDirectories", path);
    return [];
  }

  getCanonicalFileName(fileName: string): string {
    return fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    return true;
  }

  getNewLine(): string {
    return "\n";
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string,
    reusedNames?: string[]
  ): Array<ts.ResolvedModule | undefined> {
    //log("resolveModuleNames", { moduleNames, reusedNames });
    return moduleNames.map((name: string) => {
      if (
        name.startsWith("/") ||
        name.startsWith("http://") ||
        name.startsWith("https://")
      ) {
        throw Error("Non-relative imports not yet supported.");
      } else {
        // Relative import.
        const containingDir = path.dirname(containingFile);
        const resolvedFileName = path.join(containingDir, name);
        //log("relative import", { containingFile, name, resolvedFileName });
        const isExternalLibraryImport = false;
        return { resolvedFileName, isExternalLibraryImport };
      }
    });
  }

  fileExists(fileName: string): boolean {
    log("fileExists", fileName);
    return false;
  }

  readFile(fileName: string): string | undefined {
    log("readFile", fileName);
    return undefined;
  }

  /**
   * This method is a companion for 'resolveModuleNames' and is used to resolve
   * 'types' references to actual type declaration files
   */
  // resolveTypeReferenceDirectives?(typeReferenceDirectiveNames: string[],
  // containingFile: string): (ResolvedTypeReferenceDirective | undefined)[];

  // getEnvironmentVariable?(name: string): string
  // createHash?(data: string): string;
}
