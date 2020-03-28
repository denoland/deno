// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { bold, cyan, yellow } from "../colors.ts";
import { CompilerOptions } from "./api.ts";
import { buildBundle } from "./bundler.ts";
import { ConfigureResponse, Host } from "./host.ts";
import { MediaType, SourceFile } from "./sourcefile.ts";
import { atob, TextEncoder } from "../web/text_encoding.ts";
import * as compilerOps from "../ops/compiler.ts";
import * as util from "../util.ts";
import { assert } from "../util.ts";
import { writeFileSync } from "../write_file.ts";

export type WriteFileCallback = (
  fileName: string,
  data: string,
  sourceFiles?: readonly ts.SourceFile[]
) => void;

export interface WriteFileState {
  type: CompilerRequestType;
  bundle?: boolean;
  host?: Host;
  outFile?: string;
  rootNames: string[];
  emitMap?: Record<string, string>;
  emitBundle?: string;
  sources?: Record<string, string>;
}

// Warning! The values in this enum are duplicated in `cli/msg.rs`
// Update carefully!
export enum CompilerRequestType {
  Compile = 0,
  RuntimeCompile = 1,
  RuntimeTranspile = 2,
}

export const OUT_DIR = "$deno$";

function cache(
  moduleId: string,
  emittedFileName: string,
  contents: string,
  checkJs = false
): void {
  util.log("compiler::cache", { moduleId, emittedFileName, checkJs });
  const sf = SourceFile.get(moduleId);

  if (sf) {
    // NOTE: If it's a `.json` file we don't want to write it to disk.
    // JSON files are loaded and used by TS compiler to check types, but we don't want
    // to emit them to disk because output file is the same as input file.
    if (sf.mediaType === MediaType.Json) {
      return;
    }

    // NOTE: JavaScript files are only cached to disk if `checkJs`
    // option in on
    if (sf.mediaType === MediaType.JavaScript && !checkJs) {
      return;
    }
  }

  if (emittedFileName.endsWith(".map")) {
    // Source Map
    compilerOps.cache(".map", moduleId, contents);
  } else if (
    emittedFileName.endsWith(".js") ||
    emittedFileName.endsWith(".json")
  ) {
    // Compiled JavaScript
    compilerOps.cache(".js", moduleId, contents);
  } else {
    assert(false, `Trying to cache unhandled file type "${emittedFileName}"`);
  }
}

export function getAsset(name: string): string {
  return compilerOps.getAsset(name);
}

export function createWriteFile(state: WriteFileState): WriteFileCallback {
  const encoder = new TextEncoder();
  if (state.type === CompilerRequestType.Compile) {
    return function writeFile(
      fileName: string,
      data: string,
      sourceFiles?: readonly ts.SourceFile[]
    ): void {
      assert(
        sourceFiles != null,
        `Unexpected emit of "${fileName}" which isn't part of a program.`
      );
      assert(state.host);
      if (!state.bundle) {
        assert(sourceFiles.length === 1);
        cache(
          sourceFiles[0].fileName,
          fileName,
          data,
          state.host.getCompilationSettings().checkJs
        );
      } else {
        // if the fileName is set to an internal value, just noop, this is
        // used in the Rust unit tests.
        if (state.outFile && state.outFile.startsWith(OUT_DIR)) {
          return;
        }
        // we only support single root names for bundles
        assert(
          state.rootNames.length === 1,
          `Only one root name supported.  Got "${JSON.stringify(
            state.rootNames
          )}"`
        );
        // this enriches the string with the loader and re-exports the
        // exports of the root module
        const content = buildBundle(state.rootNames[0], data, sourceFiles);
        if (state.outFile) {
          const encodedData = encoder.encode(content);
          console.warn(`Emitting bundle to "${state.outFile}"`);
          writeFileSync(state.outFile, encodedData);
          console.warn(`${humanFileSize(encodedData.length)} emitted.`);
        } else {
          console.log(content);
        }
      }
    };
  }

  return function writeFile(
    fileName: string,
    data: string,
    sourceFiles?: readonly ts.SourceFile[]
  ): void {
    assert(sourceFiles != null);
    assert(state.host);
    assert(state.emitMap);
    if (!state.bundle) {
      assert(sourceFiles.length === 1);
      state.emitMap[fileName] = data;
      // we only want to cache the compiler output if we are resolving
      // modules externally
      if (!state.sources) {
        cache(
          sourceFiles[0].fileName,
          fileName,
          data,
          state.host.getCompilationSettings().checkJs
        );
      }
    } else {
      // we only support single root names for bundles
      assert(state.rootNames.length === 1);
      state.emitBundle = buildBundle(state.rootNames[0], data, sourceFiles);
    }
  };
}

export interface ConvertCompilerOptionsResult {
  files?: string[];
  options: ts.CompilerOptions;
}

export function convertCompilerOptions(
  str: string
): ConvertCompilerOptionsResult {
  const options: CompilerOptions = JSON.parse(str);
  const out: Record<string, unknown> = {};
  const keys = Object.keys(options) as Array<keyof CompilerOptions>;
  const files: string[] = [];
  for (const key of keys) {
    switch (key) {
      case "jsx":
        const value = options[key];
        if (value === "preserve") {
          out[key] = ts.JsxEmit.Preserve;
        } else if (value === "react") {
          out[key] = ts.JsxEmit.React;
        } else {
          out[key] = ts.JsxEmit.ReactNative;
        }
        break;
      case "module":
        switch (options[key]) {
          case "amd":
            out[key] = ts.ModuleKind.AMD;
            break;
          case "commonjs":
            out[key] = ts.ModuleKind.CommonJS;
            break;
          case "es2015":
          case "es6":
            out[key] = ts.ModuleKind.ES2015;
            break;
          case "esnext":
            out[key] = ts.ModuleKind.ESNext;
            break;
          case "none":
            out[key] = ts.ModuleKind.None;
            break;
          case "system":
            out[key] = ts.ModuleKind.System;
            break;
          case "umd":
            out[key] = ts.ModuleKind.UMD;
            break;
          default:
            throw new TypeError("Unexpected module type");
        }
        break;
      case "target":
        switch (options[key]) {
          case "es3":
            out[key] = ts.ScriptTarget.ES3;
            break;
          case "es5":
            out[key] = ts.ScriptTarget.ES5;
            break;
          case "es6":
          case "es2015":
            out[key] = ts.ScriptTarget.ES2015;
            break;
          case "es2016":
            out[key] = ts.ScriptTarget.ES2016;
            break;
          case "es2017":
            out[key] = ts.ScriptTarget.ES2017;
            break;
          case "es2018":
            out[key] = ts.ScriptTarget.ES2018;
            break;
          case "es2019":
            out[key] = ts.ScriptTarget.ES2019;
            break;
          case "es2020":
            out[key] = ts.ScriptTarget.ES2020;
            break;
          case "esnext":
            out[key] = ts.ScriptTarget.ESNext;
            break;
          default:
            throw new TypeError("Unexpected emit target.");
        }
        break;
      case "types":
        const types = options[key];
        assert(types);
        files.push(...types);
        break;
      default:
        out[key] = options[key];
    }
  }
  return {
    options: out as ts.CompilerOptions,
    files: files.length ? files : undefined,
  };
}

export const ignoredDiagnostics = [
  // TS2306: File 'file:///Users/rld/src/deno/cli/tests/subdir/amd_like.js' is
  // not a module.
  2306,
  // TS1375: 'await' expressions are only allowed at the top level of a file
  // when that file is a module, but this file has no imports or exports.
  // Consider adding an empty 'export {}' to make this file a module.
  1375,
  // TS1103: 'for-await-of' statement is only allowed within an async function
  // or async generator.
  1103,
  // TS2691: An import path cannot end with a '.ts' extension. Consider
  // importing 'bad-module' instead.
  2691,
  // TS5009: Cannot find the common subdirectory path for the input files.
  5009,
  // TS5055: Cannot write file
  // 'http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js'
  // because it would overwrite input file.
  5055,
  // TypeScript is overly opinionated that only CommonJS modules kinds can
  // support JSON imports.  Allegedly this was fixed in
  // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
  // so we will ignore complaints about this compiler setting.
  5070,
  // TS7016: Could not find a declaration file for module '...'. '...'
  // implicitly has an 'any' type.  This is due to `allowJs` being off by
  // default but importing of a JavaScript module.
  7016,
];

export function processConfigureResponse(
  configResult: ConfigureResponse,
  configPath: string
): ts.Diagnostic[] | undefined {
  const { ignoredOptions, diagnostics } = configResult;
  if (ignoredOptions) {
    console.warn(
      yellow(`Unsupported compiler options in "${configPath}"\n`) +
        cyan(`  The following options were ignored:\n`) +
        `    ${ignoredOptions.map((value): string => bold(value)).join(", ")}`
    );
  }
  return diagnostics;
}

// Constants used by `normalizeString` and `resolvePath`
export const CHAR_DOT = 46; /* . */
export const CHAR_FORWARD_SLASH = 47; /* / */

export function normalizeString(
  path: string,
  allowAboveRoot: boolean,
  separator: string,
  isPathSeparator: (code: number) => boolean
): string {
  let res = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let code: number;
  for (let i = 0, len = path.length; i <= len; ++i) {
    if (i < len) code = path.charCodeAt(i);
    else if (isPathSeparator(code!)) break;
    else code = CHAR_FORWARD_SLASH;

    if (isPathSeparator(code)) {
      if (lastSlash === i - 1 || dots === 1) {
        // NOOP
      } else if (lastSlash !== i - 1 && dots === 2) {
        if (
          res.length < 2 ||
          lastSegmentLength !== 2 ||
          res.charCodeAt(res.length - 1) !== CHAR_DOT ||
          res.charCodeAt(res.length - 2) !== CHAR_DOT
        ) {
          if (res.length > 2) {
            const lastSlashIndex = res.lastIndexOf(separator);
            if (lastSlashIndex === -1) {
              res = "";
              lastSegmentLength = 0;
            } else {
              res = res.slice(0, lastSlashIndex);
              lastSegmentLength = res.length - 1 - res.lastIndexOf(separator);
            }
            lastSlash = i;
            dots = 0;
            continue;
          } else if (res.length === 2 || res.length === 1) {
            res = "";
            lastSegmentLength = 0;
            lastSlash = i;
            dots = 0;
            continue;
          }
        }
        if (allowAboveRoot) {
          if (res.length > 0) res += `${separator}..`;
          else res = "..";
          lastSegmentLength = 2;
        }
      } else {
        if (res.length > 0) res += separator + path.slice(lastSlash + 1, i);
        else res = path.slice(lastSlash + 1, i);
        lastSegmentLength = i - lastSlash - 1;
      }
      lastSlash = i;
      dots = 0;
    } else if (code === CHAR_DOT && dots !== -1) {
      ++dots;
    } else {
      dots = -1;
    }
  }
  return res;
}

export function commonPath(paths: string[], sep = "/"): string {
  const [first = "", ...remaining] = paths;
  if (first === "" || remaining.length === 0) {
    return first.substring(0, first.lastIndexOf(sep) + 1);
  }
  const parts = first.split(sep);

  let endOfPrefix = parts.length;
  for (const path of remaining) {
    const compare = path.split(sep);
    for (let i = 0; i < endOfPrefix; i++) {
      if (compare[i] !== parts[i]) {
        endOfPrefix = i;
      }
    }

    if (endOfPrefix === 0) {
      return "";
    }
  }
  const prefix = parts.slice(0, endOfPrefix).join(sep);
  return prefix.endsWith(sep) ? prefix : `${prefix}${sep}`;
}

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

// @internal
export function base64ToUint8Array(data: string): Uint8Array {
  const binString = atob(data);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    bytes[i] = binString.charCodeAt(i);
  }
  return bytes;
}
