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
    ObjectPrototypeHasOwnProperty,
    ObjectSetPrototypeOf,
    SafeMap,
    SafeWeakMap,
    StringPrototypeEndsWith,
    StringPrototypeIndexOf,
    StringPrototypeSlice,
    StringPrototypeStartsWith,
    StringPrototypeCharCodeAt,
    RegExpPrototypeTest,
  } = window.__bootstrap.primordials;
  const core = window.Deno.core;

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  // TODO:
  function isProxy() {
    return false;
  }

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
    const result = core.opSync("op_require_stat", filename);
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

  const realpathCache = new SafeMap();
  function toRealPath(requestPath) {
    const maybeCached = realpathCache.get(requestPath)
    if (maybeCached) {
      return maybeCached;
    }
    const rp = core.opSync("op_require_real_path", requestPath);
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
    // TODO: get basename
    const name = path.basename(filename);
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
      !isProxy(module.exports) &&
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

  // A Proxy that can be used as the prototype of a module.exports object and
  // warns when non-existent properties are accessed.
  const CircularRequirePrototypeWarningProxy = new Proxy({}, {
    get(target, prop) {
      // Allow __esModule access in any case because it is used in the output
      // of transpiled code to determine whether something comes from an
      // ES module, and is not used as a regular key of `module.exports`.
      if (prop in target || prop === "__esModule") return target[prop];
      // TODO:
      // emitCircularRequireWarning(prop);
      console.log("TODO: emitCircularRequireWarning");
      return undefined;
    },

    getOwnPropertyDescriptor(target, prop) {
      if (
        ObjectPrototypeHasOwnProperty(target, prop) || prop === "__esModule"
      ) {
        return ObjectGetOwnPropertyDescriptor(target, prop);
      }
      // TODO:
      // emitCircularRequireWarning(prop);
      console.log("TODO: emitCircularRequireWarning");
      return undefined;
    },
  });

  function loadNativeModule() {
    console.log("TODO: loadNativeModule");
    return undefined;
  }

  const moduleParentCache = new SafeWeakMap();
  function Module(id = "", parent) {
    this.id = id;
    this.path = path.dirname(id);
    this.exports = {};
    moduleParentCache.set(this, parent);
    updateChildren(parent, this, false);
    this.filename = null;
    this.loaded = false;
    this.children = [];
  }

  const builtinModules = [];
  // TODO(bartlomieju): handle adding native modules
  Module.builtinModules = builtinModules;

  Module._extensions = Object.create(null);
  Module._cache = Object.create(null);
  Module._pathCache = Object.create(null);
  let modulePaths = [];
  Module.globalPaths = modulePaths;

  const CHAR_FORWARD_SLASH = 47;
  const TRAILING_SLASH_REGEX = /(?:^|\/)\.?\.$/;
  Module._findPath = function (request, paths, isMain) {
    const absoluteRequest = core.opSync("op_require_path_is_absolute", request);
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

      const basePath = path.resolve(curPath, request);
      let filename;

      const rc = stat(basePath);
      if (!trailingSlash) {
        if (rc === 0) { // File.
          if (!isMain) {
            if (preserveSymlinks) {
              filename = path.resolve(basePath);
            } else {
              filename = toRealPath(basePath);
            }
          } else if (preserveSymlinksMain) {
            // For the main module, we use the preserveSymlinksMain flag instead
            // mainly for backward compatibility, as the preserveSymlinks flag
            // historically has not applied to the main module.  Most likely this
            // was intended to keep .bin/ binaries working, as following those
            // symlinks is usually required for the imports in the corresponding
            // files to resolve; that said, in some use cases following symlinks
            // causes bigger problems which is why the preserveSymlinksMain option
            // is needed.
            filename = path.resolve(basePath);
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
    return core.opSync("op_require_node_module_paths", from);
  };

  Module._resolveLookupPaths = function (request, parent) {
    console.log("TODO: Module._resolveLookupPaths NativeModule");
    return core.opSync(
      "op_require_resolve_lookup_paths",
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

      const module = loadNativeModule(id, request);
      if (!module?.canBeRequiredByUsers) {
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
      mod?.canBeRequiredByUsers &&
      NativeModule.canBeRequiredWithoutScheme(filename)
    ) {
      return mod.exports;
    }

    // Don't call updateChildren(), Module constructor already does.
    const module = cachedModule || new Module(filename, parent);

    if (isMain) {
      console.log("TODO: isMain CJS module not handled");
      // process.mainModule = module;
      // module.id = '.';
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
        !isProxy(module.exports) &&
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
    // TODO:
    // if (StringPrototypeStartsWith(request, 'node:') ||
    //   (NativeModule.canBeRequiredByUsers(request) &&
    //   NativeModule.canBeRequiredWithoutScheme(request))) {
    if (StringPrototypeStartsWith(request, "node:")) {
      return request;
    }

    let paths;

    if (typeof options === "object" && options !== null) {
      if (ArrayIsArray(options.paths)) {
        const isRelative = core.opSync(
          "op_require_specifier_is_relative",
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
    const parentPath = core.opSync(
      "op_require_try_self_parent_path",
      !!parent,
      parent?.filename,
      parent?.id,
    );
    // const selfResolved = core.opSync("op_require_try_self", parentPath, request);
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
    // TODO: get dirname here
    this.paths = Module._nodeModulePaths(filename);
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

  Module.prototype._compile = function (content, filename) {
    throw new Error("not implemented");
  };

  Module._extensions[".js"] = function (module, filename) {
    throw new Error("not implemented");
  };

  // Native extension for .json
  Module._extensions[".json"] = function (module, filename) {
    throw new Error("not implemented");
  };

  // Native extension for .node
  Module._extensions[".node"] = function (module, filename) {
    throw new Error("not implemented");
  };

  function createRequireFromPath(filename) {
    const proxyPath = core.opSync("op_require_proxy_path", filename);
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
    const paths = core.opSync("op_require_init_paths");
    modulePaths = paths;
    Module.globalPaths = ArrayPrototypeSlice(modulePaths);
  };

  Module.syncBuiltinESMExports = function syncBuiltinESMExports() {
    throw new Error("not implemented");
  };

  Module.Module = Module;

  window.__bootstrap.require = {
    Module,
  };
})(globalThis);
