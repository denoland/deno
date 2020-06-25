// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Because we're bootstrapping the TypeScript compiler without dependencies on
// Node, this is written in JavaScript, but leverages JSDoc that can be
// understood by the TypeScript language service, so it allows type safety
// checking in VSCode.

"use strict";

const ASSETS = "$asset$";

/**
 * @param {string} configText
 * @param {Array<string>} rootNames
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
function main(configText, rootNames) {
  ops = Deno.core.ops();
  println(`>>> ts version ${ts.version}`);
  println(`>>> rootNames ${rootNames}`);

  const host = new Host();

  assert(rootNames.length === 1);
  // If root file is external file, ie. URL with "file://"
  // then create an internal name - in case of bundling
  // cli runtime this is always true.
  const rootFile = rootNames[0];
  const result = externalSpecifierRegEx.exec(rootFile);
  let rootSpecifier = rootFile;
  if (result) {
    const [, specifier] = result;
    const internalSpecifier = `$deno$${specifier}`;
    moduleMap.set(internalSpecifier, rootFile);
    rootSpecifier = internalSpecifier;
  }
  const { options, diagnostics } = configure(configText);
  handleDiagnostics(host, diagnostics);

  println(`>>> TS config: ${JSON.stringify(options)}`);

  const program = ts.createProgram([rootSpecifier], options, host);

  handleDiagnostics(
    host,
    ts.getPreEmitDiagnostics(program).filter(({ code }) => {
      // TS1063: An export assignment cannot be used in a namespace.
      if (code === 1063) return false;
      // TS2691: An import path cannot end with a '.ts' extension. Consider
      // importing 'bad-module' instead.
      if (code === 2691) return false;
      // TS5009: Cannot find the common subdirectory path for the input files.
      if (code === 5009) return false;
      return true;
    })
  );

  const emitResult = program.emit();
  handleDiagnostics(host, emitResult.diagnostics);

  dispatch(
    "op_set_emit_result",
    Object.assign(emitResult, { tsVersion: ts.version })
  );
}

/**
 * @param {...string} s
 */
function println(...s) {
  Deno.core.print(s.join(" ") + "\n");
}

/**
 * @returns {never}
 */
function unreachable() {
  throw Error("unreachable");
}

/**
 * @param {unknown} cond
 * @returns {asserts cond}
 */
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

/**
 * @param {Uint8Array | null} ui8
 */
function decodeAscii(ui8) {
  let out = "";
  if (!ui8) {
    return out;
  }
  for (let i = 0; i < ui8.length; i++) {
    out += String.fromCharCode(ui8[i]);
  }
  return out;
}

/**
 * @param {string} str
 */
function encode(str) {
  const charCodes = str.split("").map((c) => c.charCodeAt(0));
  const ui8 = new Uint8Array(charCodes);
  return ui8;
}

/** **Warning!** Op ids must be acquired from Rust using `Deno.core.ops()`
 * before dispatching any action.
 * @type {Record<string, number>}
 */
let ops;

/**
 * @type {Map<string, string>}
 */
const moduleMap = new Map();

const externalSpecifierRegEx = /^file:\/{3}\S+\/js(\/\S+\.ts)$/;

/**
 * This is a minimal implementation of a compiler host to be able to allow the
 * creation of runtime bundles.  Some of the methods are implemented in a way
 * to just appease the TypeScript compiler, not to necessarily be a general
 * purpose implementation.
 *
 * @implements {ts.CompilerHost}
 */
class Host {
  /**
   * @param {string} _fileName
   */
  fileExists(_fileName) {
    return true;
  }

  /**
   * @param {string} _fileName
   */
  readFile(_fileName) {
    unreachable();
  }

  useCaseSensitiveFileNames() {
    return false;
  }

  /**
   * @param {ts.CompilerOptions} _options
   */
  getDefaultLibFileName(_options) {
    return "lib.esnext.d.ts";
  }

  getDefaultLibLocation() {
    return ASSETS;
  }

  getCurrentDirectory() {
    return ".";
  }

  /**
   * @param {string} fileName
   * @param {ts.ScriptTarget} languageVersion
   * @param {(message: string) => void} _onError
   * @param {boolean} shouldCreateNewSourceFile
   */
  getSourceFile(
    fileName,
    languageVersion,
    _onError,
    shouldCreateNewSourceFile
  ) {
    assert(!shouldCreateNewSourceFile); // We haven't yet encountered this.

    // This hacks around the fact that TypeScript tries to magically guess the
    // d.ts filename.
    if (fileName.startsWith("$typeRoots$")) {
      assert(fileName.startsWith("$typeRoots$/"));
      assert(fileName.endsWith("/index.d.ts"));
      fileName = fileName
        .replace("$typeRoots$/", "")
        .replace("/index.d.ts", "");
    }

    // This looks up any modules that have been mapped to internal names
    const moduleUrl = moduleMap.has(fileName)
      ? moduleMap.get(fileName)
      : fileName;

    const { sourceCode } = dispatch("op_load_module", {
      moduleUrl,
      languageVersion,
      shouldCreateNewSourceFile,
    });

    const sourceFile = ts.createSourceFile(
      fileName,
      sourceCode,
      languageVersion
    );
    sourceFile.moduleName = fileName;
    return sourceFile;
  }

  /**
   * @param {string} fileName
   * @param {string} data
   * @param {boolean} _writeByteOrderMark
   * @param {((message: string) => void)?} _onError
   * @param {ReadonlyArray<ts.SourceFile>?} sourceFiles
   */
  writeFile(
    fileName,
    data,
    _writeByteOrderMark,
    _onError = null,
    sourceFiles = null
  ) {
    if (sourceFiles == null) {
      return;
    }
    const moduleName = sourceFiles[sourceFiles.length - 1].moduleName;
    return dispatch("op_write_file", { fileName, moduleName, data });
  }

  /**
   * @param {string} _fileName
   * @param {ts.Path} _path
   * @param {ts.ScriptTarget} _languageVersion
   * @param {*} _onError
   * @param {boolean} _shouldCreateNewSourceFile
   */
  getSourceFileByPath(
    _fileName,
    _path,
    _languageVersion,
    _onError,
    _shouldCreateNewSourceFile
  ) {
    unreachable();
  }

  /**
   * @param {string} fileName
   */
  getCanonicalFileName(fileName) {
    return fileName;
  }

  getNewLine() {
    return "\n";
  }

  /**
   * @param {string[]} moduleNames
   * @param {string} containingFile
   * @return {Array<ts.ResolvedModule | undefined>}
   */
  resolveModuleNames(moduleNames, containingFile) {
    // If the containing file is an internal specifier, map it back to the
    // external specifier
    containingFile = moduleMap.has(containingFile)
      ? moduleMap.get(containingFile)
      : containingFile;
    /** @type {string[]} */
    const resolvedNames = dispatch("op_resolve_module_names", {
      moduleNames,
      containingFile,
    });
    /** @type {ts.ResolvedModule[]} */
    const r = resolvedNames.map((resolvedFileName) => {
      const extension = getExtension(resolvedFileName);
      if (!moduleMap.has(resolvedFileName)) {
        // If we match the external specifier regex, we will then create an internal
        // specifier and then use that when creating the source file
        const result = externalSpecifierRegEx.exec(resolvedFileName);
        if (result) {
          const [, specifier] = result;
          const internalSpecifier = `$deno$${specifier}`;
          moduleMap.set(internalSpecifier, resolvedFileName);
          resolvedFileName = internalSpecifier;
        }
      }
      return { resolvedFileName, extension };
    });
    return r;
  }
}

/**
 * @param {string} configurationText
 */
function configure(configurationText) {
  const { config, error } = ts.parseConfigFileTextToJson(
    "tsconfig.json",
    configurationText
  );
  if (error) {
    return { options: {}, diagnostics: [error] };
  }
  const { options, errors } = ts.convertCompilerOptionsFromJson(
    config.compilerOptions,
    ""
  );
  return {
    options,
    diagnostics: errors.length ? errors : undefined,
  };
}

/**
 * @param {string} opName
 * @param {Record<string,any>} obj
 */
function dispatch(opName, obj) {
  const opId = ops[opName];

  if (!opId) {
    throw new Error(`Unknown op: ${opName}`);
  }

  const s = JSON.stringify(obj);
  const msg = encode(s);
  const resUi8 = Deno.core.dispatch(opId, msg);
  const resStr = decodeAscii(resUi8);
  const res = JSON.parse(resStr);
  if (!res["ok"]) {
    throw Error(`${opName} failed ${res["err"]}. Args: ${JSON.stringify(obj)}`);
  }
  return res["ok"];
}

/**
 * @param {number} code
 */
function exit(code) {
  dispatch("op_exit2", { code });
  return unreachable();
}

// Maximum number of diagnostics to display.
const MAX_ERRORS = 5;

/**
 * @param {ts.CompilerHost} host
 * @param {ReadonlyArray<ts.Diagnostic> | undefined} diagnostics
 */
function handleDiagnostics(host, diagnostics) {
  if (diagnostics && diagnostics.length) {
    let rest = 0;
    if (diagnostics.length > MAX_ERRORS) {
      rest = diagnostics.length - MAX_ERRORS;
      diagnostics = diagnostics.slice(0, MAX_ERRORS);
    }
    const msg = ts.formatDiagnosticsWithColorAndContext(diagnostics, host);
    println(msg);
    if (rest) {
      println(`And ${rest} other errors.`);
    }
    exit(1);
  }
}

/** Returns the TypeScript Extension enum for a given media type.
 * @param {string} fileName
 * @returns {ts.Extension}
 */
function getExtension(fileName) {
  if (fileName.endsWith(".d.ts")) {
    return ts.Extension.Dts;
  } else if (fileName.endsWith(".ts")) {
    return ts.Extension.Ts;
  } else if (fileName.endsWith(".js")) {
    return ts.Extension.Js;
  } else {
    throw TypeError(`Cannot resolve extension for ${fileName}`);
  }
}
