// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// @ts-check
// eslint-disable-next-line @typescript-eslint/triple-slash-reference
/// <reference path="./globals.d.ts" />

const ASSETS = "asset:///";
const CACHE = "cache:///";

const IGNORED_DIAGNOSTICS = [
  1063, // TS1063: An export assignment cannot be used in a namespace.
  2691, // TS2691: An import path cannot end with a '.ts' extension.
  5009, // TS5009: Cannot find the common subdirectory path for the input files.
];

/** **Warning!** Op ids must be acquired from Rust using `Deno.core.ops()`
 * before dispatching any action.
 * @type {Record<string, number>} */
let ops;

/** @type {boolean | undefined} */
let debugFlag = true;

/** @type {Map<string, ts.SourceFile>} */
const sourceFileCache = new Map();

/**
 * @param {unknown} cond
 * @param {string} msg
 * @returns {asserts cond}
 */
function assert(cond, msg = "assert") {
  if (!cond) {
    throw Error(msg);
  }
}

/**
 * @param {string} opName
 * @param {Record<string,any>} args
 */
function dispatch(opName, args) {
  const opId = ops[opName];

  if (!opId) {
    throw new Error(`Unknown op: ${opName}`);
  }

  const msg = Deno.core.encode(JSON.stringify(args));
  const resUi8 = Deno.core.dispatch(opId, msg);
  const res = JSON.parse(Deno.core.decode(resUi8));
  if (!res["ok"]) {
    throw Error(
      `${opName} failed ${res["err"]}. Args: ${JSON.stringify(args)}`,
    );
  }
  return res["ok"];
}

/** @param  {...string} s */
function println(...s) {
  if (debugFlag) {
    Deno.core.print(`[TS]: ${s.join(" ")}\n`);
  }
}

/** @returns {never} */
function unreachable() {
  throw Error("unreachable");
}

/**
 * @param {readonly ts.Diagnostic[]} diagnostics
 * @returns {readonly any[]}
 */
function processDiagnostics(diagnostics) {
  return diagnostics.map((diagnostic) => {
    /** @type {any} */
    const { messageText, file, ...result } = diagnostic;
    if (file) {
      result.sourceFile = file.fileName;
    }
    if (typeof messageText === "string") {
      result.messageText = messageText;
    } else {
      result.messageChain = messageText;
    }
    return result;
  });
}

/** @type {ts.CompilerHost} */
const host = {
  fileExists(fileName) {
    println(`host.fileExists("${fileName}")`);
    return unreachable();
  },
  readFile(fileName) {
    println(`host.readFile("${fileName}")`);
    return dispatch("op_read_file", { fileName }).data;
  },
  getSourceFile(
    specifier,
    languageVersion,
    onError,
    _shouldCreateNewSourceFile,
  ) {
    println(
      `host.getSourceFile("${specifier}", ${ts.ScriptTarget[languageVersion]})`,
    );
    const sourceFile = sourceFileCache.get(specifier);
    if (sourceFile) {
      return sourceFile;
    }

    try {
      /** @type {{ data: string; hash: string; }} */
      const { data, hash } = dispatch("op_load_module", { specifier });
      const sourceFile = ts.createSourceFile(specifier, data, languageVersion);
      sourceFile.moduleName = specifier;
      sourceFile.version = hash;
      sourceFileCache.set(specifier, sourceFile);
      return sourceFile;
    } catch (err) {
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      println(`  !! error: ${message}`);
      if (onError) {
        onError(message);
      } else {
        throw err;
      }
    }
  },
  getDefaultLibFileName() {
    return `lib.esnext.d.ts`;
  },
  getDefaultLibLocation() {
    return ASSETS;
  },
  writeFile(fileName, data, _writeByteOrderMark, _onError, sourceFiles) {
    println(`host.writeFile("${fileName}")`);
    let maybeModuleName;
    if (sourceFiles) {
      assert(sourceFiles.length === 1, "unexpected number of source files");
      const [sourceFile] = sourceFiles;
      maybeModuleName = sourceFile.moduleName;
      println(`  moduleName: ${maybeModuleName}`);
    }
    return dispatch("op_write_file", { maybeModuleName, fileName, data });
  },
  getCurrentDirectory() {
    return CACHE;
  },
  getCanonicalFileName(fileName) {
    return fileName;
  },
  useCaseSensitiveFileNames() {
    return true;
  },
  getNewLine() {
    return "\n";
  },
  resolveModuleNames(specifiers, base) {
    println(`host.resolveModuleNames()`);
    println(`  base: ${base}`);
    println(`  specifiers: ${specifiers.join(", ")}`);
    /** @type {Array<[string, ts.Extension]>} */
    const resolved = dispatch("op_resolve_specifiers", {
      specifiers,
      base,
    });
    return resolved.map(([resolvedFileName, extension]) => ({
      resolvedFileName,
      extension,
      isExternalLibraryImport: false,
    }));
  },
  createHash(data) {
    return dispatch("op_create_hash", { data }).hash;
  },
};

/**
 * @param {Record<string, any>} compilerOptions
 * @returns {{ options: ts.CompilerOptions, diagnostics?: ts.Diagnostic[] }}
 */
function configure(compilerOptions) {
  const { options, errors } = ts.convertCompilerOptionsFromJson(
    compilerOptions,
    "",
    "tsconfig.json",
  );
  return { options, diagnostics: errors.length ? errors : undefined };
}

/**
 * @param {[string, number][]} stats
 * @param {number} start
 */
function addTimes(stats, start) {
  const programTime = ts.performance.getDuration("Program");
  const bindTime = ts.performance.getDuration("Bind");
  const checkTime = ts.performance.getDuration("Check");
  const emitTime = ts.performance.getDuration("Emit");
  stats.push(["Parse time", programTime]);
  stats.push(["Bind time", bindTime]);
  stats.push(["Check time", checkTime]);
  stats.push(["Emit time", emitTime]);
  stats.push(["Total TS time", programTime + bindTime + checkTime + emitTime]);
  ts.performance.disable();
  stats.push(["Duration", Date.now() - start]);
}

/**
 * @typedef {object} Source
 * @property {string} data
 * @property {string=} hash
 */

/**
 * @typedef {object} CompileRequest
 * @property {Record<string, any>} compilerOptions
 * @property {boolean} debug
 * @property {string[]} rootNames
 */

/* eslint-disable @typescript-eslint/no-unused-vars */

/**
 * @param {CompileRequest} request
 */
function compile(request) {
  const start = Date.now();
  ts.performance.enable();
  const { compilerOptions, debug, rootNames } = request;
  debugFlag = debug;

  println(">>> compile");

  const { options, diagnostics: configFileParsingDiagnostics } = configure(
    compilerOptions,
  );

  assert(rootNames.length === 1, "only single root names supported");
  const [rootSpecifier] = rootNames;
  const builderProgram = ts.createIncrementalProgram({
    rootNames: [rootSpecifier],
    options,
    host,
    configFileParsingDiagnostics,
  });

  const diagnostics = [
    ...builderProgram.getConfigFileParsingDiagnostics(),
    ...builderProgram.getSyntacticDiagnostics(),
    ...builderProgram.getOptionsDiagnostics(),
    ...builderProgram.getGlobalDiagnostics(),
    ...builderProgram.getSemanticDiagnostics(),
  ].filter(({ code }) => !IGNORED_DIAGNOSTICS.includes(code));

  /** @type {ts.EmitResult} */
  let emitResult;

  if (diagnostics.length) {
    emitResult = { emitSkipped: true, diagnostics };
  } else {
    const { emitSkipped, diagnostics: emitDiagnostics } = builderProgram.emit();
    emitResult = {
      emitSkipped,
      diagnostics: emitDiagnostics.filter(
        ({ code }) => !IGNORED_DIAGNOSTICS.includes(code),
      ),
    };
  }

  /** @type {[string, number][]} */
  const stats = [];
  const program = builderProgram.getProgram();
  stats.push(["Files", program.getSourceFiles().length]);
  stats.push(["Nodes", program.getNodeCount()]);
  stats.push(["Identifiers", program.getIdentifierCount()]);
  stats.push(["Symbols", program.getSymbolCount()]);
  stats.push(["Types", program.getTypeCount()]);
  stats.push(["Instantiations", program.getInstantiationCount()]);
  addTimes(stats, start);
  ts.performance.disable();

  dispatch(
    "op_set_emit_result",
    Object.assign(emitResult, {
      diagnostics: processDiagnostics(emitResult.diagnostics),
      stats,
    }),
  );
  println("<<< compile");
}

/**
 * @typedef {object} SourceFile
 * @property {string} data
 * @property {ts.MapLike<string>=} renamedDependencies
 */

/**
 * @typedef {object} TranspileRequest
 * @property {Record<string, any>} compilerOptions
 * @property {boolean} debug
 * @property {Record<string, SourceFile>} sources
 */

/**
 * @param {TranspileRequest} request
 */
function transpile(request) {
  const start = Date.now();
  ts.performance.enable();
  const { compilerOptions, debug, sources } = request;
  debugFlag = debug;

  println(">>> transpile");

  const { options, diagnostics } = configure(compilerOptions);

  /** @type {string[]} */
  const emittedFiles = [];
  /** @type {ts.EmitResult | undefined} */
  let emitResult;
  if (diagnostics && diagnostics.length) {
    emitResult = { emitSkipped: true, diagnostics: diagnostics, emittedFiles };
  } else {
    for (
      const [fileName, { data, renamedDependencies }] of Object.entries(
        sources,
      )
    ) {
      const { outputText, sourceMapText, diagnostics } = ts.transpileModule(
        data,
        {
          fileName,
          moduleName: fileName,
          compilerOptions: options,
          reportDiagnostics: true,
          renamedDependencies,
        },
      );
      if (diagnostics && diagnostics.length) {
        emitResult = { emitSkipped: true, diagnostics, emittedFiles };
        break;
      }
      assert(outputText, "missing code output");
      const codeFileName = fileName.replace(/\.ts$/, ".js");
      dispatch("op_write_file", {
        maybeModuleName: fileName,
        fileName: codeFileName,
        data: outputText,
      });
      emittedFiles.push(codeFileName);
      if (options.sourceMap) {
        assert(sourceMapText, "missing source map");
        const mapFileName = fileName.replace(/\.ts$/, ".js.map");
        dispatch("op_write_file", {
          maybeModuleName: fileName,
          fileName: mapFileName,
          data: sourceMapText,
        });
        emittedFiles.push(mapFileName);
      }
    }
    if (!emitResult) {
      emitResult = { emitSkipped: false, diagnostics: [], emittedFiles };
    }
  }

  /** @type {[string, number][]} */
  const stats = [];
  addTimes(stats, start);
  ts.performance.disable();

  dispatch(
    "op_set_emit_result",
    Object.assign(emitResult, {
      diagnostics: processDiagnostics(emitResult.diagnostics),
      stats,
    }),
  );
  println("<<< transpile");
}

/**
 * @typedef {object} BootstrapConfig
 * @property {string} bootSpecifier
 * @property {Record<string, any>} compilerOptions
 * @property {Record<string, string>} libs
 */

/**
 * Bootstrap the compiler.  This acquires the ops for the isolate and generates
 * a program, which hydrates the static assets as source modules.  At this point
 * the isolate can be snapshotted with as much of the TypeScript compiler warmed
 * up as possible.
 *
 * @param {BootstrapConfig} config
 */
function main(config) {
  println(`main(${JSON.stringify(config)})`);
  ops = Deno.core.ops();
  const { bootSpecifier, compilerOptions, libs } = config;
  for (const [lib, specifier] of Object.entries(libs)) {
    if (!ts.libs.includes(lib)) {
      ts.libs.push(lib);
      ts.libMap.set(lib, specifier);
    }
    assert(host.getSourceFile(`${ASSETS}${specifier}`, ts.ScriptTarget.ESNext));
  }
  const { options, diagnostics: configFileParsingDiagnostics } = configure(
    compilerOptions,
  );
  const program = ts.createProgram({
    rootNames: [bootSpecifier],
    options,
    host,
    configFileParsingDiagnostics,
  });
  program.emit();
  dispatch("op_set_version", { version: ts.version });
}
/* eslint-enable */
