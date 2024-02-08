// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="./compiler.d.ts" />
// deno-lint-ignore-file no-undef

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to type check TypeScript, and in some
// instances convert TypeScript to JavaScript.

// Removes the `__proto__` for security reasons.
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
delete Object.prototype.__proto__;

((/** @type {any} */ window) => {
  /** @type {DenoCore} */
  const core = window.Deno.core;
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
    "CreateHttpClientOptions",
    "DatagramConn",
    "HttpClient",
    "Kv",
    "KvListIterator",
    "KvU64",
    "UnsafeCallback",
    "UnsafePointer",
    "UnsafePointerView",
    "UnsafeFnPointer",
    "UnixConnectOptions",
    "UnixListenOptions",
    "createHttpClient",
    "dlopen",
    "flock",
    "flockSync",
    "funlock",
    "funlockSync",
    "listen",
    "listenDatagram",
    "openKv",
    "umask",
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
  function getCreateSourceFileOptions(versionOrOptions) {
    return isCreateSourceFileOptions(versionOrOptions)
      ? versionOrOptions
      : { languageVersion: versionOrOptions ?? ts.ScriptTarget.ESNext };
  }

  /**
   * @param debug {boolean}
   * @param source {string}
   */
  function setLogDebug(debug, source) {
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
  function debug(...args) {
    if (logDebug) {
      const stringifiedArgs = args.map((arg) =>
        typeof arg === "string" ? arg : JSON.stringify(arg)
      ).join(" ");
      printStderr(`DEBUG ${logSource} - ${stringifiedArgs}\n`);
    }
  }

  /** @param args {any[]} */
  function error(...args) {
    const stringifiedArgs = args.map((arg) =>
      typeof arg === "string" || arg instanceof Error
        ? String(arg)
        : JSON.stringify(arg)
    ).join(" ");
    printStderr(`ERROR ${logSource} = ${stringifiedArgs}\n`);
  }

  class AssertionError extends Error {
    /** @param msg {string} */
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  /** @param cond {boolean} */
  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
    }
  }

  class SpecifierIsCjsCache {
    /** @type {Set<string>} */
    #cache = new Set();

    /** @param {[string, ts.Extension]} param */
    add([specifier, ext]) {
      if (ext === ".cjs" || ext === ".d.cts" || ext === ".cts") {
        this.#cache.add(specifier);
      }
    }

    /** @param specifier {string} */
    has(specifier) {
      return this.#cache.has(specifier);
    }
  }

  // In the case of the LSP, this will only ever contain the assets.
  /** @type {Map<string, ts.SourceFile>} */
  const sourceFileCache = new Map();

  /** @type {string[]=} */
  let scriptFileNamesCache;

  /** @type {Map<string, string>} */
  const scriptVersionCache = new Map();

  /** @type {Map<string, boolean>} */
  const isNodeSourceFileCache = new Map();

  const isCjsCache = new SpecifierIsCjsCache();

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
        sourceFile = ts.createLanguageServiceSourceFile(
          fileName,
          scriptSnapshot,
          {
            ...getCreateSourceFileOptions(sourceFileOptions),
            impliedNodeFormat: isCjsCache.has(fileName)
              ? ts.ModuleKind.CommonJS
              : ts.ModuleKind.ESNext,
            // in the lsp we want to be able to show documentation
            jsDocParsingMode: ts.JSDocParsingMode.ParseAll,
          },
          version,
          true,
          scriptKind,
        );
        documentRegistrySourceFileCache.set(mapKey, sourceFile);
      }
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
      const mapKey = path + key;
      documentRegistrySourceFileCache.delete(mapKey);
    },

    reportStats() {
      return "[]";
    },
  };

  ts.deno.setIsNodeSourceFileCallback((sourceFile) => {
    const fileName = sourceFile.fileName;
    let isNodeSourceFile = isNodeSourceFileCache.get(fileName);
    if (isNodeSourceFile == null) {
      const result = ops.op_is_node_file(fileName);
      isNodeSourceFile = /** @type {boolean} */ (result);
      isNodeSourceFileCache.set(fileName, isNodeSourceFile);
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
          return `${msg} 'Deno.${property}' is an unstable API. Did you forget to run with the '--unstable' flag? ${unstableMsgSuggestion}`;
        }
        return msg;
      }
      default: {
        const property = getProperty();
        if (property && unstableDenoProps.has(property)) {
          const suggestion = getMsgSuggestion();
          if (suggestion) {
            return `${msg} 'Deno.${property}' is an unstable API. Did you forget to run with the '--unstable' flag, or did you mean '${suggestion}'? ${unstableMsgSuggestion}`;
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
      const startPos = file.getLineAndCharacterOfPosition(start);
      const sourceLine = file.getFullText().split("\n")[startPos.line];
      const fileName = file.fileName;
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
  function fromTypeScriptDiagnostics(diagnostics) {
    return diagnostics.map(({ relatedInformation: ri, source, ...diag }) => {
      /** @type {any} */
      const value = fromRelatedInformation(diag);
      value.relatedInformation = ri
        ? ri.map(fromRelatedInformation)
        : undefined;
      value.source = source;
      return value;
    });
  }

  // Using incremental compile APIs requires that all
  // paths must be either relative or absolute. Since
  // analysis in Rust operates on fully resolved URLs,
  // it makes sense to use the same scheme here.
  const ASSETS_URL_PREFIX = "asset:///";
  const CACHE_URL_PREFIX = "cache:///";

  /** Diagnostics that are intentionally ignored when compiling TypeScript in
   * Deno, as they provide misleading or incorrect information. */
  const IGNORED_DIAGNOSTICS = [
    // TS1452: 'resolution-mode' assertions are only supported when `moduleResolution` is `node16` or `nodenext`.
    // We specify the resolution mode to be CommonJS for some npm files and this
    // diagnostic gets generated even though we're using custom module resolution.
    1452,
    // TS2306: File '.../index.d.ts' is not a module.
    // We get this for `x-typescript-types` declaration files which don't export
    // anything. We prefer to treat these as modules with no exports.
    2306,
    // TS2688: Cannot find type definition file for '...'.
    // We ignore because type defintion files can end with '.ts'.
    2688,
    // TS2792: Cannot find module. Did you mean to set the 'moduleResolution'
    // option to 'node', or to add aliases to the 'paths' option?
    2792,
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
    // TS7016: Could not find a declaration file for module '...'. '...'
    // implicitly has an 'any' type.  This is due to `allowJs` being off by
    // default but importing of a JavaScript module.
    7016,
  ];

  const SNAPSHOT_COMPILE_OPTIONS = {
    esModuleInterop: true,
    jsx: ts.JsxEmit.React,
    module: ts.ModuleKind.ESNext,
    noEmit: true,
    strict: true,
    target: ts.ScriptTarget.ESNext,
    lib: ["lib.deno.window.d.ts"],
  };

  // todo(dsherret): can we remove this and just use ts.OperationCanceledException?
  /** Error thrown on cancellation. */
  class OperationCanceledError extends Error {
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

  /** @type {ts.CompilerOptions} */
  let compilationSettings = {};

  /** @type {ts.LanguageService} */
  let languageService;

  /** An object literal of the incremental compiler host, which provides the
   * specific "bindings" to the Deno environment that tsc needs to work.
   *
   * @type {ts.CompilerHost & ts.LanguageServiceHost} */
  const host = {
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
      return ops.op_project_version();
    },
    // @ts-ignore Undocumented method.
    getModuleSpecifierCache() {
      return moduleSpecifierCache;
    },
    // @ts-ignore Undocumented method.
    getCachedExportInfoMap() {
      return exportMapCache;
    },
    getGlobalTypingsCacheLocation() {
      return undefined;
    },
    getSourceFile(
      specifier,
      languageVersion,
      _onError,
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

      let sourceFile = sourceFileCache.get(specifier);
      if (sourceFile) {
        return sourceFile;
      }

      /** @type {{ data: string; scriptKind: ts.ScriptKind; version: string; }} */
      const fileInfo = ops.op_load(specifier);
      if (!fileInfo) {
        return undefined;
      }
      const { data, scriptKind, version } = fileInfo;
      assert(
        data != null,
        `"data" is unexpectedly null for "${specifier}".`,
      );
      sourceFile = ts.createSourceFile(
        specifier,
        data,
        {
          ...getCreateSourceFileOptions(languageVersion),
          impliedNodeFormat: isCjsCache.has(specifier)
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
      sourceFileCache.set(specifier, sourceFile);
      scriptVersionCache.set(specifier, version);
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
        { fileName, data },
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
    resolveTypeReferenceDirectives(
      typeDirectiveNames,
      containingFilePath,
      redirectedReference,
      options,
      containingFileMode,
    ) {
      return typeDirectiveNames.map((arg) => {
        /** @type {ts.FileReference} */
        const fileReference = typeof arg === "string"
          ? {
            pos: -1,
            end: -1,
            fileName: arg,
          }
          : arg;
        if (fileReference.fileName.startsWith("npm:")) {
          /** @type {[string, ts.Extension] | undefined} */
          const resolved = ops.op_resolve({
            specifiers: [fileReference.fileName],
            base: containingFilePath,
          })?.[0];
          if (resolved) {
            isCjsCache.add(resolved);
            return {
              primary: true,
              resolvedFileName: resolved[0],
            };
          } else {
            return undefined;
          }
        } else {
          return ts.resolveTypeReferenceDirective(
            fileReference.fileName,
            containingFilePath,
            options,
            host,
            redirectedReference,
            undefined,
            containingFileMode ?? fileReference.resolutionMode,
          ).resolvedTypeReferenceDirective;
        }
      });
    },
    resolveModuleNames(specifiers, base) {
      if (logDebug) {
        debug(`host.resolveModuleNames()`);
        debug(`  base: ${base}`);
        debug(`  specifiers: ${specifiers.join(", ")}`);
      }
      /** @type {Array<[string, ts.Extension] | undefined>} */
      const resolved = ops.op_resolve({
        specifiers,
        base,
      });
      if (resolved) {
        const result = resolved.map((item) => {
          if (item) {
            isCjsCache.add(item);
            const [resolvedFileName, extension] = item;
            if (resolvedFileName.startsWith("node:")) {
              // probably means the user doesn't have @types/node, so resolve to undefined
              return undefined;
            }
            return {
              resolvedFileName,
              extension,
              isExternalLibraryImport: false,
            };
          }
          return undefined;
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
      return compilationSettings;
    },
    getScriptFileNames() {
      if (logDebug) {
        debug("host.getScriptFileNames()");
      }
      // tsc requests the script file names multiple times even though it can't
      // possibly have changed, so we will memoize it on a per request basis.
      if (scriptFileNamesCache) {
        return scriptFileNamesCache;
      }
      return scriptFileNamesCache = ops.op_script_names();
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
      if (scriptVersionCache.has(specifier)) {
        return scriptVersionCache.get(specifier);
      }
      const scriptVersion = ops.op_script_version(specifier);
      scriptVersionCache.set(specifier, scriptVersion);
      return scriptVersion;
    },
    getScriptSnapshot(specifier) {
      if (logDebug) {
        debug(`host.getScriptSnapshot("${specifier}")`);
      }
      let sourceFile = sourceFileCache.get(specifier);
      if (
        !specifier.startsWith(ASSETS_URL_PREFIX) &&
        sourceFile?.version != this.getScriptVersion(specifier)
      ) {
        sourceFileCache.delete(specifier);
        sourceFile = undefined;
      }
      if (!sourceFile) {
        sourceFile = this.getSourceFile(
          specifier,
          specifier.endsWith(".json")
            ? ts.ScriptTarget.JSON
            : ts.ScriptTarget.ESNext,
        );
      }
      if (sourceFile) {
        return ts.ScriptSnapshot.fromString(sourceFile.text);
      }
      return undefined;
    },
  };

  // @ts-ignore Undocumented function.
  const moduleSpecifierCache = ts.server.createModuleSpecifierCache(host);

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
    };
  })());

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

  /**
   * Checks the normalized version of the root name and stores it in
   * `normalizedToOriginalMap`. If the normalized specifier is already
   * registered for the different root name, it throws an AssertionError.
   *
   * @param {string} rootName
   */
  function checkNormalizedPath(rootName) {
    const normalized = ts.normalizePath(rootName);
    const originalRootName = normalizedToOriginalMap.get(normalized);
    if (typeof originalRootName === "undefined") {
      normalizedToOriginalMap.set(normalized, rootName);
    } else if (originalRootName !== rootName) {
      // The different root names are normalizd to the same path.
      // This will cause problem when looking up the source for each.
      throw new AssertionError(
        `The different names for the same normalized specifier are specified: normalized=${normalized}, rootNames=${originalRootName},${rootName}`,
      );
    }
  }

  /** @param {Record<string, string>} config */
  function normalizeConfig(config) {
    // the typescript compiler doesn't know about the precompile
    // transform at the moment, so just tell it we're using react-jsx
    if (config.jsx === "precompile") {
      config.jsx = "react-jsx";
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

    if (logDebug) {
      debug(">>> exec start", { rootNames });
      debug(config);
    }

    rootNames.forEach(checkNormalizedPath);

    const { options, errors: configFileParsingDiagnostics } = ts
      .convertCompilerOptionsFromJson(config, "");
    // The `allowNonTsExtensions` is a "hidden" compiler option used in VSCode
    // which is not allowed to be passed in JSON, we need it to allow special
    // URLs which Deno supports. So we need to either ignore the diagnostic, or
    // inject it ourselves.
    Object.assign(options, { allowNonTsExtensions: true });
    const program = ts.createIncrementalProgram({
      rootNames,
      options,
      host,
      configFileParsingDiagnostics,
    });

    const checkFiles = localOnly
      ? rootNames
        .filter((n) => !n.startsWith("http"))
        .map((checkName) => {
          const sourceFile = program.getSourceFile(checkName);
          if (sourceFile == null) {
            throw new Error("Could not find source file for: " + checkName);
          }
          return sourceFile;
        })
      : undefined;

    if (checkFiles != null) {
      // When calling program.getSemanticDiagnostics(...) with a source file, we
      // need to call this code first in order to get it to invalidate cached
      // diagnostics correctly. This is what program.getSemanticDiagnostics()
      // does internally when calling without any arguments.
      const checkFileNames = new Set(checkFiles.map((f) => f.fileName));
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
    ].filter((diagnostic) => !IGNORED_DIAGNOSTICS.includes(diagnostic.code));

    // emit the tsbuildinfo file
    // @ts-ignore: emitBuildInfo is not exposed (https://github.com/microsoft/TypeScript/issues/49871)
    program.emitBuildInfo(host.writeFile);

    performanceProgram({ program });

    ops.op_respond({
      diagnostics: fromTypeScriptDiagnostics(diagnostics),
      stats: performanceEnd(),
    });
    debug("<<< exec stop");
  }

  function getAssets() {
    /** @type {{ specifier: string; text: string; }[]} */
    const assets = [];
    for (const sourceFile of sourceFileCache.values()) {
      if (sourceFile.fileName.startsWith(ASSETS_URL_PREFIX)) {
        assets.push({
          specifier: sourceFile.fileName,
          text: sourceFile.text,
        });
      }
    }
    return assets;
  }

  /**
   * @param {number} id
   * @param {any} data
   */
  // TODO(bartlomieju): this feels needlessly generic, both type chcking
  // and language server use it with inefficient serialization. Id is not used
  // anyway...
  function respond(id, data = null) {
    ops.op_respond({ id, data });
  }

  function serverRequest(id, method, args) {
    if (logDebug) {
      debug(`serverRequest()`, id, method, args);
    }

    // reset all memoized source files names
    scriptFileNamesCache = undefined;
    // evict all memoized source file versions
    scriptVersionCache.clear();
    switch (method) {
      case "$restart": {
        serverRestart();
        return respond(id, true);
      }
      case "$configure": {
        const config = normalizeConfig(args[0]);
        const { options, errors } = ts
          .convertCompilerOptionsFromJson(config, "");
        Object.assign(options, {
          allowNonTsExtensions: true,
          allowImportingTsExtensions: true,
        });
        if (errors.length > 0 && logDebug) {
          debug(ts.formatDiagnostics(errors, host));
        }
        compilationSettings = options;
        moduleSpecifierCache.clear();
        return respond(id, true);
      }
      case "$getSupportedCodeFixes": {
        return respond(
          id,
          ts.getSupportedCodeFixes(),
        );
      }
      case "$getAssets": {
        return respond(id, getAssets());
      }
      case "$getDiagnostics": {
        try {
          /** @type {Record<string, any[]>} */
          const diagnosticMap = {};
          for (const specifier of args[0]) {
            diagnosticMap[specifier] = fromTypeScriptDiagnostics([
              ...languageService.getSemanticDiagnostics(specifier),
              ...languageService.getSuggestionDiagnostics(specifier),
              ...languageService.getSyntacticDiagnostics(specifier),
            ].filter(({ code }) => !IGNORED_DIAGNOSTICS.includes(code)));
          }
          return respond(id, diagnosticMap);
        } catch (e) {
          if (
            !(e instanceof OperationCanceledError ||
              e instanceof ts.OperationCanceledException)
          ) {
            if ("stack" in e) {
              error(e.stack);
            } else {
              error(e);
            }
          }
          return respond(id, {});
        }
      }
      default:
        if (typeof languageService[method] === "function") {
          // The `getCompletionEntryDetails()` method returns null if the
          // `source` is `null` for whatever reason. It must be `undefined`.
          if (method == "getCompletionEntryDetails") {
            args[4] ??= undefined;
          }
          return respond(id, languageService[method](...args));
        }
        throw new TypeError(
          // @ts-ignore exhausted case statement sets type to never
          `Invalid request method for request: "${method}" (${id})`,
        );
    }
  }

  let hasStarted = false;
  /** @param {{ debug: boolean; }} init */
  function serverInit({ debug: debugFlag }) {
    if (hasStarted) {
      throw new Error("The language server has already been initialized.");
    }
    hasStarted = true;
    languageService = ts.createLanguageService(host, documentRegistry);
    setLogDebug(debugFlag, "TSLS");
    debug("serverInit()");
  }

  function serverRestart() {
    languageService = ts.createLanguageService(host, documentRegistry);
    isNodeSourceFileCache.clear();
    debug("serverRestart()");
  }

  // A build time only op that provides some setup information that is used to
  // ensure the snapshot is setup properly.
  /** @type {{ buildSpecifier: string; libs: string[]; nodeBuiltInModuleNames: string[] }} */
  const { buildSpecifier, libs, nodeBuiltInModuleNames } = ops.op_build_info();

  ts.deno.setNodeBuiltInModuleNames(nodeBuiltInModuleNames);

  // list of globals that should be kept in Node's globalThis
  ts.deno.setNodeOnlyGlobalNames([
    // when bumping the @types/node version we should check if
    // anything needs to be updated here
    "NodeRequire",
    "RequireResolve",
    "RequireResolve",
    "process",
    "console",
    "__filename",
    "__dirname",
    "require",
    "module",
    "exports",
    "gc",
    "BufferEncoding",
    "BufferConstructor",
    "WithImplicitCoercion",
    "Buffer",
    "Console",
    "ImportMeta",
    "setTimeout",
    "setInterval",
    "setImmediate",
    "Global",
    "AbortController",
    "AbortSignal",
    "Blob",
    "BroadcastChannel",
    "MessageChannel",
    "MessagePort",
    "Event",
    "EventTarget",
    "performance",
    "TextDecoder",
    "TextEncoder",
    "URL",
    "URLSearchParams",
  ]);

  for (const lib of libs) {
    const specifier = `lib.${lib}.d.ts`;
    // we are using internal APIs here to "inject" our custom libraries into
    // tsc, so things like `"lib": [ "deno.ns" ]` are supported.
    if (!ts.libs.includes(lib)) {
      ts.libs.push(lib);
      ts.libMap.set(lib, `lib.${lib}.d.ts`);
    }
    // we are caching in memory common type libraries that will be re-used by
    // tsc on when the snapshot is restored
    assert(
      !!host.getSourceFile(
        `${ASSETS_URL_PREFIX}${specifier}`,
        ts.ScriptTarget.ESNext,
      ),
      `failed to load '${ASSETS_URL_PREFIX}${specifier}'`,
    );
  }
  // this helps ensure as much as possible is in memory that is re-usable
  // before the snapshotting is done, which helps unsure fast "startup" for
  // subsequent uses of tsc in Deno.
  const TS_SNAPSHOT_PROGRAM = ts.createProgram({
    rootNames: [buildSpecifier],
    options: SNAPSHOT_COMPILE_OPTIONS,
    host,
  });
  assert(
    ts.getPreEmitDiagnostics(TS_SNAPSHOT_PROGRAM).length === 0,
    "lib.d.ts files have errors",
  );

  // remove this now that we don't need it anymore for warming up tsc
  sourceFileCache.delete(buildSpecifier);

  // exposes the functions that are called by `tsc::exec()` when type
  // checking TypeScript.
  /** @type {any} */
  const global = globalThis;
  global.exec = exec;
  global.getAssets = getAssets;

  // exposes the functions that are called when the compiler is used as a
  // language service.
  global.serverInit = serverInit;
  global.serverRequest = serverRequest;
})(this);
