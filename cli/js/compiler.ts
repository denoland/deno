// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// TODO(ry) Combine this implementation with //deno_typescript/compiler_main.js

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to compile TS/WASM to JS.
//
// It provides a two functions that should be called by Rust:
//  - `bootstrapTsCompilerRuntime`
//  - `bootstrapWasmCompilerRuntime`
// Either of these functions must be called when creating isolate
// to properly setup runtime.

// NOTE: this import has side effects!
import "./compiler/ts_global.d.ts";

import { TranspileOnlyResult } from "./compiler/api.ts";
import { TS_SNAPSHOT_PROGRAM } from "./compiler/bootstrap.ts";
import { setRootExports } from "./compiler/bundler.ts";
import {
  CompilerHostTarget,
  defaultBundlerOptions,
  defaultRuntimeCompileOptions,
  defaultTranspileOptions,
  Host,
} from "./compiler/host.ts";
import {
  processImports,
  processLocalImports,
  resolveModules,
} from "./compiler/imports.ts";
import {
  EmmitedSource,
  WriteFileCallback,
  createCompileWriteFile,
  createBundleWriteFile,
  CompilerRequestType,
  convertCompilerOptions,
  ignoredDiagnostics,
  WriteFileState,
  processConfigureResponse,
  base64ToUint8Array,
} from "./compiler/util.ts";
import { Diagnostic, DiagnosticItem } from "./diagnostics.ts";
import { fromTypeScriptDiagnostic } from "./diagnostics_util.ts";
import { assert } from "./util.ts";
import * as util from "./util.ts";
import { bootstrapWorkerRuntime } from "./runtime_worker.ts";

interface CompilerRequestCompile {
  type: CompilerRequestType.Compile;
  target: CompilerHostTarget;
  rootNames: string[];
  // TODO(ry) add compiler config to this interface.
  // options: ts.CompilerOptions;
  configPath?: string;
  config?: string;
  unstable: boolean;
  bundle: boolean;
  cwd: string;
}

interface CompilerRequestRuntimeCompile {
  type: CompilerRequestType.RuntimeCompile;
  target: CompilerHostTarget;
  rootName: string;
  sources?: Record<string, string>;
  unstable?: boolean;
  bundle?: boolean;
  options?: string;
}

interface CompilerRequestRuntimeTranspile {
  type: CompilerRequestType.RuntimeTranspile;
  sources: Record<string, string>;
  options?: string;
}

type CompilerRequest =
  | CompilerRequestCompile
  | CompilerRequestRuntimeCompile
  | CompilerRequestRuntimeTranspile;

interface CompileResult {
  emitMap?: Record<string, EmmitedSource>;
  bundleOutput?: string;
  diagnostics: Diagnostic;
}

interface RuntimeCompileResult {
  emitMap: Record<string, EmmitedSource>;
  diagnostics: DiagnosticItem[];
}

interface RuntimeBundleResult {
  output: string;
  diagnostics: DiagnosticItem[];
}

async function compile(
  request: CompilerRequestCompile
): Promise<CompileResult> {
  const {
    bundle,
    config,
    configPath,
    rootNames,
    target,
    unstable,
    cwd,
  } = request;
  util.log(">>> compile start", {
    rootNames,
    type: CompilerRequestType[request.type],
  });

  // When a programme is emitted, TypeScript will call `writeFile` with
  // each file that needs to be emitted.  The Deno compiler host delegates
  // this, to make it easier to perform the right actions, which vary
  // based a lot on the request.
  const state: WriteFileState = {
    type: request.type,
    emitMap: {},
    bundle,
    host: undefined,
    rootNames,
  };
  let writeFile: WriteFileCallback;
  if (bundle) {
    writeFile = createBundleWriteFile(state);
  } else {
    writeFile = createCompileWriteFile(state);
  }
  const host = (state.host = new Host({
    bundle,
    target,
    writeFile,
    unstable,
  }));
  let diagnostics: readonly ts.Diagnostic[] = [];

  // if there is a configuration supplied, we need to parse that
  if (config && config.length && configPath) {
    const configResult = host.configure(cwd, configPath, config);
    diagnostics = processConfigureResponse(configResult, configPath) || [];
  }

  // This will recursively analyse all the code for other imports,
  // requesting those from the privileged side, populating the in memory
  // cache which will be used by the host, before resolving.
  const resolvedRootModules = await processImports(
    rootNames.map((rootName) => [rootName, rootName]),
    undefined,
    bundle || host.getCompilationSettings().checkJs
  );

  // if there was a configuration and no diagnostics with it, we will continue
  // to generate the program and possibly emit it.
  if (diagnostics.length === 0) {
    const options = host.getCompilationSettings();
    const program = ts.createProgram({
      rootNames,
      options,
      host,
      oldProgram: TS_SNAPSHOT_PROGRAM,
    });

    diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !ignoredDiagnostics.includes(code));

    // We will only proceed with the emit if there are no diagnostics.
    if (diagnostics && diagnostics.length === 0) {
      if (bundle) {
        // we only support a single root module when bundling
        assert(resolvedRootModules.length === 1);
        setRootExports(program, resolvedRootModules[0]);
      }
      const emitResult = program.emit();
      assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");
      // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
      // without casting.
      diagnostics = emitResult.diagnostics;
    }
  }

  let bundleOutput = undefined;

  if (bundle) {
    assert(state.bundleOutput);
    bundleOutput = state.bundleOutput;
  }

  assert(state.emitMap);
  const result: CompileResult = {
    emitMap: state.emitMap,
    bundleOutput,
    diagnostics: fromTypeScriptDiagnostic(diagnostics),
  };

  util.log("<<< compile end", {
    rootNames,
    type: CompilerRequestType[request.type],
  });

  return result;
}

async function runtimeCompile(
  request: CompilerRequestRuntimeCompile
): Promise<RuntimeCompileResult | RuntimeBundleResult> {
  const { bundle, options, rootName, sources, target, unstable } = request;

  util.log(">>> runtime compile start", {
    rootName,
    bundle,
    sources: sources ? Object.keys(sources) : undefined,
  });

  // resolve the root name, if there are sources, the root name does not
  // get resolved
  const resolvedRootName = sources ? rootName : resolveModules([rootName])[0];

  // if there are options, convert them into TypeScript compiler options,
  // and resolve any external file references
  let convertedOptions: ts.CompilerOptions | undefined;
  let additionalFiles: string[] | undefined;
  if (options) {
    const result = convertCompilerOptions(options);
    convertedOptions = result.options;
    additionalFiles = result.files;
  }

  const checkJsImports =
    bundle || (convertedOptions && convertedOptions.checkJs);

  // recursively process imports, loading each file into memory.  If there
  // are sources, these files are pulled out of the there, otherwise the
  // files are retrieved from the privileged side
  const rootNames = sources
    ? processLocalImports(
        sources,
        [[resolvedRootName, resolvedRootName]],
        undefined,
        checkJsImports
      )
    : await processImports(
        [[resolvedRootName, resolvedRootName]],
        undefined,
        checkJsImports
      );

  if (additionalFiles) {
    // any files supplied in the configuration are resolved externally,
    // even if sources are provided
    const resolvedNames = resolveModules(additionalFiles);
    rootNames.push(
      ...(await processImports(
        resolvedNames.map((rn) => [rn, rn]),
        undefined,
        checkJsImports
      ))
    );
  }

  const state: WriteFileState = {
    type: request.type,
    bundle,
    host: undefined,
    rootNames,
    sources,
    emitMap: {},
    bundleOutput: undefined,
  };
  let writeFile: WriteFileCallback;
  if (bundle) {
    writeFile = createBundleWriteFile(state);
  } else {
    writeFile = createCompileWriteFile(state);
  }

  const host = (state.host = new Host({
    bundle,
    target,
    writeFile,
  }));
  const compilerOptions = [defaultRuntimeCompileOptions];
  if (convertedOptions) {
    compilerOptions.push(convertedOptions);
  }
  if (unstable) {
    compilerOptions.push({
      lib: [
        "deno.unstable",
        ...((convertedOptions && convertedOptions.lib) || ["deno.window"]),
      ],
    });
  }
  if (bundle) {
    compilerOptions.push(defaultBundlerOptions);
  }
  host.mergeOptions(...compilerOptions);

  const program = ts.createProgram({
    rootNames,
    options: host.getCompilationSettings(),
    host,
    oldProgram: TS_SNAPSHOT_PROGRAM,
  });

  if (bundle) {
    setRootExports(program, rootNames[0]);
  }

  const diagnostics = ts
    .getPreEmitDiagnostics(program)
    .filter(({ code }) => !ignoredDiagnostics.includes(code));

  const emitResult = program.emit();

  assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

  assert(state.emitMap);
  util.log("<<< runtime compile finish", {
    rootName,
    sources: sources ? Object.keys(sources) : undefined,
    bundle,
    emitMap: Object.keys(state.emitMap),
  });

  const maybeDiagnostics = diagnostics.length
    ? fromTypeScriptDiagnostic(diagnostics).items
    : [];

  if (bundle) {
    return {
      diagnostics: maybeDiagnostics,
      output: state.bundleOutput,
    } as RuntimeBundleResult;
  } else {
    return {
      diagnostics: maybeDiagnostics,
      emitMap: state.emitMap,
    } as RuntimeCompileResult;
  }
}

function runtimeTranspile(
  request: CompilerRequestRuntimeTranspile
): Promise<Record<string, TranspileOnlyResult>> {
  const result: Record<string, TranspileOnlyResult> = {};
  const { sources, options } = request;
  const compilerOptions = options
    ? Object.assign(
        {},
        defaultTranspileOptions,
        convertCompilerOptions(options).options
      )
    : defaultTranspileOptions;

  for (const [fileName, inputText] of Object.entries(sources)) {
    const { outputText: source, sourceMapText: map } = ts.transpileModule(
      inputText,
      {
        fileName,
        compilerOptions,
      }
    );
    result[fileName] = { source, map };
  }
  return Promise.resolve(result);
}

async function tsCompilerOnMessage({
  data: request,
}: {
  data: CompilerRequest;
}): Promise<void> {
  switch (request.type) {
    case CompilerRequestType.Compile: {
      const result = await compile(request as CompilerRequestCompile);
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeCompile: {
      const result = await runtimeCompile(
        request as CompilerRequestRuntimeCompile
      );
      globalThis.postMessage(result);
      break;
    }
    case CompilerRequestType.RuntimeTranspile: {
      const result = await runtimeTranspile(
        request as CompilerRequestRuntimeTranspile
      );
      globalThis.postMessage(result);
      break;
    }
    default:
      util.log(
        `!!! unhandled CompilerRequestType: ${
          (request as CompilerRequest).type
        } (${CompilerRequestType[(request as CompilerRequest).type]})`
      );
  }
  // Currently Rust shuts down worker after single request
}

async function wasmCompilerOnMessage({
  data: binary,
}: {
  data: string;
}): Promise<void> {
  const buffer = base64ToUint8Array(binary);
  // @ts-ignore
  const compiled = await WebAssembly.compile(buffer);

  util.log(">>> WASM compile start");

  const importList = Array.from(
    // @ts-ignore
    new Set(WebAssembly.Module.imports(compiled).map(({ module }) => module))
  );
  const exportList = Array.from(
    // @ts-ignore
    new Set(WebAssembly.Module.exports(compiled).map(({ name }) => name))
  );

  globalThis.postMessage({ importList, exportList });

  util.log("<<< WASM compile end");

  // Currently Rust shuts down worker after single request
}

function bootstrapTsCompilerRuntime(): void {
  bootstrapWorkerRuntime("TS", false);
  globalThis.onmessage = tsCompilerOnMessage;
}

function bootstrapWasmCompilerRuntime(): void {
  bootstrapWorkerRuntime("WASM", false);
  globalThis.onmessage = wasmCompilerOnMessage;
}

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete (Object.prototype as any).__proto__;

Object.defineProperties(globalThis, {
  bootstrap: {
    value: {
      ...globalThis.bootstrap,
      wasmCompilerRuntime: bootstrapWasmCompilerRuntime,
      tsCompilerRuntime: bootstrapTsCompilerRuntime,
    },
    configurable: true,
    writable: true,
  },
});
