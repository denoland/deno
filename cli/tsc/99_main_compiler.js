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
    "DatagramConn",
    "Kv",
    "KvListIterator",
    "KvU64",
    "UnixConnectOptions",
    "UnixListenOptions",
    "listen",
    "listenDatagram",
    "openKv",
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

  // In the case of the LSP, this will only ever contain the assets.
  /** @type {Map<string, ts.SourceFile>} */
  const sourceFileCache = new Map();

  /** @type {Map<string, string>} */
  const sourceTextCache = new Map();

  /** @type {Map<string, number>} */
  const sourceRefCounts = new Map();

  /** @type {Map<string, string>} */
  const scriptVersionCache = new Map();

  /** @type {Map<string, boolean>} */
  const isNodeSourceFileCache = new Map();

  /** @type {Map<string, boolean>} */
  const isCjsCache = new Map();

  // Maps asset specifiers to the first scope that the asset was loaded into.
  /** @type {Map<string, string | null>} */
  const assetScopes = new Map();

  /** @type {number | null} */
  let projectVersionCache = null;

  /** @type {string | null} */
  let lastRequestMethod = null;

  /** @type {string | null} */
  let lastRequestScope = null;

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
        sourceFile = ts.createLanguageServiceSourceFile(
          fileName,
          scriptSnapshot,
          {
            ...getCreateSourceFileOptions(sourceFileOptions),
            impliedNodeFormat: (isCjsCache.get(fileName) ?? false)
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
      const sourceRefCount = sourceRefCounts.get(fileName) ?? 0;
      sourceRefCounts.set(fileName, sourceRefCount + 1);
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
      const sourceRefCount = sourceRefCounts.get(path) ?? 1;
      if (sourceRefCount <= 1) {
        sourceRefCounts.delete(path);
        // We call `cleanupSemanticCache` for other purposes, don't bust the
        // source cache in this case.
        if (lastRequestMethod != "cleanupSemanticCache") {
          const mapKey = path + key;
          documentRegistrySourceFileCache.delete(mapKey);
          sourceTextCache.delete(path);
          ops.op_release(path);
        }
      } else {
        sourceRefCounts.set(path, sourceRefCount - 1);
      }
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
    // We ignore because type definition files can end with '.ts'.
    2688,
    // TS2792: Cannot find module. Did you mean to set the 'moduleResolution'
    // option to 'node', or to add aliases to the 'paths' option?
    2792,
    // TS2307: Cannot find module '{0}' or its corresponding type declarations.
    2307,
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

  /** @typedef {{
   *    ls: ts.LanguageService & { [k:string]: any },
   *    compilerOptions: ts.CompilerOptions,
   *    forceEnabledVerbatimModuleSyntax: boolean,
   *  }} LanguageServiceEntry */
  /** @type {{ unscoped: LanguageServiceEntry, byScope: Map<string, LanguageServiceEntry> }} */
  const languageServiceEntries = {
    // @ts-ignore Will be set later.
    unscoped: null,
    byScope: new Map(),
  };

  /** @type {{ unscoped: string[], byScope: Map<string, string[]> } | null} */
  let scriptNamesCache = null;

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
      if (
        projectVersionCache
      ) {
        debug(`getProjectVersion cache hit : ${projectVersionCache}`);
        return projectVersionCache;
      }
      const projectVersion = ops.op_project_version();
      projectVersionCache = projectVersion;
      debug(`getProjectVersion cache miss : ${projectVersionCache}`);
      return projectVersion;
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
    // @ts-ignore Undocumented method.
    toPath(fileName) {
      // @ts-ignore Undocumented function.
      ts.toPath(
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

      let sourceFile = sourceFileCache.get(specifier);
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

      isCjsCache.set(specifier, isCjs);

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
          const resolved = ops.op_resolve(
            containingFilePath,
            isCjsCache.get(containingFilePath) ?? false,
            [fileReference.fileName],
          )?.[0];
          if (resolved) {
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
      const resolved = ops.op_resolve(
        base,
        isCjsCache.get(base) ?? false,
        specifiers,
      );
      if (resolved) {
        const result = resolved.map((item) => {
          if (item) {
            const [resolvedFileName, extension] = item;
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
      return (lastRequestScope
        ? languageServiceEntries.byScope.get(lastRequestScope)?.compilerOptions
        : null) ?? languageServiceEntries.unscoped.compilerOptions;
    },
    getScriptFileNames() {
      if (logDebug) {
        debug("host.getScriptFileNames()");
      }
      if (!scriptNamesCache) {
        const { unscoped, byScope } = ops.op_script_names();
        scriptNamesCache = {
          unscoped,
          byScope: new Map(Object.entries(byScope)),
        };
      }
      return (lastRequestScope
        ? scriptNamesCache.byScope.get(lastRequestScope)
        : null) ?? scriptNamesCache.unscoped;
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
      const sourceFile = sourceFileCache.get(specifier);
      if (sourceFile) {
        if (!assetScopes.has(specifier)) {
          assetScopes.set(specifier, lastRequestScope);
        }
        // This case only occurs for assets.
        return ts.ScriptSnapshot.fromString(sourceFile.text);
      }
      let sourceText = sourceTextCache.get(specifier);
      if (sourceText == undefined) {
        /** @type {{ data: string, version: string, isCjs: boolean }} */
        const fileInfo = ops.op_load(specifier);
        if (!fileInfo) {
          return undefined;
        }
        isCjsCache.set(specifier, fileInfo.isCjs);
        sourceTextCache.set(specifier, fileInfo.data);
        scriptVersionCache.set(specifier, fileInfo.version);
        sourceText = fileInfo.data;
      }
      return ts.ScriptSnapshot.fromString(sourceText);
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
    if (errors.length > 0 && logDebug) {
      debug(ts.formatDiagnostics(errors, host));
    }
    return options;
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
    ].filter(filterMapDiagnostic.bind(null, false));

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

  /**
   * @param {boolean} isLsp
   * @param {ts.Diagnostic} diagnostic
   */
  function filterMapDiagnostic(isLsp, diagnostic) {
    if (IGNORED_DIAGNOSTICS.includes(diagnostic.code)) {
      return false;
    }
    if (isLsp) {
      // TS1484: `...` is a type and must be imported using a type-only import when 'verbatimModuleSyntax' is enabled.
      // We force-enable `verbatimModuleSyntax` in the LSP so the `type`
      // modifier is used when auto-importing types. But we don't want this
      // diagnostic unless it was explicitly enabled by the user.
      if (diagnostic.code == 1484) {
        const entry = (lastRequestScope
          ? languageServiceEntries.byScope.get(lastRequestScope)
          : null) ?? languageServiceEntries.unscoped;
        if (entry.forceEnabledVerbatimModuleSyntax) {
          return false;
        }
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

  /**
   * @param {any} e
   * @returns {e is (OperationCanceledError | ts.OperationCanceledException)}
   */
  function isCancellationError(e) {
    return e instanceof OperationCanceledError ||
      e instanceof ts.OperationCanceledException;
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

  /** @typedef {[[string, number][], number, [string, any][]] } PendingChange */
  /**
   * @template T
   * @typedef {T | null} Option<T> */

  /** @returns {Promise<[number, string, any[], string | null, Option<PendingChange>] | null>} */
  async function pollRequests() {
    return await ops.op_poll_requests();
  }

  let hasStarted = false;

  /** @param {boolean} enableDebugLogging */
  async function serverMainLoop(enableDebugLogging) {
    if (hasStarted) {
      throw new Error("The language server has already been initialized.");
    }
    hasStarted = true;
    languageServiceEntries.unscoped = {
      ls: ts.createLanguageService(
        host,
        documentRegistry,
      ),
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
        "verbatimModuleSyntax": true,
        "jsx": "react",
        "jsxFactory": "React.createElement",
        "jsxFragmentFactory": "React.Fragment",
      }),
      forceEnabledVerbatimModuleSyntax: true,
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
   * @param {number} id
   * @param {string} method
   * @param {any[]} args
   * @param {string | null} scope
   * @param {PendingChange | null} maybeChange
   */
  function serverRequest(id, method, args, scope, maybeChange) {
    if (logDebug) {
      debug(`serverRequest()`, id, method, args, scope, maybeChange);
    }
    if (maybeChange !== null) {
      const changedScripts = maybeChange[0];
      const newProjectVersion = maybeChange[1];
      const newConfigsByScope = maybeChange[2];
      if (newConfigsByScope) {
        isNodeSourceFileCache.clear();
        assetScopes.clear();
        /** @type { typeof languageServiceEntries.byScope } */
        const newByScope = new Map();
        for (const [scope, config] of newConfigsByScope) {
          lastRequestScope = scope;
          const oldEntry = languageServiceEntries.byScope.get(scope);
          const ls = oldEntry
            ? oldEntry.ls
            : ts.createLanguageService(host, documentRegistry);
          let forceEnabledVerbatimModuleSyntax = false;
          if (!config["verbatimModuleSyntax"]) {
            config["verbatimModuleSyntax"] = true;
            forceEnabledVerbatimModuleSyntax = true;
          }
          const compilerOptions = lspTsConfigToCompilerOptions(config);
          newByScope.set(scope, {
            ls,
            compilerOptions,
            forceEnabledVerbatimModuleSyntax,
          });
          languageServiceEntries.byScope.delete(scope);
        }
        for (const oldEntry of languageServiceEntries.byScope.values()) {
          oldEntry.ls.dispose();
        }
        languageServiceEntries.byScope = newByScope;
      }

      projectVersionCache = newProjectVersion;

      let opened = false;
      let closed = false;
      for (const { 0: script, 1: changeKind } of changedScripts) {
        if (changeKind === ChangeKind.Opened) {
          opened = true;
        } else if (changeKind === ChangeKind.Closed) {
          closed = true;
        }
        scriptVersionCache.delete(script);
        sourceTextCache.delete(script);
      }

      if (newConfigsByScope || opened || closed) {
        scriptNamesCache = null;
      }
    }

    // For requests pertaining to an asset document, we make it so that the
    // passed scope is just its own specifier. We map it to an actual scope here
    // based on the first scope that the asset was loaded into.
    if (scope?.startsWith(ASSETS_URL_PREFIX)) {
      scope = assetScopes.get(scope) ?? null;
    }
    lastRequestMethod = method;
    lastRequestScope = scope;
    const ls = (scope ? languageServiceEntries.byScope.get(scope)?.ls : null) ??
      languageServiceEntries.unscoped.ls;
    switch (method) {
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
        const projectVersion = args[1];
        // there's a possibility that we receive a change notification
        // but the diagnostic server queues a `$getDiagnostics` request
        // with a stale project version. in that case, treat it as cancelled
        // (it's about to be invalidated anyway).
        if (projectVersionCache && projectVersion !== projectVersionCache) {
          return respond(id, {});
        }
        try {
          /** @type {Record<string, any[]>} */
          const diagnosticMap = {};
          for (const specifier of args[0]) {
            diagnosticMap[specifier] = fromTypeScriptDiagnostics([
              ...ls.getSemanticDiagnostics(specifier),
              ...ls.getSuggestionDiagnostics(specifier),
              ...ls.getSyntacticDiagnostics(specifier),
            ].filter(filterMapDiagnostic.bind(null, true)));
          }
          return respond(id, diagnosticMap);
        } catch (e) {
          if (
            !isCancellationError(e)
          ) {
            return respond(
              id,
              {},
              formatErrorWithArgs(e, [id, method, args, scope, maybeChange]),
            );
          }
          return respond(id, {});
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
                formatErrorWithArgs(e, [id, method, args, scope, maybeChange]),
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

  // A build time only op that provides some setup information that is used to
  // ensure the snapshot is setup properly.
  /** @type {{ buildSpecifier: string; libs: string[]; nodeBuiltInModuleNames: string[] }} */
  const { buildSpecifier, libs } = ops.op_build_info();

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
    "exports",
    "gc",
    "Global",
    "ImportMeta",
    "localStorage",
    "module",
    "NodeModule",
    "NodeRequire",
    "process",
    "queueMicrotask",
    "RequestInit",
    "require",
    "ResponseInit",
    "sessionStorage",
    "setImmediate",
    "setInterval",
    "setTimeout",
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
  global.serverMainLoop = serverMainLoop;
})(this);
