// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// @ts-check

const ASSETS = "asset:///";
const INTERNAL = "internal:///";
const CACHE = "cache:///";
const EXTERNAL_SPECIFIER_REGEX =
  /^file:\/{3}\S+\/(?:js|tests)\/(\S+\.(?:ts|js))$/;
const MAX_ERRORS = 5;

const IGNORED_DIAGNOSTICS = [
  1063, // TS1063: An export assignment cannot be used in a namespace.
  2691, // TS2691: An import path cannot end with a '.ts' extension.
  5009, // TS5009: Cannot find the common subdirectory path for the input files.
];

/** **Warning!** Op ids must be acquired from Rust using `Deno.core.ops()`
 * before dispatching any action.
 * @type {Record<string, number>} */
let ops;

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

/** @param {number} code */
function exit(code) {
  dispatch("op_exit2", { code });
  return unreachable();
}

/** @param  {...string} s */
function println(...s) {
  Deno.core.print(`${s.join(" ")}\n`);
}

/** @returns {never} */
function unreachable() {
  throw Error("unreachable");
}

/**
 * @type {Map<string, string>}
 */
const moduleMap = new Map();

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
  } else if (fileName.endsWith(".json")) {
    return ts.Extension.Json;
  } else if (fileName.endsWith(".jsx")) {
    return ts.Extension.Jsx;
  } else if (fileName.endsWith(".tsx")) {
    return ts.Extension.Tsx;
  } else if (fileName.endsWith(".tsbuildinfo")) {
    return ts.Extension.TsBuildInfo;
  } else {
    throw TypeError(`Cannot resolve extension for ${fileName}`);
  }
}

/**
 * @param {string} specifier
 * @returns {string}
 */
function getInternalSpecifier(specifier) {
  if (!moduleMap.has(specifier)) {
    const result = EXTERNAL_SPECIFIER_REGEX.exec(specifier);
    if (result) {
      const [, mid] = result;
      const internalSpecifier = `${INTERNAL}${mid}`;
      moduleMap.set(internalSpecifier, specifier);
      return internalSpecifier;
    }
    return specifier;
  }
  return moduleMap.get(specifier);
}

/** @type {ts.CompilerHost} */
const host = {
  fileExists(fileName) {
    println(`host.fileExists("${fileName}")`);
    return unreachable();
  },
  readFile(fileName) {
    return dispatch("op_read_file", { fileName }).data;
  },
  getSourceFile(fileName, languageVersion, onError, shouldCreateNewSourceFile) {
    assert(!shouldCreateNewSourceFile); // We haven't yet encountered this.
    const moduleUrl = moduleMap.get(fileName) ?? fileName;

    try {
      /** @type {{ sourceCode: string; hash: string; }} */
      const { sourceCode, hash } = dispatch("op_load_module", {
        moduleUrl,
        languageVersion,
        shouldCreateNewSourceFile,
      });
      const sourceFile = ts.createSourceFile(
        fileName,
        sourceCode,
        languageVersion,
      );
      sourceFile.moduleName = fileName;
      sourceFile.version = hash;
      return sourceFile;
    } catch (err) {
      if (onError) {
        onError(err instanceof Error ? err.message : "unexpected error");
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
    let maybeModuleName;
    if (sourceFiles) {
      assert(sourceFiles.length === 1, "unexpected number of source files");
      const [sourceFile] = sourceFiles;
      maybeModuleName = sourceFile.moduleName;
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
  resolveModuleNames(moduleNames, containingFile) {
    containingFile = moduleMap.get(containingFile) ?? containingFile;
    /** @type {string[]} */
    const resolvedNames = dispatch("op_resolve_module_names", {
      moduleNames,
      containingFile,
    });
    return resolvedNames.map((resolvedFileName) => {
      const extension = getExtension(resolvedFileName);
      resolvedFileName = getInternalSpecifier(resolvedFileName);
      return { resolvedFileName, extension, isExternalLibraryImport: false };
    });
  },
  createHash(data) {
    return dispatch("op_create_hash", { data }).hash;
  },
};

/**
 * @param {string} configText
 * @returns {{ options: ts.CompilerOptions, diagnostics?: ts.Diagnostic[] }}
 */
function configure(configText) {
  const { config, error } = ts.parseConfigFileTextToJson(
    "tsconfig.json",
    configText,
  );
  if (error) {
    return { options: {}, diagnostics: [error] };
  }
  const { options, errors } = ts.convertCompilerOptionsFromJson(
    config.compilerOptions,
    "",
    "tsconfig.json",
  );
  return { options, diagnostics: errors.length ? errors : undefined };
}

/**
 *
 * @param {ts.CompilerHost} host
 * @param {ReadonlyArray<ts.Diagnostic>} diagnostics
 */
function handleDiagnostics(host, diagnostics = []) {
  if (diagnostics.length) {
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
    exit(diagnostics.length + rest);
  }
}

/**
 * @param {string} configText
 * @param {string[]} rootNames
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
function main(configText, rootNames) {
  ops = Deno.core.ops();
  println(`>>> ts version ${ts.version}`);
  println(`>>> rootNames ${rootNames}`);

  assert(rootNames.length === 1, "only single root names supported");
  const [rootName] = rootNames;
  const rootSpecifier = getInternalSpecifier(rootName);
  const { options, diagnostics: configFileParsingDiagnostics } = configure(
    configText,
  );

  const program = ts.createIncrementalProgram({
    rootNames: [rootSpecifier],
    options,
    host,
    configFileParsingDiagnostics,
  });

  const preEmitDiagnostics = [
    ...program.getConfigFileParsingDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getOptionsDiagnostics(),
    ...program.getGlobalDiagnostics(),
    ...program.getSemanticDiagnostics(),
  ].filter(({ code }) => !IGNORED_DIAGNOSTICS.includes(code));
  handleDiagnostics(host, preEmitDiagnostics);

  const emitResult = program.emit();

  dispatch("op_set_emit_result", Object.assign(emitResult, { rootSpecifier }));
}
