// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="./compiler.d.ts" />
// deno-lint-ignore-file no-undef

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to type check TypeScript, and in some
// instances convert TypeScript to JavaScript.

// Removes the `__proto__` for security reasons.
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
delete Object.prototype.__proto__;

import {
  debug,
  filterMapDiagnostic,
  fromTypeScriptDiagnostics,
  host,
  setLogDebug,
} from "./97_ts_host.js";
import { serverMainLoop } from "./98_lsp.js";

/** @type {DenoCore} */
const core = globalThis.Deno.core;
const ops = core.ops;

/** @type {Array<[string, number]>} */
const stats = [];
let statsStart = 0;

function performanceStart() {
  stats.length = 0;
  statsStart = Date.now();
  ts.performance.enable();
}

/**
 * @param {{ program: ts.Program | ts.EmitAndSemanticDiagnosticsBuilderProgram, fileCount?: number }} options
 */
function performanceProgram({ program, fileCount }) {
  if (program) {
    if ("getProgram" in program) {
      program = program.getProgram();
    }
    stats.push(["Files", program.getSourceFiles().length]);
    stats.push(["Nodes", program.getNodeCount()]);
    stats.push(["Identifiers", program.getIdentifierCount()]);
    stats.push(["Symbols", program.getSymbolCount()]);
    stats.push(["Types", program.getTypeCount()]);
    stats.push(["Instantiations", program.getInstantiationCount()]);
  } else if (fileCount != null) {
    stats.push(["Files", fileCount]);
  }
  const programTime = ts.performance.getDuration("Program");
  const bindTime = ts.performance.getDuration("Bind");
  const checkTime = ts.performance.getDuration("Check");
  const emitTime = ts.performance.getDuration("Emit");
  stats.push(["Parse time", programTime]);
  stats.push(["Bind time", bindTime]);
  stats.push(["Check time", checkTime]);
  stats.push(["Emit time", emitTime]);
  stats.push(
    ["Total TS time", programTime + bindTime + checkTime + emitTime],
  );
}

function performanceEnd() {
  const duration = Date.now() - statsStart;
  stats.push(["Compile time", duration]);
  return stats;
}

/**
 * @typedef {object} Request
 * @property {Record<string, any>} config
 * @property {boolean} debug
 * @property {string[]} rootNames
 * @property {boolean} localOnly
 */

/** @param {Record<string, unknown>} config */
function normalizeConfig(config) {
  // the typescript compiler doesn't know about the precompile
  // transform at the moment, so just tell it we're using react-jsx
  if (config.jsx === "precompile") {
    config.jsx = "react-jsx";
  }
  if (config.jsxPrecompileSkipElements) {
    delete config.jsxPrecompileSkipElements;
  }
  return config;
}

/** The API that is called by Rust when executing a request.
 * @param {Request} request
 */
function exec({ config, debug: debugFlag, rootNames, localOnly }) {
  setLogDebug(debugFlag, "TS");
  performanceStart();

  config = normalizeConfig(config);

  debug(">>> exec start", { rootNames });
  debug(config);

  const { options, errors: configFileParsingDiagnostics } = ts
    .convertCompilerOptionsFromJson(config, "");
  Object.assign(options, {
    // The `allowNonTsExtensions` is a "hidden" compiler option used in VSCode
    // which is not allowed to be passed in JSON, we need it to allow special
    // URLs which Deno supports. So we need to either ignore the diagnostic, or
    // inject it ourselves.
    allowNonTsExtensions: true,
    // This is special functionality we inject into the compiler options
    // so that TypeScript can resolve jsxImportSource based on a referrer.
    resolveJsxImportSource: ops.op_resolve_jsx_import_source,
  });
  const program = ts.createIncrementalProgram({
    rootNames,
    options,
    host,
    configFileParsingDiagnostics,
  });

  let checkFiles = undefined;

  if (localOnly) {
    const checkFileNames = new Set();
    checkFiles = [];

    for (const checkName of rootNames) {
      if (checkName.startsWith("http")) {
        continue;
      }
      const sourceFile = program.getSourceFile(checkName);
      if (sourceFile != null) {
        checkFiles.push(sourceFile);
      }
      checkFileNames.add(checkName);
    }

    // When calling program.getSemanticDiagnostics(...) with a source file, we
    // need to call this code first in order to get it to invalidate cached
    // diagnostics correctly. This is what program.getSemanticDiagnostics()
    // does internally when calling without any arguments.
    while (
      program.getSemanticDiagnosticsOfNextAffectedFile(
        undefined,
        /* ignoreSourceFile */ (s) => !checkFileNames.has(s.fileName),
      )
    ) {
      // keep going until there are no more affected files
    }
  }

  const diagnostics = [
    ...program.getConfigFileParsingDiagnostics(),
    ...(checkFiles == null
      ? program.getSyntacticDiagnostics()
      : ts.sortAndDeduplicateDiagnostics(
        checkFiles.map((s) => program.getSyntacticDiagnostics(s)).flat(),
      )),
    ...program.getOptionsDiagnostics(),
    ...program.getGlobalDiagnostics(),
    ...(checkFiles == null
      ? program.getSemanticDiagnostics()
      : ts.sortAndDeduplicateDiagnostics(
        checkFiles.map((s) => program.getSemanticDiagnostics(s)).flat(),
      )),
    ...(options.isolatedDeclarations
      ? program.getDeclarationDiagnostics()
      : []),
  ].filter(filterMapDiagnostic);

  // emit the tsbuildinfo file
  // @ts-ignore: emitBuildInfo is not exposed (https://github.com/microsoft/TypeScript/issues/49871)
  program.emitBuildInfo(host.writeFile);

  performanceProgram({ program });

  const checker = program.getProgram().getTypeChecker();
  ops.op_respond({
    diagnostics: fromTypeScriptDiagnostics(diagnostics),
    ambientModules: checker.getAmbientModules().map((symbol) => symbol.name),
    stats: performanceEnd(),
  });
  debug("<<< exec stop");
}

const libs = ops.op_libs();
for (const lib of libs) {
  const specifier = `lib.${lib}.d.ts`;
  // we are using internal APIs here to "inject" our custom libraries into
  // tsc, so things like `"lib": [ "deno.ns" ]` are supported.
  if (!ts.libs.includes(lib)) {
    ts.libs.push(lib);
    ts.libMap.set(lib, specifier);
  }
}

// exposes the functions that are called by `tsc::exec()` when type
// checking TypeScript.
globalThis.exec = exec;

// exposes the functions that are called when the compiler is used as a
// language service.
globalThis.serverMainLoop = serverMainLoop;
