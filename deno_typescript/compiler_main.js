// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Because we're bootstrapping the TypeScript compiler without dependencies on
// Node, this is written in JavaScript, but leverages JSDoc that can be
// understood by the TypeScript language service, so it allows type safety
// checking in VSCode.

const ASSETS = "$asset$";

/**
 * @param {string} configText
 * @param {Array<string>} rootNames
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
function main(configText, rootNames) {
  println(`>>> ts version ${ts.version}`);
  println(`>>> rootNames ${rootNames}`);

  const host = new Host();

  assert(rootNames.length > 0);

  const { options, diagnostics } = configure(configText);
  handleDiagnostics(host, diagnostics);

  println(`>>> TS config: ${JSON.stringify(options)}`);

  const program = ts.createProgram(rootNames, options, host);

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
    "setEmitResult",
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
  const charCodes = str.split("").map(c => c.charCodeAt(0));
  const ui8 = new Uint8Array(charCodes);
  return ui8;
}

//
/** **Warning!** The op_id values below are shared between this code and the
 * Rust side. Update with care!
 * @type {Record<string, number>}
 */
const ops = {
  readFile: 49,
  exit: 50,
  writeFile: 51,
  resolveModuleNames: 52,
  setEmitResult: 53
};

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
    return undefined;
  }

  useCaseSensitiveFileNames() {
    return false;
  }

  /**
   * @param {ts.CompilerOptions} _options
   */
  getDefaultLibFileName(_options) {
    return "lib.deno_core.d.ts";
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

    const { sourceCode, moduleName } = dispatch("readFile", {
      fileName,
      languageVersion,
      shouldCreateNewSourceFile
    });

    const sourceFile = ts.createSourceFile(
      fileName,
      sourceCode,
      languageVersion
    );
    sourceFile.moduleName = moduleName;
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
    return dispatch("writeFile", { fileName, moduleName, data });
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
    return undefined;
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
    /** @type {string[]} */
    const resolvedNames = dispatch("resolveModuleNames", {
      moduleNames,
      containingFile
    });
    const r = resolvedNames.map(resolvedFileName => {
      const extension = getExtension(resolvedFileName);
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
    diagnostics: errors.length ? errors : undefined
  };
}

/**
 * @param {string} opName
 * @param {Record<string,any>} obj
 */
function dispatch(opName, obj) {
  const s = JSON.stringify(obj);
  const msg = encode(s);
  const resUi8 = Deno.core.dispatch(ops[opName], msg);
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
  dispatch("exit", { code });
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
