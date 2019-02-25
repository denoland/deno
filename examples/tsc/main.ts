// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import "../../js/globals";

import * as ts from "typescript";

import { ASSET, readAsset } from "./assets";
import { args } from "../../js/deno";
import * as dir from "../../js/dir";
import { libdeno } from "../../js/libdeno";
import { mkdirSync } from "../../js/mkdir";
import * as os from "../../js/os";
import { readDirSync } from "../../js/read_dir";
import { readFileSync } from "../../js/read_file";
import { removeSync } from "../../js/remove";
import { statSync } from "../../js/stat";
import { clearTimer, setTimeout } from "../../js/timers";
import { atob, btoa, TextDecoder, TextEncoder } from "../../js/text_encoding";
import { log } from "../../js/util";
import { writeFileSync } from "../../js/write_file";

// These are functions that are used in TypeScript directly, but are not part
// of the public APIs, therefore we are replicating them here.  They could be
// in theory used directly though.
import {
  combinePaths,
  fileSystemEntryExists,
  FileSystemEntryKind,
  findConfigFile,
  getAccessibleFileSystemEntries,
  matchFiles,
  resolve,
  resolvePath
} from "./utils";

const byteOrderMarkIndicator = "\uFEFF";

// All privilaged communication is in Uint8Array's so we will create instances
// of the decoders for our own use.
const decoder = new TextDecoder();
const encoder = new TextEncoder();

// This replicates the lib name resolution in TypeScript
function getDefaultLibFileName(options: ts.CompilerOptions): string {
  switch (options.target) {
    case ts.ScriptTarget.ESNext:
      return "lib.esnext.full.d.ts";
    case ts.ScriptTarget.ES2018:
      return "lib.es2018.full.d.ts";
    case ts.ScriptTarget.ES2017:
      return "lib.es2017.full.d.ts";
    case ts.ScriptTarget.ES2016:
      return "lib.es2016.full.d.ts";
    case ts.ScriptTarget.ES2015:
      return "lib.es6.d.ts";
    default:
      return "lib.d.ts";
  }
}

// Provides a low level interface to the host system
// TODO: improve performance by caching stats?
class DenoSystem implements ts.System {
  get args(): string[] {
    return args;
  }

  get newLine(): string {
    return "\n";
  }

  get useCaseSensitiveFileNames(): boolean {
    return true;
  }

  write(s: string): void {
    libdeno.print(s);
  }

  writeOutputIsTTY(): boolean {
    return os.isTTY().stdout;
  }

  readFile(path: string): string | undefined {
    // Normally, TypeScript reads its libraries by looking for libs relative to
    // where the script is being loaded from.  In this case, we have inlined all
    // the libs into the bundle which makes up the snapshot, so any internal
    // files have to be loaded from the in memory bundle.
    if (path.startsWith(ASSET)) {
      return readAsset(path);
    }
    return decoder.decode(readFileSync(path));
  }

  getFileSize(path: string): number {
    return statSync(path).len;
  }

  writeFile(path: string, data: string, writeByteOrderMark?: boolean): void {
    if (writeByteOrderMark) {
      data = `${byteOrderMarkIndicator}${data}`;
    }
    writeFileSync(path, encoder.encode(data), { create: true });
  }

  resolvePath(path: string): string {
    return resolve(path);
  }

  fileExists(path: string): boolean {
    try {
      const isFile = statSync(path).isFile();
      return isFile;
    } catch {
      return false;
    }
  }

  directoryExists(path: string): boolean {
    try {
      const isDirectory = statSync(path).isDirectory();
      return isDirectory;
    } catch {
      return false;
    }
  }

  createDirectory(path: string): void {
    mkdirSync(path);
  }

  getExecutingFilePath(): string {
    return os.execPath;
  }

  getCurrentDirectory(): string {
    return dir.cwd();
  }

  getDirectories(path: string): string[] {
    return readDirSync(path)
      .map(fileInfo => fileInfo.name)
      .filter(
        dir =>
          dir &&
          fileSystemEntryExists(
            combinePaths(path, dir),
            FileSystemEntryKind.Directory
          )
      ) as string[];
  }

  readDirectory(
    path: string,
    extensions?: ReadonlyArray<string>,
    excludes?: ReadonlyArray<string>,
    includes?: ReadonlyArray<string>,
    depth?: number
  ): string[] {
    return matchFiles(
      path,
      extensions,
      excludes,
      includes,
      this.useCaseSensitiveFileNames,
      this.getCurrentDirectory(),
      depth,
      getAccessibleFileSystemEntries
    );
  }

  getModifiedTime(path: string): Date | undefined {
    try {
      const info = statSync(path);
      if (info.modified) {
        return new Date(info.modified);
      } else {
        return undefined;
      }
    } catch {
      return undefined;
    }
  }

  deleteFile(path: string): void {
    removeSync(path);
  }

  exit(exitCode?: number): never {
    return os.exit(exitCode);
  }

  realpath(path: string): string {
    return path;
  }

  // tslint:disable:no-any
  setTimeout(
    callback: (...args: any[]) => void,
    ms: number,
    ...args: any[]
  ): number {
    return setTimeout(callback, ms, ...args);
  }
  // tslint:enable:no-any

  clearTimeout(timeoutId: number): void {
    clearTimer(timeoutId);
  }

  base64decode(input: string): string {
    return atob(input);
  }

  base64encode(input: string): string {
    return btoa(input);
  }
}

class DenoCompilerHost implements ts.CompilerHost, ts.FormatDiagnosticsHost {
  private _env?: { [index: string]: string };

  constructor(private _sys: ts.System) {}

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
      text = this._sys.readFile(fileName);
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
    return combinePaths(
      this.getDefaultLibLocation(),
      getDefaultLibFileName(options)
    );
  }

  getDefaultLibLocation(): string {
    log("getDefaultLibLocation");
    // All libs are included inlined into the bundle
    return ASSET;
  }

  writeFile(
    fileName: string,
    data: string,
    writeByteOrderMark: boolean,
    onError?: ((message: string) => void)
  ): void {
    log("writeFile", { fileName, data, writeByteOrderMark });
    try {
      this._sys.writeFile(fileName, data, writeByteOrderMark);
    } catch (e) {
      if (onError) {
        onError(e);
      }
    }
  }

  getCurrentDirectory(): string {
    log("getCurrentDirectory");
    return this._sys.getCurrentDirectory();
  }

  getDirectories(path: string): string[] {
    log("getDirectories", path);
    return this._sys.getDirectories(path);
  }

  getCanonicalFileName(fileName: string): string {
    log("getCanonicalFileName", fileName);
    return fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    log("useCaseSensitiveFileNames");
    return this._sys.useCaseSensitiveFileNames;
  }

  getNewLine(): string {
    log("getNewLine");
    return this._sys.newLine;
  }

  readDirectory(
    rootDir: string,
    extensions: ReadonlyArray<string>,
    excludes: ReadonlyArray<string> | undefined,
    includes: ReadonlyArray<string>,
    depth?: number
  ): string[] {
    log("readDirectory", { rootDir, extensions, excludes, includes, depth });
    return this._sys.readDirectory(
      rootDir,
      extensions,
      excludes,
      includes,
      depth
    );
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
    return this._sys.fileExists(fileName);
  }

  readFile(fileName: string): string | undefined {
    log("readFile", fileName);
    return this._sys.readFile(fileName);
  }

  trace(s: string): void {
    log("trace", s);
    this._sys.write(s + this._sys.newLine);
  }

  directoryExists(directoryName: string): boolean {
    log("directoryExists", directoryName);
    return this._sys.directoryExists(directoryName);
  }

  realpath(path: string): string {
    log("realpath", path);
    return (this._sys.realpath && this._sys.realpath(path)) || path;
  }
}

const sys = new DenoSystem();
const host = new DenoCompilerHost(sys);

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

// This mirrors a function in TypeScript which uses the system interface to
// help load and parse configuration
function parseConfigFileWithSystem(
  configFileName: string,
  optionsToExtend: ts.CompilerOptions,
  system: ts.System
) {
  // tslint:disable-next-line:no-any
  const h: ts.ParseConfigFileHost = system as any;
  h.onUnRecoverableConfigFileDiagnostic = diagnostic => {
    console.log(ts.formatDiagnosticsWithColorAndContext([diagnostic], host));
    system.exit(1);
  };
  const result = ts.getParsedCommandLineOfConfigFile(
    configFileName,
    optionsToExtend,
    h
  );
  h.onUnRecoverableConfigFileDiagnostic = undefined!;
  return result;
}

// tslint:disable-next-line:no-default-export
export default function denoMain() {
  const startResMsg = os.start("TSC");

  console.log("tsc on Deno\n");
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

  // This mirrors the process in `tsc` but drops some functionality.
  const commandLine = ts.parseCommandLine(args);

  if (commandLine.options.locale) {
    ts.validateLocaleAndSetLanguage(
      commandLine.options.locale,
      sys,
      commandLine.errors
    );
  }

  if (commandLine.errors.length) {
    console.log(
      ts.formatDiagnosticsWithColorAndContext(commandLine.errors, host)
    );
    sys.exit(1);
  }

  let configFileName: string | undefined;
  if (commandLine.options.project) {
    if (commandLine.fileNames.length) {
      console.error("files passed with project");
      sys.exit(1);
    }
    const fileOrDirectory = resolvePath(commandLine.options.project);
    if (!fileOrDirectory || sys.directoryExists(fileOrDirectory)) {
      configFileName = combinePaths(fileOrDirectory, "tsconfig.json");
      if (!sys.fileExists(configFileName)) {
        console.error("missing tsconfig.json");
        sys.exit(1);
      }
    } else {
      configFileName = fileOrDirectory;
      if (!sys.fileExists(configFileName)) {
        console.error("missing tsconfig.json");
        sys.exit(1);
      }
    }
  } else if (!commandLine.fileNames.length) {
    const searchPath = resolvePath(sys.getCurrentDirectory());
    configFileName = findConfigFile(searchPath, sys.fileExists);
  }

  if (!commandLine.fileNames.length && !configFileName) {
    sys.exit(0);
  }

  const commandLineOptions = commandLine.options;
  if (configFileName) {
    const configParseResult = parseConfigFileWithSystem(
      configFileName,
      commandLineOptions,
      sys
    )!;
    compile(configParseResult.fileNames, configParseResult.options);
  } else {
    compile(commandLine.fileNames, commandLineOptions);
  }
}
