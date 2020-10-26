// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "compiler" isolate, ie. the one
// that is created when Deno needs to compile TS/WASM to JS.
//
// It provides two functions that should be called by Rust:
//  - `startup`
// This functions must be called when creating isolate
// to properly setup runtime.
//  - `tsCompilerOnMessage`
// This function must be called when sending a request
// to the compiler.

// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
delete Object.prototype.__proto__;

((window) => {
  const core = window.Deno.core;

  let logDebug = false;
  let logSource = "JS";

  /** Instructs the host to behave in a legacy fashion, with the legacy
   * pipeline for handling code.  Setting the value to `true` will cause the
   * host to behave in the modern way. */
  let legacy = true;

  function setLogDebug(debug, source) {
    logDebug = debug;
    if (source) {
      logSource = source;
    }
  }

  function debug(...args) {
    if (logDebug) {
      const stringifiedArgs = args.map((arg) => JSON.stringify(arg)).join(" ");
      core.print(`DEBUG ${logSource} - ${stringifiedArgs}\n`);
    }
  }

  class AssertionError extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
    }
  }

  /** @type {Map<string, ts.SourceFile>} */
  const sourceFileCache = new Map();

  /**
   * @param {import("../dts/typescript").DiagnosticRelatedInformation} diagnostic
   */
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
      messageText = msgText;
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

  /**
   * @param {import("../dts/typescript").Diagnostic[]} diagnostics 
   */
  function fromTypeScriptDiagnostic(diagnostics) {
    return diagnostics.map(({ relatedInformation: ri, source, ...diag }) => {
      const value = fromRelatedInformation(diag);
      value.relatedInformation = ri
        ? ri.map(fromRelatedInformation)
        : undefined;
      value.source = source;
      return value;
    });
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
  const ASSETS = "asset:///";
  const OUT_DIR = "deno://";
  const CACHE = "cache:///";
  // This constant is passed to compiler settings when
  // doing incremental compiles. Contents of this
  // file are passed back to Rust and saved to $DENO_DIR.
  const TS_BUILD_INFO = "cache:///tsbuildinfo.json";

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
    3: "Dts",
    4: "TSX",
    5: "Json",
    6: "Wasm",
    7: "TsBuildInfo",
    8: "SourceMap",
    9: "Unknown",
    JavaScript: 0,
    JSX: 1,
    TypeScript: 2,
    Dts: 3,
    TSX: 4,
    Json: 5,
    Wasm: 6,
    TsBuildInfo: 7,
    SourceMap: 8,
    Unknown: 9,
  };

  function getExtension(fileName, mediaType) {
    switch (mediaType) {
      case MediaType.JavaScript:
        return ts.Extension.Js;
      case MediaType.JSX:
        return ts.Extension.Jsx;
      case MediaType.TypeScript:
        return ts.Extension.Ts;
      case MediaType.Dts:
        return ts.Extension.Dts;
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

  function parseCompilerOptions(compilerOptions) {
    const { options, errors } = ts.convertCompilerOptionsFromJson(
      compilerOptions,
      "",
      "tsconfig.json",
    );
    return {
      options,
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

  /** There was some private state in the legacy host, that is moved out to
   * here which can then be refactored out later. */
  const legacyHostState = {
    buildInfo: "",
    target: CompilerHostTarget.Main,
    writeFile: (_fileName, _data, _sourceFiles) => {},
  };

  /** @type {import("../dts/typescript").CompilerHost} */
  const host = {
    fileExists(fileName) {
      debug(`host.fileExists("${fileName}")`);
      return false;
    },
    readFile(specifier) {
      debug(`host.readFile("${specifier}")`);
      if (legacy) {
        if (specifier == TS_BUILD_INFO) {
          return legacyHostState.buildInfo;
        }
        return unreachable();
      } else {
        return core.jsonOpSync("op_load", { specifier }).data;
      }
    },
    getSourceFile(
      specifier,
      languageVersion,
      onError,
      shouldCreateNewSourceFile,
    ) {
      debug(
        `host.getSourceFile("${specifier}", ${
          ts.ScriptTarget[languageVersion]
        })`,
      );
      if (legacy) {
        try {
          assert(!shouldCreateNewSourceFile);
          const sourceFile = specifier.startsWith(ASSETS)
            ? getAssetInternal(specifier)
            : SourceFile.getCached(specifier);
          assert(sourceFile != null);
          if (!sourceFile.tsSourceFile) {
            assert(sourceFile.sourceCode != null);
            const tsSourceFileName = specifier.startsWith(ASSETS)
              ? sourceFile.filename
              : specifier;

            sourceFile.tsSourceFile = ts.createSourceFile(
              tsSourceFileName,
              sourceFile.sourceCode,
              languageVersion,
            );
            sourceFile.tsSourceFile.version = sourceFile.versionHash;
            delete sourceFile.sourceCode;

            // This code is to support transition from the "legacy" compiler
            // to the new one, by populating the new source file cache.
            if (
              !sourceFileCache.has(specifier) && specifier.startsWith(ASSETS)
            ) {
              sourceFileCache.set(specifier, sourceFile.tsSourceFile);
            }
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
      } else {
        let sourceFile = sourceFileCache.get(specifier);
        if (sourceFile) {
          return sourceFile;
        }

        /** @type {{ data: string; hash: string; }} */
        const { data, hash, scriptKind } = core.jsonOpSync(
          "op_load",
          { specifier },
        );
        assert(data, `"data" is unexpectedly null for "${specifier}".`);
        sourceFile = ts.createSourceFile(
          specifier,
          data,
          languageVersion,
          false,
          scriptKind,
        );
        sourceFile.moduleName = specifier;
        sourceFile.version = hash;
        sourceFileCache.set(specifier, sourceFile);
        return sourceFile;
      }
    },
    getDefaultLibFileName() {
      if (legacy) {
        switch (legacyHostState.target) {
          case CompilerHostTarget.Main:
          case CompilerHostTarget.Runtime:
            return `${ASSETS}/lib.deno.window.d.ts`;
          case CompilerHostTarget.Worker:
            return `${ASSETS}/lib.deno.worker.d.ts`;
        }
      } else {
        return `${ASSETS}/lib.esnext.d.ts`;
      }
    },
    getDefaultLibLocation() {
      return ASSETS;
    },
    writeFile(fileName, data, _writeByteOrderMark, _onError, sourceFiles) {
      debug(`host.writeFile("${fileName}")`);
      if (legacy) {
        legacyHostState.writeFile(fileName, data, sourceFiles);
      } else {
        let maybeSpecifiers;
        if (sourceFiles) {
          maybeSpecifiers = sourceFiles.map((sf) => sf.moduleName);
        }
        return core.jsonOpSync(
          "op_emit",
          { maybeSpecifiers, fileName, data },
        );
      }
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
      debug(`host.resolveModuleNames()`);
      debug(`  base: ${base}`);
      debug(`  specifiers: ${specifiers.join(", ")}`);
      if (legacy) {
        const resolved = specifiers.map((specifier) => {
          const maybeUrl = SourceFile.getResolvedUrl(specifier, base);

          debug("compiler::host.resolveModuleNames maybeUrl", {
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
        debug(resolved);
        return resolved;
      } else {
        /** @type {Array<[string, import("../dts/typescript").Extension]>} */
        const resolved = core.jsonOpSync("op_resolve", {
          specifiers,
          base,
        });
        let r = resolved.map(([resolvedFileName, extension]) => ({
          resolvedFileName,
          extension,
          isExternalLibraryImport: false,
        }));
        return r;
      }
    },
    createHash(data) {
      return core.jsonOpSync("op_create_hash", { data }).hash;
    },
  };

  // This is a hacky way of adding our libs to the libs available in TypeScript()
  // as these are internal APIs of TypeScript which maintain valid libs
  ts.libs.push("deno.ns", "deno.window", "deno.worker", "deno.shared_globals");
  ts.libMap.set("deno.ns", "lib.deno.ns.d.ts");
  ts.libMap.set("deno.web", "lib.deno.web.d.ts");
  ts.libMap.set("deno.fetch", "lib.deno.fetch.d.ts");
  ts.libMap.set("deno.window", "lib.deno.window.d.ts");
  ts.libMap.set("deno.worker", "lib.deno.worker.d.ts");
  ts.libMap.set("deno.shared_globals", "lib.deno.shared_globals.d.ts");
  ts.libMap.set("deno.unstable", "lib.deno.unstable.d.ts");

  // TODO(@kitsonk) remove once added to TypeScript
  ts.libs.push("esnext.weakref");
  ts.libMap.set("esnext.weakref", "lib.esnext.weakref.d.ts");

  // this pre-populates the cache at snapshot time of our library files, so they
  // are available in the future when needed.
  host.getSourceFile(
    `${ASSETS}lib.deno.ns.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.web.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.fetch.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.window.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.worker.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.shared_globals.d.ts`,
    ts.ScriptTarget.ESNext,
  );
  host.getSourceFile(
    `${ASSETS}lib.deno.unstable.d.ts`,
    ts.ScriptTarget.ESNext,
  );

  // We never use this program; it's only created
  // during snapshotting to hydrate and populate
  // source file cache with lib declaration files.
  const _TS_SNAPSHOT_PROGRAM = ts.createProgram({
    rootNames: [`${ASSETS}bootstrap.ts`],
    options: DEFAULT_COMPILE_OPTIONS,
    host,
  });

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

  // Warning! The values in this enum are duplicated in `cli/msg.rs`
  // Update carefully!
  const CompilerRequestType = {
    RuntimeCompile: 2,
    RuntimeBundle: 3,
    RuntimeTranspile: 4,
  };

  function createBundleWriteFile(state) {
    return function writeFile(_fileName, data, sourceFiles) {
      assert(sourceFiles != null);
      assert(state.options);
      // we only support single root names for bundles
      assert(state.rootNames.length === 1);
      state.bundleOutput = buildBundle(
        state.rootNames[0],
        data,
        sourceFiles,
        state.options.target ?? ts.ScriptTarget.ESNext,
      );
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

  const IGNORED_DIAGNOSTICS = [
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

  const IGNORED_COMPILE_DIAGNOSTICS = [
    // TS1208: All files must be modules when the '--isolatedModules' flag is
    // provided.  We can ignore because we guarantuee that all files are
    // modules.
    1208,
  ];

  /** @type {Array<{ key: string, value: number }>} */
  const stats = [];
  let statsStart = 0;

  function performanceStart() {
    stats.length = 0;
    // TODO(kitsonk) replace with performance.mark() when landed
    statsStart = new Date();
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
    const duration = new Date() - statsStart;
    stats.push({ key: "Compile time", value: duration });
    return stats;
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

  function runtimeCompile(request) {
    const { compilerOptions, rootNames, target, sourceFileMap } = request;

    debug(">>> runtime compile start", {
      rootNames,
    });

    // if there are options, convert them into TypeScript compiler options,
    // and resolve any external file references
    const result = parseCompilerOptions(
      compilerOptions,
    );
    const options = result.options;
    // TODO(bartlomieju): this options is excluded by `ts.convertCompilerOptionsFromJson`
    // however stuff breaks if it's not passed (type_directives_js_main.js, compiler_js_error.ts)
    options.allowNonTsExtensions = true;

    buildLocalSourceFileCache(sourceFileMap);

    const state = {
      rootNames,
      emitMap: {},
    };
    legacyHostState.target = target;
    legacyHostState.writeFile = createRuntimeCompileWriteFile(state);
    const program = ts.createProgram({
      rootNames,
      options,
      host,
    });

    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) =>
        !IGNORED_DIAGNOSTICS.includes(code) &&
        !IGNORED_COMPILE_DIAGNOSTICS.includes(code)
      );

    const emitResult = program.emit();
    assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

    debug("<<< runtime compile finish", {
      rootNames,
      emitMap: Object.keys(state.emitMap),
    });

    const maybeDiagnostics = diagnostics.length
      ? fromTypeScriptDiagnostic(diagnostics)
      : [];

    return {
      diagnostics: maybeDiagnostics,
      emitMap: state.emitMap,
    };
  }

  function runtimeBundle(request) {
    const { compilerOptions, rootNames, target, sourceFileMap } = request;

    debug(">>> runtime bundle start", {
      rootNames,
    });

    // if there are options, convert them into TypeScript compiler options,
    // and resolve any external file references
    const result = parseCompilerOptions(
      compilerOptions,
    );
    const options = result.options;
    // TODO(bartlomieju): this options is excluded by `ts.convertCompilerOptionsFromJson`
    // however stuff breaks if it's not passed (type_directives_js_main.js, compiler_js_error.ts)
    options.allowNonTsExtensions = true;

    buildLocalSourceFileCache(sourceFileMap);

    const state = {
      rootNames,
      bundleOutput: undefined,
    };

    legacyHostState.target = target;
    legacyHostState.writeFile = createBundleWriteFile(state);
    state.options = options;

    const program = ts.createProgram({
      rootNames,
      options,
      host,
    });

    setRootExports(program, rootNames[0]);
    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .filter(({ code }) => !IGNORED_DIAGNOSTICS.includes(code));

    const emitResult = program.emit();

    assert(emitResult.emitSkipped === false, "Unexpected skip of the emit.");

    debug("<<< runtime bundle finish", {
      rootNames,
    });

    const maybeDiagnostics = diagnostics.length
      ? fromTypeScriptDiagnostic(diagnostics)
      : [];

    return {
      diagnostics: maybeDiagnostics,
      output: state.bundleOutput,
    };
  }

  function runtimeTranspile(request) {
    const result = {};
    const { sources, compilerOptions } = request;

    const parseResult = parseCompilerOptions(
      compilerOptions,
    );
    const options = parseResult.options;
    // TODO(bartlomieju): this options is excluded by `ts.convertCompilerOptionsFromJson`
    // however stuff breaks if it's not passed (type_directives_js_main.js, compiler_js_error.ts)
    options.allowNonTsExtensions = true;

    for (const [fileName, inputText] of Object.entries(sources)) {
      const { outputText: source, sourceMapText: map } = ts.transpileModule(
        inputText,
        {
          fileName,
          compilerOptions: options,
        },
      );
      result[fileName] = { source, map };
    }
    return result;
  }

  function opCompilerRespond(msg) {
    core.jsonOpSync("op_compiler_respond", msg);
  }

  function tsCompilerOnMessage(msg) {
    const request = msg.data;
    switch (request.type) {
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
        const result = runtimeTranspile(request);
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

  /**
   * @typedef {object} Request
   * @property {Record<string, any>} config
   * @property {boolean} debug
   * @property {string[]} rootNames
   */

  /** The API that is called by Rust when executing a request.
   * @param {Request} request 
   */
  function exec({ config, debug: debugFlag, rootNames }) {
    setLogDebug(debugFlag, "TS");
    performanceStart();
    debug(">>> exec start", { rootNames });
    debug(config);

    const { options, errors: configFileParsingDiagnostics } = ts
      .convertCompilerOptionsFromJson(config, "", "tsconfig.json");
    const program = ts.createIncrementalProgram({
      rootNames,
      options,
      host,
      configFileParsingDiagnostics,
    });

    const { diagnostics: emitDiagnostics } = program.emit();

    const diagnostics = [
      ...program.getConfigFileParsingDiagnostics(),
      ...program.getSyntacticDiagnostics(),
      ...program.getOptionsDiagnostics(),
      ...program.getGlobalDiagnostics(),
      ...program.getSemanticDiagnostics(),
      ...emitDiagnostics,
    ].filter(({ code }) =>
      !IGNORED_DIAGNOSTICS.includes(code) &&
      !IGNORED_COMPILE_DIAGNOSTICS.includes(code)
    );
    performanceProgram({ program });

    // TODO(@kitsonk) when legacy stats are removed, convert to just tuples
    let stats = performanceEnd().map(({ key, value }) => [key, value]);
    core.jsonOpSync("op_respond", {
      diagnostics: fromTypeScriptDiagnostic(diagnostics),
      stats,
    });
    debug("<<< exec stop");
  }

  let hasStarted = false;

  /** Startup the runtime environment, setting various flags.
   * @param {{ debugFlag?: boolean; legacyFlag?: boolean; }} msg 
   */
  function startup({ debugFlag = false, legacyFlag = true }) {
    if (hasStarted) {
      throw new Error("The compiler runtime already started.");
    }
    hasStarted = true;
    core.ops();
    core.registerErrorClass("Error", Error);
    setLogDebug(!!debugFlag, "TS");
    legacy = legacyFlag;
  }

  globalThis.startup = startup;
  globalThis.exec = exec;
  // TODO(@kitsonk) remove when converted from legacy tsc
  globalThis.tsCompilerOnMessage = tsCompilerOnMessage;
})(this);
