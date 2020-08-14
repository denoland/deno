// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to compile TS/WASM to JS.
//
// It provides two functions that should be called by Rust:
//  - `bootstrapCompilerRuntime`
// This functions must be called when creating isolate
// to properly setup runtime.
//  - `tsCompilerOnMessage`
// This function must be called when sending a request
// to the compiler.

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete Object.prototype.__proto__;

((window) => {
  const core = Deno.core;
  const { assert, log, notImplemented } = window.__bootstrap.util;
  const dispatchJson = window.__bootstrap.dispatchJson;
  const util = window.__bootstrap.util;
  const errorStack = window.__bootstrap.errorStack;
  const errors = window.__bootstrap.errors.errors;

  function opNow() {
    const res = dispatchJson.sendSync("op_now");
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }

  const DiagnosticCategory = {
    0: "Log",
    1: "Debug",
    2: "Info",
    3: "Error",
    4: "Warning",
    5: "Suggestion",

    Log: 0,
    Debug: 1,
    Info: 2,
    Error: 3,
    Warning: 4,
    Suggestion: 5,
  };

  const unstableDenoGlobalProperties = [
    "CompilerOptions",
    "DatagramConn",
    "Diagnostic",
    "DiagnosticCategory",
    "DiagnosticItem",
    "DiagnosticMessageChain",
    "EnvPermissionDescriptor",
    "HrtimePermissionDescriptor",
    "HttpClient",
    "LinuxSignal",
    "Location",
    "MacOSSignal",
    "NetPermissionDescriptor",
    "PermissionDescriptor",
    "PermissionName",
    "PermissionState",
    "PermissionStatus",
    "Permissions",
    "PluginPermissionDescriptor",
    "ReadPermissionDescriptor",
    "RunPermissionDescriptor",
    "ShutdownMode",
    "Signal",
    "SignalStream",
    "StartTlsOptions",
    "SymlinkOptions",
    "TranspileOnlyResult",
    "UnixConnectOptions",
    "UnixListenOptions",
    "WritePermissionDescriptor",
    "applySourceMap",
    "bundle",
    "compile",
    "connect",
    "consoleSize",
    "createHttpClient",
    "fdatasync",
    "fdatasyncSync",
    "formatDiagnostics",
    "fstat",
    "fstatSync",
    "fsync",
    "fsyncSync",
    "ftruncate",
    "ftruncateSync",
    "hostname",
    "kill",
    "link",
    "linkSync",
    "listen",
    "listenDatagram",
    "loadavg",
    "mainModule",
    "openPlugin",
    "osRelease",
    "permissions",
    "ppid",
    "setRaw",
    "shutdown",
    "signal",
    "signals",
    "startTls",
    "symlink",
    "symlinkSync",
    "transpileOnly",
    "umask",
    "utime",
    "utimeSync",
  ];

  function transformMessageText(messageText, code) {
    switch (code) {
      case 2339: {
        const property = messageText
          .replace(/^Property '/, "")
          .replace(/' does not exist on type 'typeof Deno'\./, "");

        if (
          messageText.endsWith("on type 'typeof Deno'.") &&
          unstableDenoGlobalProperties.includes(property)
        ) {
          return `${messageText} 'Deno.${property}' is an unstable API. Did you forget to run with the '--unstable' flag?`;
        }
        break;
      }
      case 2551: {
        const suggestionMessagePattern = / Did you mean '(.+)'\?$/;
        const property = messageText
          .replace(/^Property '/, "")
          .replace(/' does not exist on type 'typeof Deno'\./, "")
          .replace(suggestionMessagePattern, "");
        const suggestion = messageText.match(suggestionMessagePattern);
        const replacedMessageText = messageText.replace(
          suggestionMessagePattern,
          "",
        );
        if (suggestion && unstableDenoGlobalProperties.includes(property)) {
          const suggestedProperty = suggestion[1];
          return `${replacedMessageText} 'Deno.${property}' is an unstable API. Did you forget to run with the '--unstable' flag, or did you mean '${suggestedProperty}'?`;
        }
        break;
      }
    }

    return messageText;
  }

  function fromDiagnosticCategory(category) {
    switch (category) {
      case ts.DiagnosticCategory.Error:
        return DiagnosticCategory.Error;
      case ts.DiagnosticCategory.Message:
        return DiagnosticCategory.Info;
      case ts.DiagnosticCategory.Suggestion:
        return DiagnosticCategory.Suggestion;
      case ts.DiagnosticCategory.Warning:
        return DiagnosticCategory.Warning;
      default:
        throw new Error(
          `Unexpected DiagnosticCategory: "${category}"/"${
            ts.DiagnosticCategory[category]
          }"`,
        );
    }
  }

  function getSourceInformation(sourceFile, start, length) {
    const scriptResourceName = sourceFile.fileName;
    const {
      line: lineNumber,
      character: startColumn,
    } = sourceFile.getLineAndCharacterOfPosition(start);
    const endPosition = sourceFile.getLineAndCharacterOfPosition(
      start + length,
    );
    const endColumn = lineNumber === endPosition.line
      ? endPosition.character
      : startColumn;
    const lastLineInFile = sourceFile.getLineAndCharacterOfPosition(
      sourceFile.text.length,
    ).line;
    const lineStart = sourceFile.getPositionOfLineAndCharacter(lineNumber, 0);
    const lineEnd = lineNumber < lastLineInFile
      ? sourceFile.getPositionOfLineAndCharacter(lineNumber + 1, 0)
      : sourceFile.text.length;
    const sourceLine = sourceFile.text
      .slice(lineStart, lineEnd)
      .replace(/\s+$/g, "")
      .replace("\t", " ");
    return {
      sourceLine,
      lineNumber,
      scriptResourceName,
      startColumn,
      endColumn,
    };
  }

  function fromDiagnosticMessageChain(messageChain) {
    if (!messageChain) {
      return undefined;
    }

    return messageChain.map(({ messageText, code, category, next }) => {
      const message = transformMessageText(messageText, code);
      return {
        message,
        code,
        category: fromDiagnosticCategory(category),
        next: fromDiagnosticMessageChain(next),
      };
    });
  }

  function parseDiagnostic(item) {
    const {
      messageText,
      category: sourceCategory,
      code,
      file,
      start: startPosition,
      length,
    } = item;
    const sourceInfo = file && startPosition && length
      ? getSourceInformation(file, startPosition, length)
      : undefined;
    const endPosition = startPosition && length
      ? startPosition + length
      : undefined;
    const category = fromDiagnosticCategory(sourceCategory);

    let message;
    let messageChain;
    if (typeof messageText === "string") {
      message = transformMessageText(messageText, code);
    } else {
      message = transformMessageText(messageText.messageText, messageText.code);
      messageChain = fromDiagnosticMessageChain([messageText])[0];
    }

    const base = {
      message,
      messageChain,
      code,
      category,
      startPosition,
      endPosition,
    };

    return sourceInfo ? { ...base, ...sourceInfo } : base;
  }

  function parseRelatedInformation(relatedInformation) {
    const result = [];
    for (const item of relatedInformation) {
      result.push(parseDiagnostic(item));
    }
    return result;
  }

  function fromTypeScriptDiagnostic(diagnostics) {
    const items = [];
    for (const sourceDiagnostic of diagnostics) {
      const item = parseDiagnostic(sourceDiagnostic);
      if (sourceDiagnostic.relatedInformation) {
        item.relatedInformation = parseRelatedInformation(
          sourceDiagnostic.relatedInformation,
        );
      }
      items.push(item);
    }
    return { items };
  }

  // We really don't want to depend on JSON dispatch during snapshotting, so
  // this op exchanges strings with Rust as raw byte arrays.
  function getAsset(name) {
    const opId = core.ops()["op_fetch_asset"];
    const sourceCodeBytes = core.dispatch(opId, core.encode(name));
    return core.decode(sourceCodeBytes);
  }

  // Constants used by `normalizeString` and `resolvePath`
  const CHAR_DOT = 46; /* . */
  const CHAR_FORWARD_SLASH = 47; /* / */
  // Using incremental compile APIs requires that all
  // paths must be either relative or absolute. Since
  // analysis in Rust operates on fully resolved URLs,
  // it makes sense to use the same scheme here.
  const ASSETS = "asset://";
  const OUT_DIR = "deno://";
  // This constant is passed to compiler settings when
  // doing incremental compiles. Contents of this
  // file are passed back to Rust and saved to $DENO_DIR.
  const TS_BUILD_INFO = "cache:///tsbuildinfo.json";

  // TODO(Bartlomieju): this check should be done in Rust
  const IGNORED_COMPILER_OPTIONS = [
    "allowSyntheticDefaultImports",
    "allowUmdGlobalAccess",
    "assumeChangesOnlyAffectDirectDependencies",
    "baseUrl",
    "build",
    "composite",
    "declaration",
    "declarationDir",
    "declarationMap",
    "diagnostics",
    "downlevelIteration",
    "emitBOM",
    "emitDeclarationOnly",
    "esModuleInterop",
    "extendedDiagnostics",
    "forceConsistentCasingInFileNames",
    "generateCpuProfile",
    "help",
    "importHelpers",
    "incremental",
    "inlineSourceMap",
    "inlineSources",
    "init",
    "listEmittedFiles",
    "listFiles",
    "mapRoot",
    "maxNodeModuleJsDepth",
    "module",
    "moduleResolution",
    "newLine",
    "noEmit",
    "noEmitHelpers",
    "noEmitOnError",
    "noLib",
    "noResolve",
    "out",
    "outDir",
    "outFile",
    "paths",
    "preserveSymlinks",
    "preserveWatchOutput",
    "pretty",
    "rootDir",
    "rootDirs",
    "showConfig",
    "skipDefaultLibCheck",
    "skipLibCheck",
    "sourceMap",
    "sourceRoot",
    "stripInternal",
    "target",
    "traceResolution",
    "tsBuildInfoFile",
    "types",
    "typeRoots",
    "version",
    "watch",
  ];

  const DEFAULT_BUNDLER_OPTIONS = {
    allowJs: true,
    inlineSourceMap: false,
    module: ts.ModuleKind.System,
    outDir: undefined,
    outFile: `${OUT_DIR}/bundle.js`,
    // disabled until we have effective way to modify source maps
    sourceMap: false,
  };

  const DEFAULT_INCREMENTAL_COMPILE_OPTIONS = {
    allowJs: false,
    allowNonTsExtensions: true,
    checkJs: false,
    esModuleInterop: true,
    incremental: true,
    inlineSourceMap: true,
    jsx: ts.JsxEmit.React,
    module: ts.ModuleKind.ESNext,
    outDir: OUT_DIR,
    resolveJsonModule: true,
    sourceMap: false,
    strict: true,
    stripComments: true,
    target: ts.ScriptTarget.ESNext,
    tsBuildInfoFile: TS_BUILD_INFO,
  };

  const DEFAULT_COMPILE_OPTIONS = {
    allowJs: false,
    allowNonTsExtensions: true,
    checkJs: false,
    esModuleInterop: true,
    jsx: ts.JsxEmit.React,
    module: ts.ModuleKind.ESNext,
    outDir: OUT_DIR,
    sourceMap: true,
    strict: true,
    removeComments: true,
    target: ts.ScriptTarget.ESNext,
  };

  const DEFAULT_TRANSPILE_OPTIONS = {
    esModuleInterop: true,
    inlineSourceMap: true,
    jsx: ts.JsxEmit.React,
    module: ts.ModuleKind.ESNext,
    removeComments: true,
    target: ts.ScriptTarget.ESNext,
  };

  const DEFAULT_RUNTIME_COMPILE_OPTIONS = {
    outDir: undefined,
  };

  const DEFAULT_RUNTIME_TRANSPILE_OPTIONS = {
    esModuleInterop: true,
    module: ts.ModuleKind.ESNext,
    sourceMap: true,
    scriptComments: true,
    target: ts.ScriptTarget.ESNext,
  };

  const CompilerHostTarget = {
    Main: "main",
    Runtime: "runtime",
    Worker: "worker",
  };

  // Warning! The values in this enum are duplicated in `cli/msg.rs`
  // Update carefully!
  const MediaType = {
    0: "JavaScript",
    1: "JSX",
    2: "TypeScript",
    3: "TSX",
    4: "Json",
    5: "Wasm",
    6: "Unknown",
    JavaScript: 0,
    JSX: 1,
    TypeScript: 2,
    TSX: 3,
    Json: 4,
    Wasm: 5,
    Unknown: 6,
  };

  function getExtension(fileName, mediaType) {
    switch (mediaType) {
      case MediaType.JavaScript:
        return ts.Extension.Js;
      case MediaType.JSX:
        return ts.Extension.Jsx;
      case MediaType.TypeScript:
        return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
      case MediaType.TSX:
        return ts.Extension.Tsx;
      case MediaType.Wasm:
        // Custom marker for Wasm type.
        return ts.Extension.Js;
      case MediaType.Unknown:
      default:
        throw TypeError(
          `Cannot resolve extension for "${fileName}" with mediaType "${
            MediaType[mediaType]
          }".`,
        );
    }
  }

  /** A global cache of module source files that have been loaded.
   * This cache will be rewritten to be populated on compiler startup
   * with files provided from Rust in request message.
   */
  const SOURCE_FILE_CACHE = new Map();
  /** A map of maps which cache resolved specifier for each import in a file.
   * This cache is used so `resolveModuleNames` ops is called as few times
   * as possible.
   *
   * First map's key is "referrer" URL ("file://a/b/c/mod.ts")
   * Second map's key is "raw" import specifier ("./foo.ts")
   * Second map's value is resolved import URL ("file:///a/b/c/foo.ts")
   */
  const RESOLVED_SPECIFIER_CACHE = new Map();

  function configure(defaultOptions, source, path, cwd) {
    const { config, error } = ts.parseConfigFileTextToJson(path, source);
    if (error) {
      return { diagnostics: [error], options: defaultOptions };
    }
    const { options, errors } = ts.convertCompilerOptionsFromJson(
      config.compilerOptions,
      cwd,
    );
    const ignoredOptions = [];
    for (const key of Object.keys(options)) {
      if (
        IGNORED_COMPILER_OPTIONS.includes(key) &&
        (!(key in defaultOptions) || options[key] !== defaultOptions[key])
      ) {
        ignoredOptions.push(key);
        delete options[key];
      }
    }
    return {
      options: Object.assign({}, defaultOptions, options),
      ignoredOptions: ignoredOptions.length ? ignoredOptions : undefined,
      diagnostics: errors.length ? errors : undefined,
    };
  }

  class SourceFile {
    constructor(json) {
      this.processed = false;
      Object.assign(this, json);
      this.extension = getExtension(this.url, this.mediaType);
    }

    static addToCache(json) {
      if (SOURCE_FILE_CACHE.has(json.url)) {
        throw new TypeError("SourceFile already exists");
      }
      const sf = new SourceFile(json);
      SOURCE_FILE_CACHE.set(sf.url, sf);
      return sf;
    }

    static getCached(url) {
      return SOURCE_FILE_CACHE.get(url);
    }

    static cacheResolvedUrl(resolvedUrl, rawModuleSpecifier, containingFile) {
      containingFile = containingFile || "";
      let innerCache = RESOLVED_SPECIFIER_CACHE.get(containingFile);
      if (!innerCache) {
        innerCache = new Map();
        RESOLVED_SPECIFIER_CACHE.set(containingFile, innerCache);
      }
      innerCache.set(rawModuleSpecifier, resolvedUrl);
    }

    static getResolvedUrl(moduleSpecifier, containingFile) {
      const containingCache = RESOLVED_SPECIFIER_CACHE.get(containingFile);
      if (containingCache) {
        return containingCache.get(moduleSpecifier);
      }
      return undefined;
    }
  }

  function getAssetInternal(filename) {
    const lastSegment = filename.split("/").pop();
    const url = ts.libMap.has(lastSegment)
      ? ts.libMap.get(lastSegment)
      : lastSegment;
    const sourceFile = SourceFile.getCached(url);
    if (sourceFile) {
      return sourceFile;
    }
    const name = url.includes(".") ? url : `${url}.d.ts`;
    const sourceCode = getAsset(name);
    return SourceFile.addToCache({
      url,
      filename: `${ASSETS}/${name}`,
      mediaType: MediaType.TypeScript,
      versionHash: "1",
      sourceCode,
    });
  }

  class Host {
    #options = DEFAULT_COMPILE_OPTIONS;
    #target = "";
    #writeFile = null;
    /* Deno specific APIs */

    constructor({
      bundle = false,
      incremental = false,
      target,
      unstable,
      writeFile,
    }) {
      this.#target = target;
      this.#writeFile = writeFile;
      if (bundle) {
        // options we need to change when we are generating a bundle
        Object.assign(this.#options, DEFAULT_BUNDLER_OPTIONS);
      } else if (incremental) {
        Object.assign(this.#options, DEFAULT_INCREMENTAL_COMPILE_OPTIONS);
      }
      if (unstable) {
        this.#options.lib = [
          target === CompilerHostTarget.Worker
            ? "lib.deno.worker.d.ts"
            : "lib.deno.window.d.ts",
          "lib.deno.unstable.d.ts",
        ];
      }
    }

    get options() {
      return this.#options;
    }

    configure(cwd, path, configurationText) {
      log("compiler::host.configure", path);
      const { options, ...result } = configure(
        this.#options,
        configurationText,
        path,
        cwd,
      );
      this.#options = options;
      return result;
    }

    mergeOptions(...options) {
      Object.assign(this.#options, ...options);
      return Object.assign({}, this.#options);
    }

    /* TypeScript CompilerHost APIs */

    fileExists(_fileName) {
      return notImplemented();
    }

    getCanonicalFileName(fileName) {
      return fileName;
    }

    getCompilationSettings() {
      log("compiler::host.getCompilationSettings()");
      return this.#options;
    }

    getCurrentDirectory() {
      return "";
    }

    getDefaultLibFileName(_options) {
      log("compiler::host.getDefaultLibFileName()");
      switch (this.#target) {
        case CompilerHostTarget.Main:
        case CompilerHostTarget.Runtime:
          return `${ASSETS}/lib.deno.window.d.ts`;
        case CompilerHostTarget.Worker:
          return `${ASSETS}/lib.deno.worker.d.ts`;
      }
    }

    getNewLine() {
      return "\n";
    }

    getSourceFile(
      fileName,
      languageVersion,
      onError,
      shouldCreateNewSourceFile,
    ) {
      log("compiler::host.getSourceFile", fileName);
      try {
        assert(!shouldCreateNewSourceFile);
        const sourceFile = fileName.startsWith(ASSETS)
          ? getAssetInternal(fileName)
          : SourceFile.getCached(fileName);
        assert(sourceFile != null);
        if (!sourceFile.tsSourceFile) {
          assert(sourceFile.sourceCode != null);
          const tsSourceFileName = fileName.startsWith(ASSETS)
            ? sourceFile.filename
            : fileName;

          sourceFile.tsSourceFile = ts.createSourceFile(
            tsSourceFileName,
            sourceFile.sourceCode,
            languageVersion,
          );
          sourceFile.tsSourceFile.version = sourceFile.versionHash;
          delete sourceFile.sourceCode;
        }
        return sourceFile.tsSourceFile;
      } catch (e) {
        if (onError) {
          onError(String(e));
        } else {
          throw e;
        }
        return undefined;
      }
    }

    readFile(_fileName) {
      return notImplemented();
    }

    resolveModuleNames(moduleNames, containingFile) {
      log("compiler::host.resolveModuleNames", {
        moduleNames,
        containingFile,
      });
      const resolved = moduleNames.map((specifier) => {
        const maybeUrl = SourceFile.getResolvedUrl(specifier, containingFile);

        log("compiler::host.resolveModuleNames maybeUrl", {
          specifier,
          maybeUrl,
        });

        let sourceFile = undefined;

        if (specifier.startsWith(ASSETS)) {
          sourceFile = getAssetInternal(specifier);
        } else if (typeof maybeUrl !== "undefined") {
          sourceFile = SourceFile.getCached(maybeUrl);
        }

        if (!sourceFile) {
          return undefined;
        }

        return {
          resolvedFileName: sourceFile.url,
          isExternalLibraryImport: specifier.startsWith(ASSETS),
          extension: sourceFile.extension,
        };
      });
      log(resolved);
      return resolved;
    }

    useCaseSensitiveFileNames() {
      return true;
    }

    writeFile(fileName, data, _writeByteOrderMark, _onError, sourceFiles) {
      log("compiler::host.writeFile", fileName);
      this.#writeFile(fileName, data, sourceFiles);
    }
  }

  class IncrementalCompileHost extends Host {
    #buildInfo = "";

    constructor(options) {
      super({ ...options, incremental: true });
      const { buildInfo } = options;
      if (buildInfo) {
        this.#buildInfo = buildInfo;
      }
    }

    readFile(fileName) {
      if (fileName == TS_BUILD_INFO) {
        return this.#buildInfo;
      }
      throw new Error("unreachable");
    }
  }

  // NOTE: target doesn't really matter here,
  // this is in fact a mock host created just to
  // load all type definitions and snapshot them.
  let SNAPSHOT_HOST = new Host({
    target: CompilerHostTarget.Main,
    writeFile() {},
  });
  const SNAPSHOT_COMPILER_OPTIONS = SNAPSHOT_HOST.getCompilationSettings();

  // This is a hacky way of adding our libs to the libs available in TypeScript()
  // as these are internal APIs of TypeScript which maintain valid libs
  ts.libs.push("deno.ns", "deno.window", "deno.worker", "deno.shared_globals");
  ts.libMap.set("deno.ns", "lib.deno.ns.d.ts");
  ts.libMap.set("deno.web", "lib.deno.web.d.ts");
  ts.libMap.set("deno.window", "lib.deno.window.d.ts");
  ts.libMap.set("deno.worker", "lib.deno.worker.d.ts");
  ts.libMap.set("deno.shared_globals", "lib.deno.shared_globals.d.ts");
  ts.libMap.set("deno.unstable", "lib.deno.unstable.d.ts");

  // this pre-populates the cache at snapshot time of our library files, so they
  // are available in the future when needed.
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.ns.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.web.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.window.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.worker.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.shared_globals.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  SNAPSHOT_HOST.getSourceFile(
    `${ASSETS}/lib.deno.unstable.d.ts`,
    ts.ScriptTarget.ESNext,
  );

  // We never use this program; it's only created
  // during snapshotting to hydrate and populate
  // source file cache with lib declaration files.
  const _TS_SNAPSHOT_PROGRAM = ts.createProgram({
    rootNames: [`${ASSETS}/bootstrap.ts`],
    options: SNAPSHOT_COMPILER_OPTIONS,
    host: SNAPSHOT_HOST,
  });

  // Derference the snapshot host so it can be GCed
  SNAPSHOT_HOST = undefined;

  // This function is called only during snapshotting process
  const SYSTEM_LOADER = getAsset("system_loader.js");
  const SYSTEM_LOADER_ES5 = getAsset("system_loader_es5.js");

  function buildLocalSourceFileCache(sourceFileMap) {
    for (const entry of Object.values(sourceFileMap)) {
      assert(entry.sourceCode.length > 0);
      SourceFile.addToCache({
        url: entry.url,
        filename: entry.url,
        mediaType: entry.mediaType,
        sourceCode: entry.sourceCode,
        versionHash: entry.versionHash,
      });

      for (const importDesc of entry.imports) {
        let mappedUrl = importDesc.resolvedSpecifier;
        const importedFile = sourceFileMap[importDesc.resolvedSpecifier];
        assert(importedFile);
        const isJsOrJsx = importedFile.mediaType === MediaType.JavaScript ||
          importedFile.mediaType === MediaType.JSX;
        // If JS or JSX perform substitution for types if available
        if (isJsOrJsx) {
          // @deno-types has highest precedence, followed by
          // X-TypeScript-Types header
          if (importDesc.resolvedTypeDirective) {
            mappedUrl = importDesc.resolvedTypeDirective;
          } else if (importedFile.typeHeaders.length > 0) {
            const typeHeaders = importedFile.typeHeaders[0];
            mappedUrl = typeHeaders.resolvedSpecifier;
          } else if (importedFile.typesDirectives.length > 0) {
            const typeDirective = importedFile.typesDirectives[0];
            mappedUrl = typeDirective.resolvedSpecifier;
          }
        }

        mappedUrl = mappedUrl.replace("memory://", "");
        SourceFile.cacheResolvedUrl(mappedUrl, importDesc.specifier, entry.url);
      }
      for (const fileRef of entry.referencedFiles) {
        SourceFile.cacheResolvedUrl(
          fileRef.resolvedSpecifier.replace("memory://", ""),
          fileRef.specifier,
          entry.url,
        );
      }
      for (const fileRef of entry.libDirectives) {
        SourceFile.cacheResolvedUrl(
          fileRef.resolvedSpecifier.replace("memory://", ""),
          fileRef.specifier,
          entry.url,
        );
      }
    }
  }

  function buildSourceFileCache(sourceFileMap) {
    for (const entry of Object.values(sourceFileMap)) {
      SourceFile.addToCache({
        url: entry.url,
        filename: entry.url,
        mediaType: entry.mediaType,
        sourceCode: entry.sourceCode,
        versionHash: entry.versionHash,
      });

      for (const importDesc of entry.imports) {
        let mappedUrl = importDesc.resolvedSpecifier;
        const importedFile = sourceFileMap[importDesc.resolvedSpecifier];
        // IMPORTANT: due to HTTP redirects we might end up in situation
        // where URL points to a file with completely different URL.
        // In that case we take value of `redirect` field and cache
        // resolved specifier pointing to the value of the redirect.
        // It's not very elegant solution and should be rethinked.
        assert(importedFile);
        if (importedFile.redirect) {
          mappedUrl = importedFile.redirect;
        }
        const isJsOrJsx = importedFile.mediaType === MediaType.JavaScript ||
          importedFile.mediaType === MediaType.JSX;
        // If JS or JSX perform substitution for types if available
        if (isJsOrJsx) {
          // @deno-types has highest precedence, followed by
          // X-TypeScript-Types header
          if (importDesc.resolvedTypeDirective) {
            mappedUrl = importDesc.resolvedTypeDirective;
          } else if (importedFile.typeHeaders.length > 0) {
            const typeHeaders = importedFile.typeHeaders[0];
            mappedUrl = typeHeaders.resolvedSpecifier;
          } else if (importedFile.typesDirectives.length > 0) {
            const typeDirective = importedFile.typesDirectives[0];
            mappedUrl = typeDirective.resolvedSpecifier;
          }
        }

        SourceFile.cacheResolvedUrl(mappedUrl, importDesc.specifier, entry.url);
      }
      for (const fileRef of entry.referencedFiles) {
        SourceFile.cacheResolvedUrl(
          fileRef.resolvedSpecifier,
          fileRef.specifier,
          entry.url,
        );
      }
      for (const fileRef of entry.libDirectives) {
        SourceFile.cacheResolvedUrl(
          fileRef.resolvedSpecifier,
          fileRef.specifier,
          entry.url,
        );
      }
    }
  }

  // Warning! The values in this enum are duplicated in `cli/msg.rs`
  // Update carefully!
  const CompilerRequestType = {
    Compile: 0,
    Transpile: 1,
    Bundle: 2,
    RuntimeCompile: 3,
    RuntimeBundle: 4,
    RuntimeTranspile: 5,
  };

  function createBundleWriteFile(state) {
    return function writeFile(_fileName, data, sourceFiles) {
      assert(sourceFiles != null);
      assert(state.host);
      // we only support single root names for bundles
      assert(state.rootNames.length === 1);
      state.bundleOutput = buildBundle(
        state.rootNames[0],
        data,
        sourceFiles,
        state.host.options.target ?? ts.ScriptTarget.ESNext,
      );
    };
  }

  function createCompileWriteFile(state) {
    return function writeFile(fileName, data, sourceFiles) {
      const isBuildInfo = fileName === TS_BUILD_INFO;

      if (isBuildInfo) {
        assert(isBuildInfo);
        state.buildInfo = data;
        return;
      }

      assert(sourceFiles);
      assert(sourceFiles.length === 1);
      state.emitMap[fileName] = {
        filename: sourceFiles[0].fileName,
        contents: data,
      };
    };
  }

  function createRuntimeCompileWriteFile(state) {
    return function writeFile(fileName, data, sourceFiles) {
      assert(sourceFiles);
      assert(sourceFiles.length === 1);
      state.emitMap[fileName] = {
        filename: sourceFiles[0].fileName,
        contents: data,
      };
    };
  }

  function convertCompilerOptions(str) {
    const options = JSON.parse(str);
    const out = {};
    const keys = Object.keys(options);
    const files = [];
    for (const key of keys) {
      switch (key) {
        case "jsx":
          const value = options[key];
          if (value === "preserve") {
            out[key] = ts.JsxEmit.Preserve;
          } else if (value === "react") {
            out[key] = ts.JsxEmit.React;
          } else {
            out[key] = ts.JsxEmit.ReactNative;
          }
          break;
        case "module":
          switch (options[key]) {
            case "amd":
              out[key] = ts.ModuleKind.AMD;
              break;
            case "commonjs":
              out[key] = ts.ModuleKind.CommonJS;
              break;
            case "es2015":
            case "es6":
              out[key] = ts.ModuleKind.ES2015;
              break;
            case "esnext":
              out[key] = ts.ModuleKind.ESNext;
              break;
            case "none":
              out[key] = ts.ModuleKind.None;
              break;
            case "system":
              out[key] = ts.ModuleKind.System;
              break;
            case "umd":
              out[key] = ts.ModuleKind.UMD;
              break;
            default:
              throw new TypeError("Unexpected module type");
          }
          break;
        case "target":
          switch (options[key]) {
            case "es3":
              out[key] = ts.ScriptTarget.ES3;
              break;
            case "es5":
              out[key] = ts.ScriptTarget.ES5;
              break;
            case "es6":
            case "es2015":
              out[key] = ts.ScriptTarget.ES2015;
              break;
            case "es2016":
              out[key] = ts.ScriptTarget.ES2016;
              break;
            case "es2017":
              out[key] = ts.ScriptTarget.ES2017;
              break;
            case "es2018":
              out[key] = ts.ScriptTarget.ES2018;
              break;
            case "es2019":
              out[key] = ts.ScriptTarget.ES2019;
              break;
            case "es2020":
              out[key] = ts.ScriptTarget.ES2020;
              break;
            case "esnext":
              out[key] = ts.ScriptTarget.ESNext;
              break;
            default:
              throw new TypeError("Unexpected emit target.");
          }
          break;
        case "types":
          const types = options[key];
          assert(types);
          files.push(...types);
          break;
        default:
          out[key] = options[key];
      }
    }
    return {
      options: out,
      files: files.length ? files : undefined,
    };
  }

  const ignoredDiagnostics = [
    // TS2306: File 'file:///Users/rld/src/deno/cli/tests/subdir/amd_like.js' is
    // not a module.
    2306,
    // TS1375: 'await' expressions are only allowed at the top level of a file
    // when that file is a module, but this file has no imports or exports.
    // Consider adding an empty 'export {}' to make this file a module.
    1375,
    // TS1103: 'for-await-of' statement is only allowed within an async function
    // or async generator.
    1103,
    // TS2691: An import path cannot end with a '.ts' extension. Consider
    // importing 'bad-module' instead.
    2691,
    // TS5009: Cannot find the common subdirectory path for the input files.
    5009,
    // TS5055: Cannot write file
    // 'http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js'
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

  const stats = [];
  let statsStart = 0;

  function performanceStart() {
    stats.length = 0;
    // TODO(kitsonk) replace with performance.mark() when landed
    statsStart = opNow();
    ts.performance.enable();
  }

  function performanceProgram({ program, fileCount }) {
    if (program) {
      if ("getProgram" in program) {
        program = program.getProgram();
      }
      stats.push({ key: "Files", value: program.getSourceFiles().length });
      stats.push({ key: "Nodes", value: program.getNodeCount() });
      stats.push({ key: "Identifiers", value: program.getIdentifierCount() });
      stats.push({ key: "Symbols", value: program.getSymbolCount() });
      stats.push({ key: "Types", value: program.getTypeCount() });
      stats.push({
        key: "Instantiations",
        value: program.getInstantiationCount(),
      });
    } else if (fileCount != null) {
      stats.push({ key: "Files", value: fileCount });
    }
    const programTime = ts.performance.getDuration("Program");
    const bindTime = ts.performance.getDuration("Bind");
    const checkTime = ts.performance.getDuration("Check");
    const emitTime = ts.performance.getDuration("Emit");
    stats.push({ key: "Parse time", value: programTime });
    stats.push({ key: "Bind time", value: bindTime });
    stats.push({ key: "Check time", value: checkTime });
    stats.push({ key: "Emit time", value: emitTime });
    stats.push({
      key: "Total TS time",
      value: programTime + bindTime + checkTime + emitTime,
    });
  }

  function performanceEnd() {
    // TODO(kitsonk) replace with performance.measure() when landed
    const duration = opNow() - statsStart;
    stats.push({ key: "Compile time", value: duration });
    return stats;
  }

  // TODO(Bartlomieju): this check should be done in Rust; there should be no
  function processConfigureResponse(configResult, configPath) {
    const { ignoredOptions, diagnostics } = configResult;
    if (ignoredOptions) {
      const msg =
        `Unsupported compiler options in "${configPath}"\n  The following options were ignored:\n    ${
          ignoredOptions
            .map((value) => value)
            .join(", ")
        }\n`;
      core.print(msg, true);
    }
    return diagnostics;
  }

  function normalizeString(path) {
    let res = "";
    let lastSegmentLength = 0;
    let lastSlash = -1;
    let dots = 0;
    let code;
    for (let i = 0, len = path.length; i <= len; ++i) {
      if (i < len) code = path.charCodeAt(i);
      else if (code === CHAR_FORWARD_SLASH) break;
      else code = CHAR_FORWARD_SLASH;

      if (code === CHAR_FORWARD_SLASH) {
        if (lastSlash === i - 1 || dots === 1) {
          // NOOP
        } else if (lastSlash !== i - 1 && dots === 2) {
          if (
            res.length < 2 ||
            lastSegmentLength !== 2 ||
            res.charCodeAt(res.length - 1) !== CHAR_DOT ||
            res.charCodeAt(res.length - 2) !== CHAR_DOT
          ) {
            if (res.length > 2) {
              const lastSlashIndex = res.lastIndexOf("/");
              if (lastSlashIndex === -1) {
                res = "";
                lastSegmentLength = 0;
              } else {
                res = res.slice(0, lastSlashIndex);
                lastSegmentLength = res.length - 1 - res.lastIndexOf("/");
              }
              lastSlash = i;
              dots = 0;
              continue;
            } else if (res.length === 2 || res.length === 1) {
              res = "";
              lastSegmentLength = 0;
              lastSlash = i;
              dots = 0;
              continue;
            }
          }
        } else {
          if (res.length > 0) res += "/" + path.slice(lastSlash + 1, i);
          else res = path.slice(lastSlash + 1, i);
          lastSegmentLength = i - lastSlash - 1;
        }
        lastSlash = i;
        dots = 0;
      } else if (code === CHAR_DOT && dots !== -1) {
        ++dots;
      } else {
        dots = -1;
      }
    }
    return res;
  }

  function commonPath(paths, sep = "/") {
    const [first = "", ...remaining] = paths;
    if (first === "" || remaining.length === 0) {
      return first.substring(0, first.lastIndexOf(sep) + 1);
    }
    const parts = first.split(sep);

    let endOfPrefix = parts.length;
    for (const path of remaining) {
      const compare = path.split(sep);
      for (let i = 0; i < endOfPrefix; i++) {
        if (compare[i] !== parts[i]) {
          endOfPrefix = i;
        }
      }

      if (endOfPrefix === 0) {
        return "";
      }
    }
    const prefix = parts.slice(0, endOfPrefix).join(sep);
    return prefix.endsWith(sep) ? prefix : `${prefix}${sep}`;
  }

  let rootExports;

  function normalizeUrl(rootName) {
    const match = /^(\S+:\/{2,3})(.+)$/.exec(rootName);
    if (match) {
      const [, protocol, path] = match;
      return `${protocol}${normalizeString(path)}`;
    } else {
      return rootName;
    }
  }

  function buildBundle(rootName, data, sourceFiles, target) {
    // when outputting to AMD and a single outfile, TypeScript makes up the module
    // specifiers which are used to define the modules, and doesn't expose them
    // publicly, so we have to try to replicate
    const sources = sourceFiles.map((sf) => sf.fileName);
    const sharedPath = commonPath(sources);
    rootName = normalizeUrl(rootName)
      .replace(sharedPath, "")
      .replace(/\.\w+$/i, "");
    // If one of the modules requires support for top-level-await, TypeScript will
    // emit the execute function as an async function.  When this is the case we
    // need to bubble up the TLA to the instantiation, otherwise we instantiate
    // synchronously.
    const hasTla = data.match(/execute:\sasync\sfunction\s/);
    let instantiate;
    if (rootExports && rootExports.length) {
      instantiate = hasTla
        ? `const __exp = await __instantiate("${rootName}", true);\n`
        : `const __exp = __instantiate("${rootName}", false);\n`;
      for (const rootExport of rootExports) {
        if (rootExport === "default") {
          instantiate += `export default __exp["${rootExport}"];\n`;
        } else {
          instantiate +=
            `export const ${rootExport} = __exp["${rootExport}"];\n`;
        }
      }
    } else {
      instantiate = hasTla
        ? `await __instantiate("${rootName}", true);\n`
        : `__instantiate("${rootName}", false);\n`;
    }
    const es5Bundle = target === ts.ScriptTarget.ES3 ||
      target === ts.ScriptTarget.ES5 ||
      target === ts.ScriptTarget.ES2015 ||
      target === ts.ScriptTarget.ES2016;
    return `${
      es5Bundle ? SYSTEM_LOADER_ES5 : SYSTEM_LOADER
    }\n${data}\n${instantiate}`;
  }

  function setRootExports(program, rootModule) {
    // get a reference to the type checker, this will let us find symbols from
    // the AST.
    const checker = program.getTypeChecker();
    // get a reference to the main source file for the bundle
    const mainSourceFile = program.getSourceFile(rootModule);
    assert(mainSourceFile);
    // retrieve the internal TypeScript symbol for this AST node
    const mainSymbol = checker.getSymbolAtLocation(mainSourceFile);
    if (!mainSymbol) {
      return;
    }
    rootExports = checker
      .getExportsOfModule(mainSymbol)
      // .getExportsOfModule includes type only symbols which are exported from
      // the module, so we need to try to filter those out.  While not critical
      // someone looking at the bundle would think there is runtime code behind
      // that when there isn't.  There appears to be no clean way of figuring that
      // out, so inspecting SymbolFlags that might be present that are type only
      .filter(
        (sym) =>
          sym.flags & ts.SymbolFlags.Class ||
          !(
            sym.flags & ts.SymbolFlags.Interface ||
            sym.flags & ts.SymbolFlags.TypeLiteral ||
            sym.flags & ts.SymbolFlags.Signature ||
            sym.flags & ts.SymbolFlags.TypeParameter ||
            sym.flags & ts.SymbolFlags.TypeAlias ||
            sym.flags & ts.SymbolFlags.Type ||
            sym.flags & ts.SymbolFlags.Namespace ||
            sym.flags & ts.SymbolFlags.InterfaceExcludes ||
            sym.flags & ts.SymbolFlags.TypeParameterExcludes ||
            sym.flags & ts.SymbolFlags.TypeAliasExcludes
          ),
      )
      .map((sym) => sym.getName());
  }

  function compile({
    allowJs,
    buildInfo,
    config,
    configPath,
    rootNames,
    target,
    unstable,
    cwd,
    sourceFileMap,
    type,
    performance,
  }) {
    if (performance) {
      performanceStart();
    }
    log(">>> compile start", { rootNames, type: CompilerRequestType[type] });

    // When a programme is emitted, TypeScript will call `writeFile` with
    // each file that needs to be emitted.  The Deno compiler host delegates
    // this, to make it easier to perform the right actions, which vary
    // based a lot on the request.
    const state = {
      rootNames,
      emitMap: {},
    };
    const host = new IncrementalCompileHost({
      bundle: false,
      target,
      unstable,
      writeFile: createCompileWriteFile(state),
      rootNames,
      buildInfo,
    });
    let diagnostics = [];

    host.mergeOptions({ allowJs });

    // if there is a configuration supplied, we need to parse that
    if (config && config.length && configPath) {
      const configResult = host.configure(cwd, configPath, config);
      diagnostics = processConfigureResponse(configResult, configPath) || [];
    }

    buildSourceFileCache(sourceFileMap);
    // if there was a configuration and no diagnostics with it, we will continue
    // to generate the program and possibly emit it.
    if (diagnostics.length === 0) {
      const options = host.getCompilationSettings();
      const program = ts.createIncrementalProgram({
        rootNames,
        options,
        host,
      });

      // TODO(bartlomieju): check if this is ok
      diagnostics = [
        ...program.getConfigFileParsingDiagnostics(),
        ...program.getSyntacticDiagnostics(),
        ...program.getOptionsDiagnostics(),
        ...program.getGlobalDiagnostics(),
        ...program.getSemanticDiagnostics(),
      ];
      diagnostics = diagnostics.filter(
        ({ code }) => !ignoredDiagnostics.includes(code),
      );

      // We will only proceed with the emit if there are no diagnostics.
      if (diagnostics.length === 0) {
        const emitResult = program.emit();
        // If `checkJs` is off we still might be compiling entry point JavaScript file
        // (if it has `.ts` imports), but it won't be emitted. In that case we skip
        // assertion.
        if (options.checkJs) {
          assert(
            emitResult.emitSkipped === false,
            "Unexpected skip of the emit.",
          );
        }
        // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
        // without casting.
        diagnostics = emitResult.diagnostics;
      }
      performanceProgram({ program });
    }

    log("<<< compile end", { rootNames, type: CompilerRequestType[type] });
    const stats = performance ? performanceEnd() : undefined;

    return {
      emitMap: state.emitMap,
      buildInfo: state.buildInfo,
      diagnostics: fromTypeScriptDiagnostic(diagnostics),
      stats,
    };
  }

  function transpile({
    config: configText,
    configPath,
    cwd,
    performance,
    sourceFiles,
  }) {
    if (performance) {
      performanceStart();
    }
    log(">>> transpile start");
    let compilerOptions;
    if (configText && configPath && cwd) {
      const { options, ...response } = configure(
        DEFAULT_TRANSPILE_OPTIONS,
        configText,
        configPath,
        cwd,
      );
      const diagnostics = processConfigureResponse(response, configPath);
      if (diagnostics && diagnostics.length) {
        return {
          diagnostics: fromTypeScriptDiagnostic(diagnostics),
          emitMap: {},
        };
      }
      compilerOptions = options;
    } else {
      compilerOptions = Object.assign({}, DEFAULT_TRANSPILE_OPTIONS);
    }
    const emitMap = {};
    let diagnostics = [];
    for (const { sourceCode, fileName } of sourceFiles) {
      const {
        outputText,
        sourceMapText,
        diagnostics: diags,
      } = ts.transpileModule(sourceCode, {
        fileName,
        compilerOptions,
        reportDiagnostics: true,
      });
      if (diags) {
        diagnostics = diagnostics.concat(...diags);
      }
      emitMap[`${fileName}.js`] = { filename: fileName, contents: outputText };
      // currently we inline source maps, but this is good logic to have if this
      // ever changes
      if (sourceMapText) {
        emitMap[`${fileName}.map`] = {
          filename: fileName,
          contents: sourceMapText,
        };
      }
    }
    performanceProgram({ fileCount: sourceFiles.length });
    const stats = performance ? performanceEnd() : undefined;
    log("<<< transpile end");
    return {
      diagnostics: fromTypeScriptDiagnostic(diagnostics),
      emitMap,
      stats,
    };
  }

  function bundle({
    config,
    configPath,
    rootNames,
    target,
    unstable,
    cwd,
    sourceFileMap,
    type,
    performance,
  }) {
    if (performance) {
      performanceStart();
    }
    log(">>> bundle start", {
      rootNames,
      type: CompilerRequestType[type],
    });

    // When a programme is emitted, TypeScript will call `writeFile` with
    // each file that needs to be emitted.  The Deno compiler host delegates
    // this, to make it easier to perform the right actions, which vary
    // based a lot on the request.
    const state = {
      rootNames,
      bundleOutput: undefined,
    };
    const host = new Host({
      bundle: true,
      target,
      unstable,
      writeFile: createBundleWriteFile(state),
    });
    state.host = host;
    let diagnostics = [];

    // if there is a configuration supplied, we need to parse that
    if (config && config.length && configPath) {
      const configResult = host.configure(cwd, configPath, config);
      diagnostics = processConfigureResponse(configResult, configPath) || [];
    }

    buildSourceFileCache(sourceFileMap);
    // if there was a configuration and no diagnostics with it, we will continue
    // to generate the program and possibly emit it.
    if (diagnostics.length === 0) {
      const options = host.getCompilationSettings();
      const program = ts.createProgram({
        rootNames,
        options,
        host,
      });

      diagnostics = ts
        .getPreEmitDiagnostics(program)
        .filter(({ code }) => !ignoredDiagnostics.includes(code));

      // We will only proceed with the emit if there are no diagnostics.
      if (diagnostics.length === 0) {
        // we only support a single root module when bundling
        assert(rootNames.length === 1);
        setRootExports(program, rootNames[0]);
        const emitResult = program.emit();
        assert(
          emitResult.emitSkipped === false,
          "Unexpected skip of the emit.",
        );
        // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
        // without casting.
        diagnostics = emitResult.diagnostics;
      }
      if (performance) {
        performanceProgram({ program });
      }
    }

    let bundleOutput;

    if (diagnostics.length === 0) {
      assert(state.bundleOutput);
      bundleOutput = state.bundleOutput;
    }

    const stats = performance ? performanceEnd() : undefined;

    const result = {
      bundleOutput,
      diagnostics: fromTypeScriptDiagnostic(diagnostics),
      stats,
    };

    log("<<< bundle end", {
      rootNames,
      type: CompilerRequestType[type],
    });

    return result;
  }

  function runtimeCompile(request) {
    const { options, rootNames, target, unstable, sourceFileMap } = request;

    log(">>> runtime compile start", {
      rootNames,
    });

    // if there are options, convert them into TypeScript compiler options,
    // and resolve any external file references
    let convertedOptions;
    if (options) {
      const result = convertCompilerOptions(options);
      convertedOptions = result.options;
    }

    buildLocalSourceFileCache(sourceFileMap);

    const state = {
      rootNames,
      emitMap: {},
    };
    const host = new Host({
      bundle: false,
      target,
      writeFile: createRuntimeCompileWriteFile(state),
    });
    const compilerOptions = [DEFAULT_RUNTIME_COMPILE_OPTIONS];
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

    host.mergeOptions(...compilerOptions);

    const program = ts.createProgram({
      rootNames,
      options: host.getCompilationSettings(),
      host,
    });

    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !ignoredDiagnostics.includes(code));

    const emitResult = program.emit();

    assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

    log("<<< runtime compile finish", {
      rootNames,
      emitMap: Object.keys(state.emitMap),
    });

    const maybeDiagnostics = diagnostics.length
      ? fromTypeScriptDiagnostic(diagnostics).items
      : [];

    return {
      diagnostics: maybeDiagnostics,
      emitMap: state.emitMap,
    };
  }

  function runtimeBundle(request) {
    const { options, rootNames, target, unstable, sourceFileMap } = request;

    log(">>> runtime bundle start", {
      rootNames,
    });

    // if there are options, convert them into TypeScript compiler options,
    // and resolve any external file references
    let convertedOptions;
    if (options) {
      const result = convertCompilerOptions(options);
      convertedOptions = result.options;
    }

    buildLocalSourceFileCache(sourceFileMap);

    const state = {
      rootNames,
      bundleOutput: undefined,
    };
    const host = new Host({
      bundle: true,
      target,
      writeFile: createBundleWriteFile(state),
    });
    state.host = host;

    const compilerOptions = [DEFAULT_RUNTIME_COMPILE_OPTIONS];
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
    compilerOptions.push(DEFAULT_BUNDLER_OPTIONS);
    host.mergeOptions(...compilerOptions);

    const program = ts.createProgram({
      rootNames,
      options: host.getCompilationSettings(),
      host,
    });

    setRootExports(program, rootNames[0]);
    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !ignoredDiagnostics.includes(code));

    const emitResult = program.emit();

    assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

    log("<<< runtime bundle finish", {
      rootNames,
    });

    const maybeDiagnostics = diagnostics.length
      ? fromTypeScriptDiagnostic(diagnostics).items
      : [];

    return {
      diagnostics: maybeDiagnostics,
      output: state.bundleOutput,
    };
  }

  function runtimeTranspile(request) {
    const result = {};
    const { sources, options } = request;
    const compilerOptions = options
      ? Object.assign(
        {},
        DEFAULT_RUNTIME_TRANSPILE_OPTIONS,
        convertCompilerOptions(options).options,
      )
      : DEFAULT_RUNTIME_TRANSPILE_OPTIONS;

    for (const [fileName, inputText] of Object.entries(sources)) {
      const { outputText: source, sourceMapText: map } = ts.transpileModule(
        inputText,
        {
          fileName,
          compilerOptions,
        },
      );
      result[fileName] = { source, map };
    }
    return Promise.resolve(result);
  }

  function opCompilerRespond(msg) {
    dispatchJson.sendSync("op_compiler_respond", msg);
  }

  async function tsCompilerOnMessage(msg) {
    const request = msg.data;
    switch (request.type) {
      case CompilerRequestType.Compile: {
        const result = compile(request);
        opCompilerRespond(result);
        break;
      }
      case CompilerRequestType.Transpile: {
        const result = transpile(request);
        opCompilerRespond(result);
        break;
      }
      case CompilerRequestType.Bundle: {
        const result = bundle(request);
        opCompilerRespond(result);
        break;
      }
      case CompilerRequestType.RuntimeCompile: {
        const result = runtimeCompile(request);
        opCompilerRespond(result);
        break;
      }
      case CompilerRequestType.RuntimeBundle: {
        const result = runtimeBundle(request);
        opCompilerRespond(result);
        break;
      }
      case CompilerRequestType.RuntimeTranspile: {
        const result = await runtimeTranspile(request);
        opCompilerRespond(result);
        break;
      }
      default:
        throw new Error(
          `!!! unhandled CompilerRequestType: ${request.type} (${
            CompilerRequestType[request.type]
          })`,
        );
    }
  }

  // TODO(bartlomieju): temporary solution, must be fixed when moving
  // dispatches to separate crates
  function initOps() {
    const opsMap = core.ops();
    for (const [_name, opId] of Object.entries(opsMap)) {
      core.setAsyncHandler(opId, dispatchJson.asyncMsgFromRust);
    }
  }

  function runtimeStart(source) {
    initOps();
    // First we send an empty `Start` message to let the privileged side know we
    // are ready. The response should be a `StartRes` message containing the CLI
    // args and other info.
    const s = dispatchJson.sendSync("op_start");
    util.setLogDebug(s.debugFlag, source);
    errorStack.setPrepareStackTrace(Error);
    return s;
  }

  let hasBootstrapped = false;

  function bootstrapCompilerRuntime() {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    hasBootstrapped = true;
    core.registerErrorClass("NotFound", errors.NotFound);
    core.registerErrorClass("PermissionDenied", errors.PermissionDenied);
    core.registerErrorClass("ConnectionRefused", errors.ConnectionRefused);
    core.registerErrorClass("ConnectionReset", errors.ConnectionReset);
    core.registerErrorClass("ConnectionAborted", errors.ConnectionAborted);
    core.registerErrorClass("NotConnected", errors.NotConnected);
    core.registerErrorClass("AddrInUse", errors.AddrInUse);
    core.registerErrorClass("AddrNotAvailable", errors.AddrNotAvailable);
    core.registerErrorClass("BrokenPipe", errors.BrokenPipe);
    core.registerErrorClass("AlreadyExists", errors.AlreadyExists);
    core.registerErrorClass("InvalidData", errors.InvalidData);
    core.registerErrorClass("TimedOut", errors.TimedOut);
    core.registerErrorClass("Interrupted", errors.Interrupted);
    core.registerErrorClass("WriteZero", errors.WriteZero);
    core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
    core.registerErrorClass("BadResource", errors.BadResource);
    core.registerErrorClass("Http", errors.Http);
    core.registerErrorClass("URIError", URIError);
    core.registerErrorClass("TypeError", TypeError);
    core.registerErrorClass("Other", Error);
    core.registerErrorClass("Busy", errors.Busy);
    globalThis.__bootstrap = undefined;
    runtimeStart("TS");
  }

  globalThis.bootstrapCompilerRuntime = bootstrapCompilerRuntime;
  globalThis.tsCompilerOnMessage = tsCompilerOnMessage;
})(this);
