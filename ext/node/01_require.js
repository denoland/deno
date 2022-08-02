// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

"use strict";

((window) => {
  const {
    ArrayPrototypeIncludes,
    ArrayPrototypeSlice,
    ArrayPrototypePush,
    StringPrototypeEndsWith,
    StringPrototypeSlice,
    StringPrototypeIndexOf,
    SafeWeakMap,
  } = window.__bootstrap.primordials;

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  let requireDepth = 0;
  let statCache = null;
  let isPreloading = false;
  let mainModule = null;

  function updateChildren(parent, child, scan) {
    if (!parent) {
      return;
    }

    const children = parent.children;
    if (children && !(scan && ArrayPrototypeIncludes(children, child))) {
      ArrayPrototypePush(children, child);
    }
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

  Module.prototype._findPath = function (request, paths, isMain) {
    throw new Error("not implemented");
  };

  Module.prototype._nodeModulePaths = function (from) {
    return core.opSync("op_require_node_module_paths", from);
  };

  Module.prototype._resolveLookupPaths = function (request, parent) {
    throw new Error("not implemented");
  };

  Module.prototype._load = function (request, parent, isMain) {
    throw new Error("not implemented");
  };

  Module.prototype._resolveFilename = function (
    request,
    parent,
    isMain,
    options,
  ) {
    throw new Error("not implemented");
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
