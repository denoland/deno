// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import "../../js/globals";

import * as ts from "typescript";

import { args } from "../../js/deno";
import * as dir from "../../js/dir";
import * as os from "../../js/os";
import { TextDecoder, TextEncoder } from "../../js/text_encoding";
import { readFileSync } from "../../js/read_file";
import { statSync } from "../../js/stat";
import { log } from "../../js/util";
import { writeFileSync } from "../../js/write_file";

const byteOrderMarkIndicator = "\uFEFF";

const decoder = new TextDecoder();
const encoder = new TextEncoder();

class DenoCompilerHost implements ts.CompilerHost, ts.FormatDiagnosticsHost {
  private _env?: { [index: string]: string };

  getSourceFile(
    fileName: string,
    languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void,
    shouldCreateNewSourceFile?: boolean
  ): ts.SourceFile | undefined {
    log("getSourceFile", {
      fileName,
      languageVersion: ts.ScriptTarget[languageVersion],
      shouldCreateNewSourceFile
    });
    let text: string | undefined;
    try {
      const data = readFileSync(fileName);
      text = decoder.decode(data);
    } catch (e) {
      if (onError) {
        onError(e.message);
      }
      text = "";
    }
    return text !== undefined
      ? ts.createSourceFile(fileName, text, languageVersion)
      : undefined;
  }

  getDefaultLibFileName(options: ts.CompilerOptions): string {
    log("getDefaultLibFileName", options);
    return "";
  }

  getDefaultLibLocation(): string {
    log("getDefaultLibLocation");
    return "";
  }

  writeFile(
    fileName: string,
    data: string,
    writeByteOrderMark: boolean,
    onError?: ((message: string) => void)
  ): void {
    log("writeFile", { fileName, data, writeByteOrderMark });
    if (writeByteOrderMark) {
      data = `${byteOrderMarkIndicator}${data}`;
    }
    try {
      writeFileSync(fileName, encoder.encode(data), { create: true });
    } catch (e) {
      if (onError) {
        onError(e.message);
      }
    }
  }

  getCurrentDirectory(): string {
    log("getCurrentDirectory");
    return dir.cwd();
  }

  getDirectories(path: string): string[] {
    log("getDirectories", path);
    return [];
  }

  getCanonicalFileName(fileName: string): string {
    log("getCanonicalFileName", fileName);
    return fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    log("useCaseSensitiveFileNames");
    return true;
  }

  getNewLine(): string {
    log("getNewLine");
    return "\n";
  }

  readDirectory(
    rootDir: string,
    extensions: ReadonlyArray<string>,
    excludes: ReadonlyArray<string> | undefined,
    includes: ReadonlyArray<string>,
    depth?: number
  ): string[] {
    log("readDirectory", { rootDir, extensions, excludes, includes, depth });
    return [];
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string,
    reusedNames?: string[],
    redirectedReference?: ts.ResolvedProjectReference
  ): Array<ts.ResolvedModule | undefined> {
    log("resolveModuleName", { moduleNames, containingFile, reusedNames });
    const result: undefined[] = [];
    result.length = moduleNames.length;
    return result;
  }

  resolveTypeReferenceDirectives(
    typeReferenceDirectiveNames: string[],
    containingFile: string,
    redirectedReference?: ts.ResolvedProjectReference
  ): Array<ts.ResolvedTypeReferenceDirective | undefined> {
    log("resolveTypeReferenceDirectives", {
      typeReferenceDirectiveNames,
      containingFile
    });
    const result: undefined[] = [];
    result.length = typeReferenceDirectiveNames.length;
    return result;
  }

  getEnvironmentVariable(name: string): string | undefined {
    log("getEnvironmentVariable", name);
    return (this._env || (this._env = os.env()))[name];
  }

  createHash(data: string): string {
    log("createHash", data);
    return "";
  }

  fileExists(fileName: string): boolean {
    log("fileExists", fileName);
    try {
      const isFile = statSync(fileName).isFile();
      return isFile;
    } catch {
      return false;
    }
  }

  readFile(fileName: string): string | undefined {
    log("readFile", fileName);
    return;
  }

  trace(s: string): void {
    log("trace", s);
  }

  directoryExists(directoryName: string): boolean {
    log("directoryExists", directoryName);
    try {
      const isDirectory = statSync(directoryName).isDirectory();
      return isDirectory;
    } catch {
      return false;
    }
  }

  realpath(path: string): string {
    log("realpath", path);
    return path;
  }
}

const host = new DenoCompilerHost();

function compile(fileNames: string[], options: ts.CompilerOptions): void {
  log("compile", { fileNames, options });
  const program = ts.createProgram(fileNames, options, host);
  const result = program.emit();

  const diagnostics = [
    ...ts.getPreEmitDiagnostics(program),
    ...result.diagnostics
  ];

  if (diagnostics.length) {
    console.log(ts.formatDiagnosticsWithColorAndContext(diagnostics, host));
  }

  os.exit(result.emitSkipped ? 1 : 0);
}

// tslint:disable-next-line:no-default-export
export default function denoMain() {
  const startResMsg = os.start("TSC");

  console.log("tsc\n");
  console.log("deno:", startResMsg.denoVersion());
  console.log("v8:", startResMsg.v8Version());
  console.log("typescript:", ts.version);

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  for (let i = 1; i < startResMsg.argvLength(); i++) {
    args.push(startResMsg.argv(i));
  }
  log("args", args);
  Object.freeze(args);

  compile(["tests/002_hello.ts"], {});
}
