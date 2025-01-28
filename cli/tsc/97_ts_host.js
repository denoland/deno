// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="./compiler.d.ts" />
// deno-lint-ignore-file no-undef

/** @type {DenoCore} */
const core = globalThis.Deno.core;
const ops = core.ops;

let logDebug = false;
let logSource = "JS";

// The map from the normalized specifier to the original.
// TypeScript normalizes the specifier in its internal processing,
// but the original specifier is needed when looking up the source from the runtime.
// This map stores that relationship, and the original can be restored by the
// normalized specifier.
// See: https://github.com/denoland/deno/issues/9277#issuecomment-769653834
/** @type {Map<string, string>} */
const normalizedToOriginalMap = new Map();

/** @type {ReadonlySet<string>} */
const unstableDenoProps = new Set([
  "AtomicOperation",
  "DatagramConn",
  "Kv",
  "KvListIterator",
  "KvU64",
  "UnixConnectOptions",
  "UnixListenOptions",
  "listen",
  "listenDatagram",
  "openKv",
  "connectQuic",
  "listenQuic",
  "QuicBidirectionalStream",
  "QuicConn",
  "QuicListener",
  "QuicReceiveStream",
  "QuicSendStream",
]);
const unstableMsgSuggestion =
  "If not, try changing the 'lib' compiler option to include 'deno.unstable' " +
  "or add a triple-slash directive to the top of your entrypoint (main file): " +
  '/// <reference lib="deno.unstable" />';

/**
 * @param {unknown} value
 * @returns {value is ts.CreateSourceFileOptions}
 */
function isCreateSourceFileOptions(value) {
  return value != null && typeof value === "object" &&
    "languageVersion" in value;
}

/**
 * @param {ts.ScriptTarget | ts.CreateSourceFileOptions | undefined} versionOrOptions
 * @returns {ts.CreateSourceFileOptions}
 */
export function getCreateSourceFileOptions(versionOrOptions) {
  return isCreateSourceFileOptions(versionOrOptions)
    ? versionOrOptions
    : { languageVersion: versionOrOptions ?? ts.ScriptTarget.ESNext };
}

/**
 * @param debug {boolean}
 * @param source {string}
 */
export function setLogDebug(debug, source) {
  logDebug = debug;
  if (source) {
    logSource = source;
  }
}

/** @param msg {string} */
function printStderr(msg) {
  core.print(msg, true);
}

/** @param args {any[]} */
export function debug(...args) {
  if (logDebug) {
    const stringifiedArgs = args.map((arg) =>
      typeof arg === "string" ? arg : JSON.stringify(arg)
    ).join(" ");
    printStderr(`DEBUG ${logSource} - ${stringifiedArgs}\n`);
  }
}

/** @param args {any[]} */
export function error(...args) {
  const stringifiedArgs = args.map((arg) =>
    typeof arg === "string" || arg instanceof Error
      ? String(arg)
      : JSON.stringify(arg)
  ).join(" ");
  printStderr(`ERROR ${logSource} = ${stringifiedArgs}\n`);
}

export class AssertionError extends Error {
  /** @param msg {string} */
  constructor(msg) {
    super(msg);
    this.name = "AssertionError";
  }
}

/** @param cond {boolean} */
export function assert(cond, msg = "Assertion failed.") {
  if (!cond) {
    throw new AssertionError(msg);
  }
}

// In the case of the LSP, this will only ever contain the assets.
/** @type {Map<string, ts.SourceFile>} */
export const SOURCE_FILE_CACHE = new Map();

/** @type {Map<string, ts.IScriptSnapshot & { isCjs?: boolean; }>} */
export const SCRIPT_SNAPSHOT_CACHE = new Map();

/** @type {Map<string, number>} */
export const SOURCE_REF_COUNTS = new Map();

/** @type {Map<string, string>} */
export const SCRIPT_VERSION_CACHE = new Map();

/** @type {Map<string, boolean>} */
export const IS_NODE_SOURCE_FILE_CACHE = new Map();

// Maps asset specifiers to the first scope that the asset was loaded into.
/** @type {Map<string, string | null>} */
export const ASSET_SCOPES = new Map();

/** @type {number | null} */
let projectVersionCache = null;
export const PROJECT_VERSION_CACHE = {
  get: () => projectVersionCache,
  set: (version) => {
    projectVersionCache = version;
  },
};

/** @type {string | null} */
let lastRequestMethod = null;
export const LAST_REQUEST_METHOD = {
  get: () => lastRequestMethod,
  set: (method) => {
    lastRequestMethod = method;
  },
};

/** @type {string | null} */
let lastRequestScope = null;
export const LAST_REQUEST_SCOPE = {
  get: () => lastRequestScope,
  set: (scope) => {
    lastRequestScope = scope;
  },
};

ts.deno.setIsNodeSourceFileCallback((sourceFile) => {
  const fileName = sourceFile.fileName;
  let isNodeSourceFile = IS_NODE_SOURCE_FILE_CACHE.get(fileName);
  if (isNodeSourceFile == null) {
    const result = ops.op_is_node_file(fileName);
    isNodeSourceFile = /** @type {boolean} */ (result);
    IS_NODE_SOURCE_FILE_CACHE.set(fileName, isNodeSourceFile);
  }
  return isNodeSourceFile;
});

/**
 * @param msg {string}
 * @param code {number}
 */
function formatMessage(msg, code) {
  switch (code) {
    case 2304: {
      if (msg === "Cannot find name 'Deno'.") {
        msg += " Do you need to change your target library? " +
          "Try changing the 'lib' compiler option to include 'deno.ns' " +
          "or add a triple-slash directive to the top of your entrypoint " +
          '(main file): /// <reference lib="deno.ns" />';
      }
      return msg;
    }
    case 2339: {
      const property = getProperty();
      if (property && unstableDenoProps.has(property)) {
        return `${msg} 'Deno.${property}' is an unstable API. ${unstableMsgSuggestion}`;
      }
      return msg;
    }
    default: {
      const property = getProperty();
      if (property && unstableDenoProps.has(property)) {
        const suggestion = getMsgSuggestion();
        if (suggestion) {
          return `${msg} 'Deno.${property}' is an unstable API. Did you mean '${suggestion}'? ${unstableMsgSuggestion}`;
        }
      }
      return msg;
    }
  }

  function getProperty() {
    return /Property '([^']+)' does not exist on type 'typeof Deno'/
      .exec(msg)?.[1];
  }

  function getMsgSuggestion() {
    return / Did you mean '([^']+)'\?/.exec(msg)?.[1];
  }
}

/** @param {ts.DiagnosticRelatedInformation} diagnostic */
function fromRelatedInformation({
  start,
  length,
  file,
  messageText: msgText,
  ...ri
}) {
  let messageText;
  let messageChain;
  if (typeof msgText === "object") {
    messageChain = msgText;
  } else {
    messageText = formatMessage(msgText, ri.code);
  }
  if (start !== undefined && length !== undefined && file) {
    let startPos = file.getLineAndCharacterOfPosition(start);
    let sourceLine = file.getFullText().split("\n")[startPos.line];
    const originalFileName = file.fileName;
    const fileName = ops.op_remap_specifier
      ? (ops.op_remap_specifier(file.fileName) ?? file.fileName)
      : file.fileName;
    // Bit of a hack to detect when we have a .wasm file and want to hide
    // the .d.ts text. This is not perfect, but will work in most scenarios
    if (
      fileName.endsWith(".wasm") && originalFileName.endsWith(".wasm.d.mts")
    ) {
      startPos = { line: 0, character: 0 };
      sourceLine = undefined;
    }
    return {
      start: startPos,
      end: file.getLineAndCharacterOfPosition(start + length),
      fileName,
      messageChain,
      messageText,
      sourceLine,
      ...ri,
    };
  } else {
    return {
      messageChain,
      messageText,
      ...ri,
    };
  }
}

/** @param {readonly ts.Diagnostic[]} diagnostics */
export function fromTypeScriptDiagnostics(diagnostics) {
  return diagnostics.map(({ relatedInformation: ri, source, ...diag }) => {
    /** @type {any} */
    const value = fromRelatedInformation(diag);
    value.relatedInformation = ri ? ri.map(fromRelatedInformation) : undefined;
    value.source = source;
    return value;
  });
}

// Using incremental compile APIs requires that all
// paths must be either relative or absolute. Since
// analysis in Rust operates on fully resolved URLs,
// it makes sense to use the same scheme here.
export const ASSETS_URL_PREFIX = "asset:///";
const CACHE_URL_PREFIX = "cache:///";

/** Diagnostics that are intentionally ignored when compiling TypeScript in
 * Deno, as they provide misleading or incorrect information. */
const IGNORED_DIAGNOSTICS = [
  // TS1452: 'resolution-mode' assertions are only supported when `moduleResolution` is `node16` or `nodenext`.
  // We specify the resolution mode to be CommonJS for some npm files and this
  // diagnostic gets generated even though we're using custom module resolution.
  1452,
  // Module '...' cannot be imported using this construct. The specifier only resolves to an
  // ES module, which cannot be imported with 'require'.
  1471,
  // TS1479: The current file is a CommonJS module whose imports will produce 'require' calls;
  // however, the referenced file is an ECMAScript module and cannot be imported with 'require'.
  1479,
  // TS2306: File '.../index.d.ts' is not a module.
  // We get this for `x-typescript-types` declaration files which don't export
  // anything. We prefer to treat these as modules with no exports.
  2306,
  // TS2688: Cannot find type definition file for '...'.
  // We ignore because type definition files can end with '.ts'.
  2688,
  // TS2792: Cannot find module. Did you mean to set the 'moduleResolution'
  // option to 'node', or to add aliases to the 'paths' option?
  2792,
  // TS2307: Cannot find module '{0}' or its corresponding type declarations.
  2307,
  // Relative import errors to add an extension
  2834,
  2835,
  // TS5009: Cannot find the common subdirectory path for the input files.
  5009,
  // TS5055: Cannot write file
  // 'http://localhost:4545/subdir/mt_application_x_javascript.j4.js'
  // because it would overwrite input file.
  5055,
  // TypeScript is overly opinionated that only CommonJS modules kinds can
  // support JSON imports.  Allegedly this was fixed in
  // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
  // so we will ignore complaints about this compiler setting.
  5070,
  // TS6053: File '{0}' not found.
  6053,
  // TS7016: Could not find a declaration file for module '...'. '...'
  // implicitly has an 'any' type.  This is due to `allowJs` being off by
  // default but importing of a JavaScript module.
  7016,
];

// todo(dsherret): can we remove this and just use ts.OperationCanceledException?
/** Error thrown on cancellation. */
export class OperationCanceledError extends Error {
}

/**
 * This implementation calls into Rust to check if Tokio's cancellation token
 * has already been canceled.
 * @implements {ts.CancellationToken}
 */
class CancellationToken {
  isCancellationRequested() {
    return ops.op_is_cancelled();
  }

  throwIfCancellationRequested() {
    if (this.isCancellationRequested()) {
      throw new OperationCanceledError();
    }
  }
}

/** @typedef {{
 *    ls: ts.LanguageService & { [k:string]: any },
 *    compilerOptions: ts.CompilerOptions,
 *  }} LanguageServiceEntry */
/** @type {{ unscoped: LanguageServiceEntry, byScope: Map<string, LanguageServiceEntry> }} */
export const LANGUAGE_SERVICE_ENTRIES = {
  // @ts-ignore Will be set later.
  unscoped: null,
  byScope: new Map(),
};

/** @type {{ unscoped: string[], byScope: Map<string, string[]> } | null} */
let SCRIPT_NAMES_CACHE = null;

export function clearScriptNamesCache() {
  SCRIPT_NAMES_CACHE = null;
}

/** An object literal of the incremental compiler host, which provides the
 * specific "bindings" to the Deno environment that tsc needs to work.
 *
 * @type {ts.CompilerHost & ts.LanguageServiceHost} */
export const host = {
  fileExists(specifier) {
    if (logDebug) {
      debug(`host.fileExists("${specifier}")`);
    }
    // TODO(bartlomieju): is this assumption still valid?
    // this is used by typescript to find the libs path
    // so we can completely ignore it
    return false;
  },
  readFile(specifier) {
    if (logDebug) {
      debug(`host.readFile("${specifier}")`);
    }
    return ops.op_load(specifier)?.data;
  },
  getCancellationToken() {
    // createLanguageService will call this immediately and cache it
    return new CancellationToken();
  },
  getProjectVersion() {
    const cachedProjectVersion = PROJECT_VERSION_CACHE.get();
    if (
      cachedProjectVersion
    ) {
      debug(`getProjectVersion cache hit : ${cachedProjectVersion}`);
      return cachedProjectVersion;
    }
    const projectVersion = ops.op_project_version();
    PROJECT_VERSION_CACHE.set(projectVersion);
    debug(`getProjectVersion cache miss : ${projectVersion}`);
    return projectVersion;
  },
  // @ts-ignore Undocumented method.
  getCachedExportInfoMap() {
    return exportMapCache;
  },
  getGlobalTypingsCacheLocation() {
    return undefined;
  },
  // @ts-ignore Undocumented method.
  toPath(fileName) {
    // @ts-ignore Undocumented function.
    return ts.toPath(
      fileName,
      this.getCurrentDirectory(),
      this.getCanonicalFileName.bind(this),
    );
  },
  // @ts-ignore Undocumented method.
  watchNodeModulesForPackageJsonChanges() {
    return { close() {} };
  },
  getSourceFile(
    specifier,
    languageVersion,
    _onError,
    // this is not used by the lsp because source
    // files are created in the document registry
    _shouldCreateNewSourceFile,
  ) {
    if (logDebug) {
      debug(
        `host.getSourceFile("${specifier}", ${
          ts.ScriptTarget[
            getCreateSourceFileOptions(languageVersion).languageVersion
          ]
        })`,
      );
    }

    // Needs the original specifier
    specifier = normalizedToOriginalMap.get(specifier) ?? specifier;

    let sourceFile = SOURCE_FILE_CACHE.get(specifier);
    if (sourceFile) {
      return sourceFile;
    }

    /** @type {{ data: string; scriptKind: ts.ScriptKind; version: string; isCjs: boolean }} */
    const fileInfo = ops.op_load(specifier);
    if (!fileInfo) {
      return undefined;
    }
    const { data, scriptKind, version, isCjs } = fileInfo;
    assert(
      data != null,
      `"data" is unexpectedly null for "${specifier}".`,
    );

    sourceFile = ts.createSourceFile(
      specifier,
      data,
      {
        ...getCreateSourceFileOptions(languageVersion),
        impliedNodeFormat: isCjs
          ? ts.ModuleKind.CommonJS
          : ts.ModuleKind.ESNext,
        // no need to parse docs for `deno check`
        jsDocParsingMode: ts.JSDocParsingMode.ParseForTypeErrors,
      },
      false,
      scriptKind,
    );
    sourceFile.moduleName = specifier;
    sourceFile.version = version;
    if (specifier.startsWith(ASSETS_URL_PREFIX)) {
      sourceFile.version = "1";
    }
    SOURCE_FILE_CACHE.set(specifier, sourceFile);
    SCRIPT_VERSION_CACHE.set(specifier, version);
    return sourceFile;
  },
  getDefaultLibFileName() {
    return `${ASSETS_URL_PREFIX}lib.esnext.d.ts`;
  },
  getDefaultLibLocation() {
    return ASSETS_URL_PREFIX;
  },
  writeFile(fileName, data, _writeByteOrderMark, _onError, _sourceFiles) {
    if (logDebug) {
      debug(`host.writeFile("${fileName}")`);
    }
    return ops.op_emit(
      data,
      fileName,
    );
  },
  getCurrentDirectory() {
    if (logDebug) {
      debug(`host.getCurrentDirectory()`);
    }
    return CACHE_URL_PREFIX;
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
  resolveTypeReferenceDirectiveReferences(
    typeDirectiveReferences,
    containingFilePath,
    _redirectedReference,
    options,
    containingSourceFile,
    _reusedNames,
  ) {
    const isCjs =
      containingSourceFile?.impliedNodeFormat === ts.ModuleKind.CommonJS;
    const toResolve = typeDirectiveReferences.map((arg) => {
      /** @type {ts.FileReference} */
      const fileReference = typeof arg === "string"
        ? {
          pos: -1,
          end: -1,
          fileName: arg,
        }
        : arg;
      return [
        fileReference.resolutionMode == null
          ? isCjs
          : fileReference.resolutionMode === ts.ModuleKind.CommonJS,
        fileReference.fileName,
      ];
    });

    /** @type {Array<[string, ts.Extension | null] | undefined>} */
    const resolved = ops.op_resolve(
      containingFilePath,
      toResolve,
    );

    /** @type {Array<ts.ResolvedTypeReferenceDirectiveWithFailedLookupLocations>} */
    const result = resolved.map((item) => {
      if (item && item[1]) {
        const [resolvedFileName, extension] = item;
        return {
          resolvedTypeReferenceDirective: {
            primary: true,
            resolvedFileName,
            extension,
            isExternalLibraryImport: false,
          },
        };
      } else {
        return {
          resolvedTypeReferenceDirective: undefined,
        };
      }
    });

    if (logDebug) {
      debug(
        "resolveTypeReferenceDirectiveReferences ",
        typeDirectiveReferences,
        containingFilePath,
        options,
        containingSourceFile?.fileName,
        " => ",
        result,
      );
    }
    return result;
  },
  resolveModuleNameLiterals(
    moduleLiterals,
    base,
    _redirectedReference,
    compilerOptions,
    containingSourceFile,
    _reusedNames,
  ) {
    const specifiers = moduleLiterals.map((literal) => [
      ts.getModeForUsageLocation(
        containingSourceFile,
        literal,
        compilerOptions,
      ) === ts.ModuleKind.CommonJS,
      literal.text,
    ]);
    if (logDebug) {
      debug(`host.resolveModuleNames()`);
      debug(`  base: ${base}`);
      debug(`  specifiers: ${specifiers.map((s) => s[1]).join(", ")}`);
    }
    /** @type {Array<[string, ts.Extension | null] | undefined>} */
    const resolved = ops.op_resolve(
      base,
      specifiers,
    );
    if (resolved) {
      /** @type {Array<ts.ResolvedModuleWithFailedLookupLocations>} */
      const result = resolved.map((item) => {
        if (item && item[1]) {
          const [resolvedFileName, extension] = item;
          return {
            resolvedModule: {
              resolvedFileName,
              extension,
              // todo(dsherret): we should probably be setting this
              isExternalLibraryImport: false,
            },
          };
        }
        return {
          resolvedModule: undefined,
        };
      });
      result.length = specifiers.length;
      return result;
    } else {
      return new Array(specifiers.length);
    }
  },
  createHash(data) {
    return ops.op_create_hash(data);
  },

  // LanguageServiceHost
  getCompilationSettings() {
    if (logDebug) {
      debug("host.getCompilationSettings()");
    }
    const lastRequestScope = LAST_REQUEST_SCOPE.get();
    return (lastRequestScope
      ? LANGUAGE_SERVICE_ENTRIES.byScope.get(lastRequestScope)
        ?.compilerOptions
      : null) ?? LANGUAGE_SERVICE_ENTRIES.unscoped.compilerOptions;
  },
  getScriptFileNames() {
    if (logDebug) {
      debug("host.getScriptFileNames()");
    }
    if (!SCRIPT_NAMES_CACHE) {
      const { unscoped, byScope } = ops.op_script_names();
      SCRIPT_NAMES_CACHE = {
        unscoped,
        byScope: new Map(Object.entries(byScope)),
      };
    }
    const lastRequestScope = LAST_REQUEST_SCOPE.get();
    return (lastRequestScope
      ? SCRIPT_NAMES_CACHE.byScope.get(lastRequestScope)
      : null) ?? SCRIPT_NAMES_CACHE.unscoped;
  },
  getScriptVersion(specifier) {
    if (logDebug) {
      debug(`host.getScriptVersion("${specifier}")`);
    }
    if (specifier.startsWith(ASSETS_URL_PREFIX)) {
      return "1";
    }
    // tsc requests the script version multiple times even though it can't
    // possibly have changed, so we will memoize it on a per request basis.
    if (SCRIPT_VERSION_CACHE.has(specifier)) {
      return SCRIPT_VERSION_CACHE.get(specifier);
    }
    const scriptVersion = ops.op_script_version(specifier);
    SCRIPT_VERSION_CACHE.set(specifier, scriptVersion);
    return scriptVersion;
  },
  getScriptSnapshot(specifier) {
    if (logDebug) {
      debug(`host.getScriptSnapshot("${specifier}")`);
    }
    if (specifier.startsWith(ASSETS_URL_PREFIX)) {
      const sourceFile = this.getSourceFile(
        specifier,
        ts.ScriptTarget.ESNext,
      );
      if (sourceFile) {
        if (!ASSET_SCOPES.has(specifier)) {
          ASSET_SCOPES.set(specifier, LAST_REQUEST_SCOPE.get());
        }
        // This case only occurs for assets.
        return ts.ScriptSnapshot.fromString(sourceFile.text);
      }
    }
    let scriptSnapshot = SCRIPT_SNAPSHOT_CACHE.get(specifier);
    if (scriptSnapshot == undefined) {
      /** @type {{ data: string, version: string, isCjs: boolean }} */
      const fileInfo = ops.op_load(specifier);
      if (!fileInfo) {
        return undefined;
      }
      scriptSnapshot = ts.ScriptSnapshot.fromString(fileInfo.data);
      scriptSnapshot.isCjs = fileInfo.isCjs;
      SCRIPT_SNAPSHOT_CACHE.set(specifier, scriptSnapshot);
      SCRIPT_VERSION_CACHE.set(specifier, fileInfo.version);
    }
    return scriptSnapshot;
  },
};

// @ts-ignore Undocumented function.
const exportMapCache = ts.createCacheableExportInfoMap(host);

// override the npm install @types package diagnostics to be deno specific
ts.setLocalizedDiagnosticMessages((() => {
  const nodeMessage = "Cannot find name '{0}'."; // don't offer any suggestions
  const jqueryMessage =
    "Cannot find name '{0}'. Did you mean to import jQuery? Try adding `import $ from \"npm:jquery\";`.";
  return {
    "Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_node_Try_npm_i_save_dev_types_Slashno_2580":
      nodeMessage,
    "Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_node_Try_npm_i_save_dev_types_Slashno_2591":
      nodeMessage,
    "Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_jQuery_Try_npm_i_save_dev_types_Slash_2581":
      jqueryMessage,
    "Cannot_find_name_0_Do_you_need_to_install_type_definitions_for_jQuery_Try_npm_i_save_dev_types_Slash_2592":
      jqueryMessage,
    "Module_0_was_resolved_to_1_but_allowArbitraryExtensions_is_not_set_6263":
      "Module '{0}' was resolved to '{1}', but importing these modules is not supported.",
  };
})());

/** @param {ts.Diagnostic} diagnostic */
export function filterMapDiagnostic(diagnostic) {
  if (IGNORED_DIAGNOSTICS.includes(diagnostic.code)) {
    return false;
  }

  // ignore diagnostics resulting from the `ImportMeta` declaration in deno merging with
  // the one in @types/node. the types of the filename and dirname properties are different,
  // which causes tsc to error.
  const importMetaFilenameDirnameModifiersRe =
    /^All declarations of '(filename|dirname)'/;
  const importMetaFilenameDirnameTypesRe =
    /^Subsequent property declarations must have the same type.\s+Property '(filename|dirname)'/;
  // Declarations of X must have identical modifiers.
  if (diagnostic.code === 2687) {
    if (
      typeof diagnostic.messageText === "string" &&
      (importMetaFilenameDirnameModifiersRe.test(diagnostic.messageText)) &&
      (diagnostic.file?.fileName.startsWith("asset:///") ||
        diagnostic.file?.fileName?.includes("@types/node"))
    ) {
      return false;
    }
  }
  // Subsequent property declarations must have the same type.
  if (diagnostic.code === 2717) {
    if (
      typeof diagnostic.messageText === "string" &&
      (importMetaFilenameDirnameTypesRe.test(diagnostic.messageText)) &&
      (diagnostic.file?.fileName.startsWith("asset:///") ||
        diagnostic.file?.fileName?.includes("@types/node"))
    ) {
      return false;
    }
  }
  // make the diagnostic for using an `export =` in an es module a warning
  if (diagnostic.code === 1203) {
    diagnostic.category = ts.DiagnosticCategory.Warning;
    if (typeof diagnostic.messageText === "string") {
      const message =
        " This will start erroring in a future version of Deno 2 " +
        "in order to align with TypeScript.";
      // seems typescript shares objects, so check if it's already been set
      if (!diagnostic.messageText.endsWith(message)) {
        diagnostic.messageText += message;
      }
    }
  }
  return true;
}

// list of globals that should be kept in Node's globalThis
ts.deno.setNodeOnlyGlobalNames([
  "__dirname",
  "__filename",
  "Buffer",
  "BufferConstructor",
  "BufferEncoding",
  "clearImmediate",
  "clearInterval",
  "clearTimeout",
  "console",
  "Console",
  "ErrorConstructor",
  "gc",
  "Global",
  "localStorage",
  "queueMicrotask",
  "RequestInit",
  "ResponseInit",
  "sessionStorage",
  "setImmediate",
  "setInterval",
  "setTimeout",
]);

export function getAssets() {
  /** @type {{ specifier: string; text: string; }[]} */
  const assets = [];
  for (const sourceFile of SOURCE_FILE_CACHE.values()) {
    if (sourceFile.fileName.startsWith(ASSETS_URL_PREFIX)) {
      assets.push({
        specifier: sourceFile.fileName,
        text: sourceFile.text,
      });
    }
  }
  return assets;
}
