// Copyright 2018-2025 the Deno authors. MIT license.

import {
  ASSET_SCOPES,
  ASSETS_URL_PREFIX,
  clearScriptNamesCache,
  debug,
  error,
  filterMapDiagnostic,
  fromTypeScriptDiagnostics,
  getCreateSourceFileOptions,
  host,
  IS_NODE_SOURCE_FILE_CACHE,
  LANGUAGE_SERVICE_ENTRIES,
  LAST_REQUEST_METHOD,
  LAST_REQUEST_NOTEBOOK_URI,
  LAST_REQUEST_SCOPE,
  OperationCanceledError,
  PROJECT_VERSION_CACHE,
  SCRIPT_SNAPSHOT_CACHE,
  SCRIPT_VERSION_CACHE,
  setLogDebug,
  SOURCE_REF_COUNTS,
} from "./97_ts_host.js";

/** @type {DenoCore} */
const core = globalThis.Deno.core;
const ops = core.ops;

/** @type {Map<string | null, string[]>} */
const ambientModulesCacheByScope = new Map();

const ChangeKind = {
  Opened: 0,
  Modified: 1,
  Closed: 2,
};

/**
 * @param {ts.CompilerOptions | ts.MinimalResolutionCacheHost} settingsOrHost
 * @returns {ts.CompilerOptions}
 */
function getCompilationSettings(settingsOrHost) {
  if (typeof settingsOrHost.getCompilationSettings === "function") {
    return settingsOrHost.getCompilationSettings();
  }
  return /** @type {ts.CompilerOptions} */ (settingsOrHost);
}

// We need to use a custom document registry in order to provide source files
// with an impliedNodeFormat to the ts language service

/** @type {Map<string, ts.SourceFile>} */
const documentRegistrySourceFileCache = new Map();
const { getKeyForCompilationSettings } = ts.createDocumentRegistry(); // reuse this code
/** @type {ts.DocumentRegistry} */
const documentRegistry = {
  acquireDocument(
    fileName,
    compilationSettingsOrHost,
    scriptSnapshot,
    version,
    scriptKind,
    sourceFileOptions,
  ) {
    const key = getKeyForCompilationSettings(
      getCompilationSettings(compilationSettingsOrHost),
    );
    return this.acquireDocumentWithKey(
      fileName,
      /** @type {ts.Path} */ (fileName),
      compilationSettingsOrHost,
      key,
      scriptSnapshot,
      version,
      scriptKind,
      sourceFileOptions,
    );
  },

  acquireDocumentWithKey(
    fileName,
    path,
    _compilationSettingsOrHost,
    key,
    scriptSnapshot,
    version,
    scriptKind,
    sourceFileOptions,
  ) {
    const mapKey = path + key;
    let sourceFile = documentRegistrySourceFileCache.get(mapKey);
    if (!sourceFile || sourceFile.version !== version) {
      const isCjs = /** @type {any} */ (scriptSnapshot).isCjs;
      sourceFile = ts.createLanguageServiceSourceFile(
        fileName,
        scriptSnapshot,
        {
          ...getCreateSourceFileOptions(sourceFileOptions),
          impliedNodeFormat: isCjs
            ? ts.ModuleKind.CommonJS
            : ts.ModuleKind.ESNext,
          // in the lsp we want to be able to show documentation
          jsDocParsingMode: ts.JSDocParsingMode.ParseAll,
        },
        version,
        true,
        scriptKind,
      );
      if (scriptSnapshot.isClassicScript) {
        sourceFile.externalModuleIndicator = undefined;
      }
      documentRegistrySourceFileCache.set(mapKey, sourceFile);
    }
    const sourceRefCount = SOURCE_REF_COUNTS.get(fileName) ?? 0;
    SOURCE_REF_COUNTS.set(fileName, sourceRefCount + 1);
    return sourceFile;
  },

  updateDocument(
    fileName,
    compilationSettingsOrHost,
    scriptSnapshot,
    version,
    scriptKind,
    sourceFileOptions,
  ) {
    const key = getKeyForCompilationSettings(
      getCompilationSettings(compilationSettingsOrHost),
    );
    return this.updateDocumentWithKey(
      fileName,
      /** @type {ts.Path} */ (fileName),
      compilationSettingsOrHost,
      key,
      scriptSnapshot,
      version,
      scriptKind,
      sourceFileOptions,
    );
  },

  updateDocumentWithKey(
    fileName,
    path,
    compilationSettingsOrHost,
    key,
    scriptSnapshot,
    version,
    scriptKind,
    sourceFileOptions,
  ) {
    const mapKey = path + key;
    let sourceFile = documentRegistrySourceFileCache.get(mapKey) ??
      this.acquireDocumentWithKey(
        fileName,
        path,
        compilationSettingsOrHost,
        key,
        scriptSnapshot,
        version,
        scriptKind,
        sourceFileOptions,
      );

    if (sourceFile.version !== version) {
      sourceFile = ts.updateLanguageServiceSourceFile(
        sourceFile,
        scriptSnapshot,
        version,
        scriptSnapshot.getChangeRange(
          /** @type {ts.IScriptSnapshot} */ (sourceFile.scriptSnapShot),
        ),
      );
      if (scriptSnapshot.isClassicScript) {
        sourceFile.externalModuleIndicator = undefined;
      }
      documentRegistrySourceFileCache.set(mapKey, sourceFile);
    }
    return sourceFile;
  },

  getKeyForCompilationSettings(settings) {
    return getKeyForCompilationSettings(settings);
  },

  releaseDocument(
    fileName,
    compilationSettings,
    scriptKind,
    impliedNodeFormat,
  ) {
    const key = getKeyForCompilationSettings(compilationSettings);
    return this.releaseDocumentWithKey(
      /** @type {ts.Path} */ (fileName),
      key,
      scriptKind,
      impliedNodeFormat,
    );
  },

  releaseDocumentWithKey(path, key, _scriptKind, _impliedNodeFormat) {
    const sourceRefCount = SOURCE_REF_COUNTS.get(path) ?? 1;
    if (sourceRefCount <= 1) {
      SOURCE_REF_COUNTS.delete(path);
      // We call `cleanupSemanticCache` for other purposes, don't bust the
      // source cache in this case.
      if (LAST_REQUEST_METHOD.get() != "$cleanupSemanticCache") {
        const mapKey = path + key;
        documentRegistrySourceFileCache.delete(mapKey);
        SCRIPT_SNAPSHOT_CACHE.delete(path);
        ops.op_release(path);
      }
    } else {
      SOURCE_REF_COUNTS.set(path, sourceRefCount - 1);
    }
  },

  reportStats() {
    return "[]";
  },
};

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

/** @param {Record<string, unknown>} config */
function lspTsConfigToCompilerOptions(config) {
  const normalizedConfig = normalizeConfig(config);
  const { options, errors } = ts
    .convertCompilerOptionsFromJson(normalizedConfig, "");
  Object.assign(options, {
    allowNonTsExtensions: true,
    allowImportingTsExtensions: true,
    module: ts.ModuleKind.NodeNext,
    moduleResolution: ts.ModuleResolutionKind.NodeNext,
  });
  if (errors.length > 0) {
    debug(ts.formatDiagnostics(errors, host));
  }
  return options;
}

/**
 * @param {any} e
 * @returns {e is (OperationCanceledError | ts.OperationCanceledException)}
 */
function isCancellationError(e) {
  return e instanceof OperationCanceledError ||
    e instanceof ts.OperationCanceledException;
}

/**
 * @param {number} _id
 * @param {any} data
 * @param {string | null} error
 */
// TODO(bartlomieju): this feels needlessly generic, both type checking
// and language server use it with inefficient serialization. Id is not used
// anyway...
function respond(_id, data = null, error = null) {
  if (error) {
    ops.op_respond(
      "error",
      error,
    );
  } else {
    ops.op_respond(JSON.stringify(data), "");
  }
}

/** @typedef {[[string, number][], number, [string, any][], [string, string][]] } PendingChange */
/**
 * @template T
 * @typedef {T | null} Option<T> */

/** @returns {Promise<[number, string, any[], string | null, Option<PendingChange>] | null>} */
async function pollRequests() {
  return await ops.op_poll_requests();
}

let hasStarted = false;

function createLs() {
  let exportInfoMap = undefined;
  const newHost = {
    ...host,
    getCachedExportInfoMap: () => {
      // this export info map is specific to
      // the language service instance
      return exportInfoMap;
    },
  };
  const ls = ts.createLanguageService(
    newHost,
    documentRegistry,
  );
  exportInfoMap = ts.createCacheableExportInfoMap({
    getCurrentProgram() {
      return ls.getProgram();
    },
    getGlobalTypingsCacheLocation() {
      return undefined;
    },
    getPackageJsonAutoImportProvider() {
      return undefined;
    },
  });
  return ls;
}

/** @param {boolean} enableDebugLogging */
export async function serverMainLoop(enableDebugLogging) {
  ts.deno.setEnterSpan(ops.op_make_span);
  ts.deno.setExitSpan(ops.op_exit_span);
  if (hasStarted) {
    throw new Error("The language server has already been initialized.");
  }
  hasStarted = true;
  LANGUAGE_SERVICE_ENTRIES.unscoped = {
    ls: createLs(),
    compilerOptions: lspTsConfigToCompilerOptions({
      "allowJs": true,
      "esModuleInterop": true,
      "experimentalDecorators": false,
      "isolatedModules": true,
      "lib": ["deno.ns", "deno.window", "deno.unstable"],
      "module": "NodeNext",
      "moduleResolution": "NodeNext",
      "moduleDetection": "force",
      "noEmit": true,
      "noImplicitOverride": true,
      "resolveJsonModule": true,
      "strict": true,
      "target": "esnext",
      "useDefineForClassFields": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    }),
  };
  setLogDebug(enableDebugLogging, "TSLS");
  debug("serverInit()");

  while (true) {
    const request = await pollRequests();
    if (request === null) {
      break;
    }
    try {
      serverRequest(
        request[0],
        request[1],
        request[2],
        request[3],
        request[4],
        request[5],
      );
    } catch (err) {
      error(`Internal error occurred processing request: ${err}`);
    }
  }
}

/**
 * @param {any} error
 * @param {any[] | null} args
 */
function formatErrorWithArgs(error, args) {
  let errorString = "stack" in error
    ? error.stack.toString()
    : error.toString();
  if (args) {
    errorString += `\nFor request: [${
      args.map((v) => JSON.stringify(v)).join(", ")
    }]`;
  }
  return errorString;
}

/**
 * @param {string[]} a
 * @param {string[]} b
 */
function arraysEqual(a, b) {
  if (a === b) {
    return true;
  }
  if (a === null || b === null) {
    return false;
  }
  if (a.length !== b.length) {
    return false;
  }
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}

/**
 * @param {number} id
 * @param {string} method
 * @param {any[]} args
 * @param {string | null} scope
 * @param {string | null} notebookUri
 * @param {PendingChange | null} maybeChange
 */
function serverRequestInner(id, method, args, scope, notebookUri, maybeChange) {
  debug(`serverRequest()`, id, method, args, scope, notebookUri, maybeChange);
  if (maybeChange !== null) {
    const changedScripts = maybeChange[0];
    const newProjectVersion = maybeChange[1];
    const newConfigsByScope = maybeChange[2];
    const newNotebookScopes = maybeChange[3];
    if (newConfigsByScope) {
      IS_NODE_SOURCE_FILE_CACHE.clear();
      ASSET_SCOPES.clear();
      /** @type { typeof LANGUAGE_SERVICE_ENTRIES.byScope } */
      const newByScope = new Map();
      for (const [scope, config] of newConfigsByScope) {
        LAST_REQUEST_SCOPE.set(scope);
        LAST_REQUEST_NOTEBOOK_URI.set(null);
        const oldEntry = LANGUAGE_SERVICE_ENTRIES.byScope.get(scope);
        const ls = oldEntry ? oldEntry.ls : createLs();
        const compilerOptions = lspTsConfigToCompilerOptions(config);
        newByScope.set(scope, { ls, compilerOptions });
        LANGUAGE_SERVICE_ENTRIES.byScope.delete(scope);
      }
      for (const oldEntry of LANGUAGE_SERVICE_ENTRIES.byScope.values()) {
        oldEntry.ls.dispose();
      }
      LANGUAGE_SERVICE_ENTRIES.byScope = newByScope;
    }
    if (newNotebookScopes) {
      /** @type { typeof LANGUAGE_SERVICE_ENTRIES.byNotebookUri } */
      const newByNotebookUri = new Map();
      for (const [notebookUri, scope] of newNotebookScopes) {
        LAST_REQUEST_SCOPE.set(scope);
        LAST_REQUEST_NOTEBOOK_URI.set(notebookUri);
        const oldEntry = LANGUAGE_SERVICE_ENTRIES.byNotebookUri.get(
          notebookUri,
        );
        const ls = oldEntry ? oldEntry.ls : createLs();
        const compilerOptions =
          LANGUAGE_SERVICE_ENTRIES.byScope.get(scope)?.compilerOptions ??
            LANGUAGE_SERVICE_ENTRIES.unscoped.compilerOptions;
        newByNotebookUri.set(notebookUri, { ls, compilerOptions });
        LANGUAGE_SERVICE_ENTRIES.byNotebookUri.delete(notebookUri);
      }
      for (const oldEntry of LANGUAGE_SERVICE_ENTRIES.byNotebookUri.values()) {
        oldEntry.ls.dispose();
      }
      LANGUAGE_SERVICE_ENTRIES.byNotebookUri = newByNotebookUri;
    }

    PROJECT_VERSION_CACHE.set(newProjectVersion);

    let opened = false;
    let closed = false;
    for (const { 0: script, 1: changeKind } of changedScripts) {
      if (changeKind === ChangeKind.Opened) {
        opened = true;
      } else if (changeKind === ChangeKind.Closed) {
        closed = true;
      }
      SCRIPT_VERSION_CACHE.delete(script);
      SCRIPT_SNAPSHOT_CACHE.delete(script);
    }

    if (newConfigsByScope || newNotebookScopes || opened || closed) {
      clearScriptNamesCache();
    }
  }

  // For requests pertaining to an asset document, we make it so that the
  // passed scope is just its own specifier. We map it to an actual scope here
  // based on the first scope that the asset was loaded into.
  if (scope?.startsWith(ASSETS_URL_PREFIX)) {
    scope = ASSET_SCOPES.get(scope) ?? null;
  }
  LAST_REQUEST_METHOD.set(method);
  LAST_REQUEST_SCOPE.set(scope);
  LAST_REQUEST_NOTEBOOK_URI.set(notebookUri);
  const ls =
    (notebookUri
      ? LANGUAGE_SERVICE_ENTRIES.byNotebookUri.get(notebookUri)?.ls
      : null) ??
      (scope ? LANGUAGE_SERVICE_ENTRIES.byScope.get(scope)?.ls : null) ??
      LANGUAGE_SERVICE_ENTRIES.unscoped.ls;
  switch (method) {
    case "$cleanupSemanticCache": {
      for (
        const ls of [
          LANGUAGE_SERVICE_ENTRIES.unscoped.ls,
          ...[...LANGUAGE_SERVICE_ENTRIES.byScope.values()].map((e) => e.ls),
          ...[...LANGUAGE_SERVICE_ENTRIES.byNotebookUri.values()].map((e) =>
            e.ls
          ),
        ]
      ) {
        ls.cleanupSemanticCache();
      }
      return respond(id, null);
    }
    case "$getSupportedCodeFixes": {
      return respond(
        id,
        ts.getSupportedCodeFixes(),
      );
    }
    case "$getDiagnostics": {
      const projectVersion = args[1];
      // there's a possibility that we receive a change notification
      // but the diagnostic server queues a `$getDiagnostics` request
      // with a stale project version. in that case, treat it as cancelled
      // (it's about to be invalidated anyway).
      const cachedProjectVersion = PROJECT_VERSION_CACHE.get();
      if (cachedProjectVersion && projectVersion !== cachedProjectVersion) {
        return respond(id, [[], null]);
      }
      try {
        /** @type {any[][]} */
        const diagnosticsList = [];
        for (const specifier of args[0]) {
          diagnosticsList.push(fromTypeScriptDiagnostics([
            ...ls.getSemanticDiagnostics(specifier),
            ...ls.getSuggestionDiagnostics(specifier),
            ...ls.getSyntacticDiagnostics(specifier),
          ].filter(filterMapDiagnostic)));
        }
        let ambient =
          ls.getProgram()?.getTypeChecker().getAmbientModules().map((symbol) =>
            symbol.getName()
          ) ?? [];
        const previousAmbient = ambientModulesCacheByScope.get(scope);
        if (
          ambient && previousAmbient && arraysEqual(ambient, previousAmbient)
        ) {
          ambient = null; // null => use previous value
        } else {
          ambientModulesCacheByScope.set(scope, ambient);
        }
        return respond(id, [diagnosticsList, ambient]);
      } catch (e) {
        if (
          !isCancellationError(e)
        ) {
          return respond(
            id,
            [[], null],
            formatErrorWithArgs(e, [
              id,
              method,
              args,
              scope,
              notebookUri,
              maybeChange,
            ]),
          );
        }
        return respond(id, [[], null]);
      }
    }
    default:
      if (typeof ls[method] === "function") {
        // The `getCompletionEntryDetails()` method returns null if the
        // `source` is `null` for whatever reason. It must be `undefined`.
        if (method == "getCompletionEntryDetails") {
          args[4] ??= undefined;
        }
        try {
          return respond(id, ls[method](...args));
        } catch (e) {
          if (!isCancellationError(e)) {
            return respond(
              id,
              null,
              formatErrorWithArgs(e, [
                id,
                method,
                args,
                scope,
                notebookUri,
                maybeChange,
              ]),
            );
          }
          return respond(id);
        }
      }
      throw new TypeError(
        // @ts-ignore exhausted case statement sets type to never
        `Invalid request method for request: "${method}" (${id})`,
      );
  }
}

/**
 * @param {number} id
 * @param {string} method
 * @param {any[]} args
 * @param {string | null} scope
 * @param {string | null} notebookUri
 * @param {PendingChange | null} maybeChange
 */
function serverRequest(id, method, args, scope, notebookUri, maybeChange) {
  const span = ops.op_make_span(`serverRequest(${method})`, true);
  try {
    serverRequestInner(id, method, args, scope, notebookUri, maybeChange);
  } finally {
    ops.op_exit_span(span, true);
  }
}
