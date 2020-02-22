// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { bold, cyan, yellow } from "./colors.ts";
import { CompilerOptions } from "./compiler_api.ts";
import { buildBundle } from "./compiler_bundler.ts";
import { ConfigureResponse, Host } from "./compiler_host.ts";
import { SourceFile } from "./compiler_sourcefile.ts";
import { sendSync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import { core } from "./core.ts";
import * as util from "./util.ts";
import { assert } from "./util.ts";
import { writeFileSync } from "./write_file.ts";

/** Type for the write fall callback that allows delegation from the compiler
 * host on writing files. */
export type WriteFileCallback = (
  fileName: string,
  data: string,
  sourceFiles?: readonly ts.SourceFile[]
) => void;

/** An object which is passed to `createWriteFile` to be used to read and set
 * state related to the emit of a program. */
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
  RuntimeTranspile = 2
}

export const OUT_DIR = "$deno$";

/** Cache the contents of a file on the trusted side. */
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
    if (sf.extension === ts.Extension.Json) {
      return;
    }

    // NOTE: JavaScript files are only cached to disk if `checkJs`
    // option in on
    if (sf.extension === ts.Extension.Js && !checkJs) {
      return;
    }
  }

  if (emittedFileName.endsWith(".map")) {
    // Source Map
    sendSync(dispatch.OP_CACHE, {
      extension: ".map",
      moduleId,
      contents
    });
  } else if (
    emittedFileName.endsWith(".js") ||
    emittedFileName.endsWith(".json")
  ) {
    // Compiled JavaScript
    sendSync(dispatch.OP_CACHE, {
      extension: ".js",
      moduleId,
      contents
    });
  } else {
    assert(false, `Trying to cache unhandled file type "${emittedFileName}"`);
  }
}

let OP_FETCH_ASSET: number;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Retrieve an asset from Rust. */
export function getAsset(name: string): string {
  // this path should only be called for assets that are lazily loaded at
  // runtime
  if (dispatch.OP_FETCH_ASSET) {
    util.log("compiler_util::getAsset", name);
    return sendSync(dispatch.OP_FETCH_ASSET, { name }).sourceCode;
  }

  // this path should only be taken during snapshotting
  if (!OP_FETCH_ASSET) {
    const ops = core.ops();
    const opFetchAsset = ops["fetch_asset"];
    assert(opFetchAsset, "OP_FETCH_ASSET is not registered");
    OP_FETCH_ASSET = opFetchAsset;
  }

  // We really don't want to depend on JSON dispatch during snapshotting, so
  // this op exchanges strings with Rust as raw byte arrays.
  const sourceCodeBytes = core.dispatch(OP_FETCH_ASSET, encoder.encode(name));
  return decoder.decode(sourceCodeBytes!);
}

/** Generates a `writeFile` function which can be passed to the compiler `Host`
 * to use when emitting files. */
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
          console.warn(`${util.humanFileSize(encodedData.length)} emitted.`);
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

/** Take a runtime set of compiler options as stringified JSON and convert it
 * to a set of TypeScript compiler options. */
export function convertCompilerOptions(str: string): ts.CompilerOptions {
  const options: CompilerOptions = JSON.parse(str);
  const out: Record<string, unknown> = {};
  const keys = Object.keys(options) as Array<keyof CompilerOptions>;
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
      default:
        out[key] = options[key];
    }
  }
  return out as ts.CompilerOptions;
}

/** An array of TypeScript diagnostic types we ignore. */
export const ignoredDiagnostics = [
  // TS1103: 'for-await-of' statement is only allowed within an async function
  // or async generator.
  1103,
  // TS1308: 'await' expression is only allowed within an async function.
  1308,
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
  5070
];

/** When doing a host configuration, processing the response and logging out
 * and options which were ignored. */
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
