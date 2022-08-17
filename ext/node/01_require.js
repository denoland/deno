// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

"use strict";

((window) => {
  const {
    ArrayIsArray,
    ArrayPrototypeIncludes,
    ArrayPrototypeIndexOf,
    ArrayPrototypeJoin,
    ArrayPrototypePush,
    ArrayPrototypeSlice,
    ArrayPrototypeSplice,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetPrototypeOf,
    ObjectEntries,
    ObjectPrototypeHasOwnProperty,
    ObjectSetPrototypeOf,
    ObjectKeys,
    ObjectPrototype,
    ObjectCreate,
    SafeMap,
    SafeWeakMap,
    JSONParse,
    StringPrototypeEndsWith,
    StringPrototypeIndexOf,
    StringPrototypeSlice,
    StringPrototypeStartsWith,
    StringPrototypeCharCodeAt,
    RegExpPrototypeTest,
  } = window.__bootstrap.primordials;
  const core = window.Deno.core;
  const ops = core.ops;

  // Map used to store CJS parsing data.
  const cjsParseCache = new SafeWeakMap();

  function pathDirname(filepath) {
    return ops.op_require_path_dirname(filepath);
  }

  function pathResolve(...args) {
    return ops.op_require_path_resolve(args);
  }

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  // TODO(bartlomieju): verify in other parts of this file that
  // we have initialized the system before making APIs work
  let cjsInitialized = false;
  let processGlobal = null;
  const nativeModulePolyfill = new SafeMap();
  const nativeModuleExports = ObjectCreate(null);

  const relativeResolveCache = ObjectCreate(null);
  let requireDepth = 0;
  let statCache = null;
  let isPreloading = false;
  let mainModule = null;

  function stat(filename) {
    // TODO: required only on windows
    // filename = path.toNamespacedPath(filename);
    if (statCache !== null) {
      const result = statCache.get(filename);
      if (result !== undefined) {
        return result;
      }
    }
    const result = ops.op_require_stat(filename);
    if (statCache !== null && result >= 0) {
      statCache.set(filename, result);
    }

    return result;
  }

  function updateChildren(parent, child, scan) {
    if (!parent) {
      return;
    }

    const children = parent.children;
    if (children && !(scan && ArrayPrototypeIncludes(children, child))) {
      ArrayPrototypePush(children, child);
    }
  }

  function tryFile(requestPath, _isMain) {
    const rc = stat(requestPath);
    if (rc !== 0) return;
    return toRealPath(requestPath);
  }

  function tryPackage(requestPath, exts, isMain, originalPath) {
    // const pkg = readPackage(requestPath)?.main;
    let pkg = false;

    if (!pkg) {
      return tryExtensions(
        pathResolve(requestPath, "index"),
        exts,
        isMain,
      );
    }

    const filename = path.resolve(requestPath, pkg);
    let actual = tryFile(filename, isMain) ||
      tryExtensions(filename, exts, isMain) ||
      tryExtensions(
        pathResolve(filename, "index"),
        exts,
        isMain,
      );
    if (actual === false) {
      actual = tryExtensions(
        pathResolve(requestPath, "index"),
        exts,
        isMain,
      );
      if (!actual) {
        // eslint-disable-next-line no-restricted-syntax
        const err = new Error(
          `Cannot find module '${filename}'. ` +
            'Please verify that the package.json has a valid "main" entry',
        );
        err.code = "MODULE_NOT_FOUND";
        err.path = pathResolve(
          requestPath,
          "package.json",
        );
        err.requestPath = originalPath;
        throw err;
      } else {
        const jsonPath = pathResolve(
          requestPath,
          "package.json",
        );
        process.emitWarning(
          `Invalid 'main' field in '${jsonPath}' of '${pkg}'. ` +
            "Please either fix that or report it to the module author",
          "DeprecationWarning",
          "DEP0128",
        );
      }
    }
    return actual;
  }

  const realpathCache = new SafeMap();
  function toRealPath(requestPath) {
    const maybeCached = realpathCache.get(requestPath);
    if (maybeCached) {
      return maybeCached;
    }
    const rp = ops.op_require_real_path(requestPath);
    realpathCache.set(requestPath, rp);
    return rp;
  }

  function tryExtensions(p, exts, isMain) {
    for (let i = 0; i < exts.length; i++) {
      const filename = tryFile(p + exts[i], isMain);

      if (filename) {
        return filename;
      }
    }
    return false;
  }

  // Find the longest (possibly multi-dot) extension registered in
  // Module._extensions
  function findLongestRegisteredExtension(filename) {
    const name = ops.op_require_path_basename(filename);
    let currentExtension;
    let index;
    let startIndex = 0;
    while ((index = StringPrototypeIndexOf(name, ".", startIndex)) !== -1) {
      startIndex = index + 1;
      if (index === 0) continue; // Skip dotfiles like .gitignore
      currentExtension = StringPrototypeSlice(name, index);
      if (Module._extensions[currentExtension]) {
        return currentExtension;
      }
    }
    return ".js";
  }

  function getExportsForCircularRequire(module) {
    if (
      module.exports &&
      ObjectGetPrototypeOf(module.exports) === ObjectPrototype &&
      // Exclude transpiled ES6 modules / TypeScript code because those may
      // employ unusual patterns for accessing 'module.exports'. That should
      // be okay because ES6 modules have a different approach to circular
      // dependencies anyway.
      !module.exports.__esModule
    ) {
      // This is later unset once the module is done loading.
      ObjectSetPrototypeOf(
        module.exports,
        CircularRequirePrototypeWarningProxy,
      );
    }

    return module.exports;
  }

  function emitCircularRequireWarning(prop) {
    processGlobal.emitWarning(
      `Accessing non-existent property '${String(prop)}' of module exports ` +
        "inside circular dependency",
    );
  }

  // A Proxy that can be used as the prototype of a module.exports object and
  // warns when non-existent properties are accessed.
  const CircularRequirePrototypeWarningProxy = new Proxy({}, {
    get(target, prop) {
      // Allow __esModule access in any case because it is used in the output
      // of transpiled code to determine whether something comes from an
      // ES module, and is not used as a regular key of `module.exports`.
      if (prop in target || prop === "__esModule") return target[prop];
      emitCircularRequireWarning(prop);
      return undefined;
    },

    getOwnPropertyDescriptor(target, prop) {
      if (
        ObjectPrototypeHasOwnProperty(target, prop) || prop === "__esModule"
      ) {
        return ObjectGetOwnPropertyDescriptor(target, prop);
      }
      emitCircularRequireWarning(prop);
      return undefined;
    },
  });

  const moduleParentCache = new SafeWeakMap();
  function Module(id = "", parent) {
    this.id = id;
    this.path = pathDirname(id);
    this.exports = {};
    moduleParentCache.set(this, parent);
    updateChildren(parent, this, false);
    this.filename = null;
    this.loaded = false;
    this.children = [];
  }

  const builtinModules = [];
  Module.builtinModules = builtinModules;

  Module._extensions = Object.create(null);
  Module._cache = Object.create(null);
  Module._pathCache = Object.create(null);
  let modulePaths = [];
  Module.globalPaths = modulePaths;

  const CHAR_FORWARD_SLASH = 47;
  const TRAILING_SLASH_REGEX = /(?:^|\/)\.?\.$/;
  Module._findPath = function (request, paths, isMain) {
    const absoluteRequest = ops.op_require_path_is_absolute(request);
    if (absoluteRequest) {
      paths = [""];
    } else if (!paths || paths.length === 0) {
      return false;
    }

    const cacheKey = request + "\x00" + ArrayPrototypeJoin(paths, "\x00");
    const entry = Module._pathCache[cacheKey];
    if (entry) {
      return entry;
    }

    let exts;
    let trailingSlash = request.length > 0 &&
      StringPrototypeCharCodeAt(request, request.length - 1) ===
        CHAR_FORWARD_SLASH;
    if (!trailingSlash) {
      trailingSlash = RegExpPrototypeTest(TRAILING_SLASH_REGEX, request);
    }

    // For each path
    for (let i = 0; i < paths.length; i++) {
      // Don't search further if path doesn't exist
      const curPath = paths[i];
      if (curPath && stat(curPath) < 1) continue;

      if (!absoluteRequest) {
        const exportsResolved = resolveExports(curPath, request);
        if (exportsResolved) {
          return exportsResolved;
        }
      }

      const basePath = pathResolve(curPath, request);
      let filename;

      const rc = stat(basePath);
      if (!trailingSlash) {
        if (rc === 0) { // File.
          if (!isMain) {
            filename = toRealPath(basePath);
          } else {
            filename = toRealPath(basePath);
          }
        }

        if (!filename) {
          // Try it with each of the extensions
          if (exts === undefined) {
            exts = ObjectKeys(Module._extensions);
          }
          filename = tryExtensions(basePath, exts, isMain);
        }
      }

      if (!filename && rc === 1) { // Directory.
        // try it with each of the extensions at "index"
        if (exts === undefined) {
          exts = ObjectKeys(Module._extensions);
        }
        filename = tryPackage(basePath, exts, isMain, request);
      }

      if (filename) {
        Module._pathCache[cacheKey] = filename;
        return filename;
      }
    }

    return false;
  };

  Module._nodeModulePaths = function (from) {
    return ops.op_require_node_module_paths(from);
  };

  Module._resolveLookupPaths = function (request, parent) {
    return ops.op_require_resolve_lookup_paths(
      request,
      parent?.paths,
      parent?.filename ?? "",
    );
  };

  Module._load = function (request, parent, isMain) {
    let relResolveCacheIdentifier;
    if (parent) {
      // Fast path for (lazy loaded) modules in the same directory. The indirect
      // caching is required to allow cache invalidation without changing the old
      // cache key names.
      relResolveCacheIdentifier = `${parent.path}\x00${request}`;
      const filename = relativeResolveCache[relResolveCacheIdentifier];
      if (filename !== undefined) {
        const cachedModule = Module._cache[filename];
        if (cachedModule !== undefined) {
          updateChildren(parent, cachedModule, true);
          if (!cachedModule.loaded) {
            return getExportsForCircularRequire(cachedModule);
          }
          return cachedModule.exports;
        }
        delete relativeResolveCache[relResolveCacheIdentifier];
      }
    }

    const filename = Module._resolveFilename(request, parent, isMain);
    if (StringPrototypeStartsWith(filename, "node:")) {
      // Slice 'node:' prefix
      const id = StringPrototypeSlice(filename, 5);

      const module = loadNativeModule(id, id);
      if (!module) {
        // TODO:
        // throw new ERR_UNKNOWN_BUILTIN_MODULE(filename);
        throw new Error("Unknown built-in module");
      }

      return module.exports;
    }

    const cachedModule = Module._cache[filename];
    if (cachedModule !== undefined) {
      updateChildren(parent, cachedModule, true);
      if (!cachedModule.loaded) {
        const parseCachedModule = cjsParseCache.get(cachedModule);
        if (!parseCachedModule || parseCachedModule.loaded) {
          return getExportsForCircularRequire(cachedModule);
        }
        parseCachedModule.loaded = true;
      } else {
        return cachedModule.exports;
      }
    }

    const mod = loadNativeModule(filename, request);
    if (
      mod
    ) {
      return mod.exports;
    }

    // Don't call updateChildren(), Module constructor already does.
    const module = cachedModule || new Module(filename, parent);

    if (isMain) {
      processGlobal.mainModule = module;
      module.id = ".";
    }

    Module._cache[filename] = module;
    if (parent !== undefined) {
      relativeResolveCache[relResolveCacheIdentifier] = filename;
    }

    let threw = true;
    try {
      module.load(filename);
      threw = false;
    } finally {
      if (threw) {
        delete Module._cache[filename];
        if (parent !== undefined) {
          delete relativeResolveCache[relResolveCacheIdentifier];
          const children = parent?.children;
          if (ArrayIsArray(children)) {
            const index = ArrayPrototypeIndexOf(children, module);
            if (index !== -1) {
              ArrayPrototypeSplice(children, index, 1);
            }
          }
        }
      } else if (
        module.exports &&
        ObjectGetPrototypeOf(module.exports) ===
          CircularRequirePrototypeWarningProxy
      ) {
        ObjectSetPrototypeOf(module.exports, ObjectPrototype);
      }
    }

    return module.exports;
  };

  Module._resolveFilename = function (
    request,
    parent,
    isMain,
    options,
  ) {
    if (
      StringPrototypeStartsWith(request, "node:") ||
      nativeModuleCanBeRequiredByUsers(request)
    ) {
      return request;
    }

    let paths;

    if (typeof options === "object" && options !== null) {
      if (ArrayIsArray(options.paths)) {
        const isRelative = ops.op_require_is_request_relative(
          request,
        );

        if (isRelative) {
          paths = options.paths;
        } else {
          const fakeParent = new Module("", null);

          paths = [];

          for (let i = 0; i < options.paths.length; i++) {
            const path = options.paths[i];
            fakeParent.paths = Module._nodeModulePaths(path);
            const lookupPaths = Module._resolveLookupPaths(request, fakeParent);

            for (let j = 0; j < lookupPaths.length; j++) {
              if (!ArrayPrototypeIncludes(paths, lookupPaths[j])) {
                ArrayPrototypePush(paths, lookupPaths[j]);
              }
            }
          }
        }
      } else if (options.paths === undefined) {
        paths = Module._resolveLookupPaths(request, parent);
      } else {
        // TODO:
        // throw new ERR_INVALID_ARG_VALUE("options.paths", options.paths);
        throw new Error("Invalid arg value options.paths", options.path);
      }
    } else {
      paths = Module._resolveLookupPaths(request, parent);
    }

    if (parent?.filename) {
      if (request[0] === "#") {
        console.log("TODO: Module._resolveFilename with #specifier");
        // const pkg = readPackageScope(parent.filename) || {};
        // if (pkg.data?.imports != null) {
        //   try {
        //     return finalizeEsmResolution(
        //       packageImportsResolve(
        //         request,
        //         pathToFileURL(parent.filename),
        //         cjsConditions,
        //       ),
        //       parent.filename,
        //       pkg.path,
        //     );
        //   } catch (e) {
        //     if (e.code === "ERR_MODULE_NOT_FOUND") {
        //       throw createEsmNotFoundErr(request);
        //     }
        //     throw e;
        //   }
        // }
      }
    }

    // Try module self resolution first
    // TODO(bartlomieju): make into a single op
    const parentPath = ops.op_require_try_self_parent_path(
      !!parent,
      parent?.filename,
      parent?.id,
    );
    // const selfResolved = ops.op_require_try_self(parentPath, request);
    const selfResolved = false;
    if (selfResolved) {
      const cacheKey = request + "\x00" +
        (paths.length === 1 ? paths[0] : ArrayPrototypeJoin(paths, "\x00"));
      Module._pathCache[cacheKey] = selfResolved;
      return selfResolved;
    }

    // Look up the filename first, since that's the cache key.
    const filename = Module._findPath(request, paths, isMain, false);
    if (filename) return filename;
    const requireStack = [];
    for (let cursor = parent; cursor; cursor = moduleParentCache.get(cursor)) {
      ArrayPrototypePush(requireStack, cursor.filename || cursor.id);
    }
    let message = `Cannot find module '${request}'`;
    if (requireStack.length > 0) {
      message = message + "\nRequire stack:\n- " +
        ArrayPrototypeJoin(requireStack, "\n- ");
    }
    // eslint-disable-next-line no-restricted-syntax
    const err = new Error(message);
    err.code = "MODULE_NOT_FOUND";
    err.requireStack = requireStack;
    throw err;
  };

  Module.prototype.load = function (filename) {
    assert(!this.loaded);
    this.filename = filename;
    this.paths = Module._nodeModulePaths(
      pathDirname(filename),
    );
    const extension = findLongestRegisteredExtension(filename);
    // allow .mjs to be overriden
    if (
      StringPrototypeEndsWith(filename, ".mjs") && !Module._extensions[".mjs"]
    ) {
      // TODO: use proper error class
      throw new Error("require ESM", filename);
    }

    Module._extensions[extension](this, filename);
    this.loaded = true;

    // TODO: do caching
  };

  // Loads a module at the given file path. Returns that module's
  // `exports` property.
  Module.prototype.require = function (id) {
    if (typeof id !== "string") {
      // TODO(bartlomieju): it should use different error type
      // ("ERR_INVALID_ARG_VALUE")
      throw new TypeError("Invalid argument type");
    }

    if (id === "") {
      // TODO(bartlomieju): it should use different error type
      // ("ERR_INVALID_ARG_VALUE")
      throw new TypeError("id must be non empty");
    }
    requireDepth++;
    try {
      return Module._load(id, this, /* isMain */ false);
    } finally {
      requireDepth--;
    }
  };

  Module.wrapper = [
    // TODO:
    // We provide non standard timer APIs in the CommonJS wrapper
    // to avoid exposing them in global namespace.
    "(function (exports, require, module, __filename, __dirname, setTimeout, clearTimeout, setInterval, clearInterval) { (function (exports, require, module, __filename, __dirname) {",
    "\n}).call(this, exports, require, module, __filename, __dirname); })",
  ];
  Module.wrap = function (script) {
    script = script.replace(/^#!.*?\n/, "");
    return `${Module.wrapper[0]}${script}${Module.wrapper[1]}`;
  };

  function enrichCJSError(error) {
    if (error instanceof SyntaxError) {
      if (
        error.message.includes(
          "Cannot use import statement outside a module",
        ) ||
        error.message.includes("Unexpected token 'export'")
      ) {
        console.error(
          'To load an ES module, set "type": "module" in the package.json or use ' +
            "the .mjs extension.",
        );
      }
    }
  }

  function wrapSafe(
    filename,
    content,
    cjsModuleInstance,
  ) {
    const wrapper = Module.wrap(content);
    const [f, err] = core.evalContext(wrapper, filename);
    if (err) {
      if (processGlobal.mainModule === cjsModuleInstance) {
        enrichCJSError(err.thrown);
      }
      throw err.thrown;
    }
    return f;
  }

  Module.prototype._compile = function (content, filename) {
    const compiledWrapper = wrapSafe(filename, content, this);

    const dirname = pathDirname(filename);
    const require = makeRequireFunction(this);
    const exports = this.exports;
    const thisValue = exports;
    const module = this;
    if (requireDepth === 0) {
      statCache = new SafeMap();
    }
    const result = compiledWrapper.call(
      thisValue,
      exports,
      require,
      this,
      filename,
      dirname,
    );
    if (requireDepth === 0) {
      statCache = null;
    }
    return result;
  };

  Module._extensions[".js"] = function (module, filename) {
    const content = ops.op_require_read_file(filename);

    console.log(`TODO: Module._extensions[".js"] is ESM`);

    module._compile(content, filename);
  };

  function stripBOM(content) {
    if (content.charCodeAt(0) === 0xfeff) {
      content = content.slice(1);
    }
    return content;
  }

  // Native extension for .json
  Module._extensions[".json"] = function (module, filename) {
    const content = ops.op_require_read_file(filename);

    try {
      module.exports = JSONParse(stripBOM(content));
    } catch (err) {
      err.message = filename + ": " + err.message;
      throw err;
    }
  };

  // Native extension for .node
  Module._extensions[".node"] = function (module, filename) {
    throw new Error("not implemented loading .node files");
  };

  function createRequireFromPath(filename) {
    const proxyPath = ops.op_require_proxy_path(filename);
    const mod = new Module(proxyPath);
    mod.filename = proxyPath;
    mod.paths = Module._nodeModulePaths(mod.path);
    return makeRequireFunction(mod);
  }

  function makeRequireFunction(mod) {
    const require = function require(path) {
      return mod.require(path);
    };

    function resolve(request, options) {
      return Module._resolveFilename(request, mod, false, options);
    }

    require.resolve = resolve;

    function paths(request) {
      return Module._resolveLookupPaths(request, mod);
    }

    resolve.paths = paths;
    require.main = mainModule;
    // Enable support to add extra extension types.
    require.extensions = Module._extensions;
    require.cache = Module._cache;

    return require;
  }

  function createRequire(filename) {
    // FIXME: handle URLs and validation
    return createRequireFromPath(filename);
  }

  Module.createRequire = createRequire;

  Module._initPaths = function () {
    const paths = ops.op_require_init_paths();
    modulePaths = paths;
    Module.globalPaths = ArrayPrototypeSlice(modulePaths);
  };

  Module.syncBuiltinESMExports = function syncBuiltinESMExports() {
    throw new Error("not implemented");
  };

  Module.Module = Module;

  const m = {
    _cache: Module._cache,
    _extensions: Module._extensions,
    _findPath: Module._findPath,
    _initPaths: Module._initPaths,
    _load: Module._load,
    _nodeModulePaths: Module._nodeModulePaths,
    _pathCache: Module._pathCache,
    _preloadModules: Module._preloadModules,
    _resolveFilename: Module._resolveFilename,
    _resolveLookupPaths: Module._resolveLookupPaths,
    builtinModules: Module.builtinModules,
    createRequire: Module.createRequire,
    globalPaths: Module.globalPaths,
    Module,
    wrap: Module.wrap,
  };

  nativeModuleExports.module = m;

  function loadNativeModule(_id, request) {
    if (nativeModulePolyfill.has(request)) {
      return nativeModulePolyfill.get(request);
    }
    const modExports = nativeModuleExports[request];
    if (modExports) {
      const nodeMod = new Module(request);
      nodeMod.exports = modExports;
      nodeMod.loaded = true;
      nativeModulePolyfill.set(request, nodeMod);
      return nodeMod;
    }
    return undefined;
  }

  function nativeModuleCanBeRequiredByUsers(request) {
    return !!nativeModuleExports[request];
  }

  function initializeCommonJs(nodeModules, process) {
    assert(!cjsInitialized);
    cjsInitialized = true;
    for (const [name, exports] of ObjectEntries(nodeModules)) {
      nativeModuleExports[name] = exports;
      ArrayPrototypePush(Module.builtinModules, name);
    }
    processGlobal = process;
  }

  function readPackageScope() {
    throw new Error("not implemented");
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    require: {
      Module,
      wrapSafe,
      toRealPath,
      cjsParseCache,
      readPackageScope,
      initializeCommonJs,
    },
  };
})(globalThis);
