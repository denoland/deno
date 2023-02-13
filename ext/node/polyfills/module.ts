// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import "./global.ts";

import { TextDecoder } from "internal:deno_web/08_text_encoding.js";
import { core } from "internal:deno_node/polyfills/_core.ts";
import nodeMods from "internal:deno_node/polyfills/module_all.ts";
import upstreamMods from "internal:deno_node/polyfills/upstream_modules.ts";

import * as path from "internal:deno_node/polyfills/path.ts";
import { assert } from "internal:deno_node/polyfills/_util/asserts.ts";
import {
  fileURLToPath,
  pathToFileURL,
} from "internal:deno_node/polyfills/url.ts";
import { isWindows } from "internal:deno_node/polyfills/_util/os.ts";
import {
  ERR_INVALID_MODULE_SPECIFIER,
  ERR_MODULE_NOT_FOUND,
  NodeError,
} from "internal:deno_node/polyfills/internal/errors.ts";
import type { PackageConfig } from "internal:deno_node/polyfills/module_esm.ts";
import {
  encodedSepRegEx,
  packageExportsResolve,
  packageImportsResolve,
} from "internal:deno_node/polyfills/module_esm.ts";
import {
  clearInterval,
  clearTimeout,
  setInterval,
  setTimeout,
} from "internal:deno_node/polyfills/timers.ts";

const { hasOwn } = Object;
const CHAR_FORWARD_SLASH = "/".charCodeAt(0);
const CHAR_BACKWARD_SLASH = "\\".charCodeAt(0);
const CHAR_COLON = ":".charCodeAt(0);

const relativeResolveCache = Object.create(null);

let requireDepth = 0;
let statCache: Map<string, StatResult> | null = null;

type StatResult = -1 | 0 | 1;
// Returns 0 if the path refers to
// a file, 1 when it's a directory or < 0 on error.
function stat(filename: string): StatResult {
  filename = path.toNamespacedPath(filename);
  if (statCache !== null) {
    const result = statCache.get(filename);
    if (result !== undefined) return result;
  }
  try {
    const info = Deno.statSync(filename);
    const result = info.isFile ? 0 : 1;
    if (statCache !== null) statCache.set(filename, result);
    return result;
  } catch (e) {
    if (e instanceof Deno.errors.PermissionDenied) {
      throw new Error("CJS loader requires --allow-read.");
    }
    return -1;
  }
}

function updateChildren(
  parent: Module | null,
  child: Module,
  scan: boolean,
) {
  const children = parent && parent.children;
  if (children && !(scan && children.includes(child))) {
    children.push(child);
  }
}

function finalizeEsmResolution(
  resolved: string,
  parentPath: string,
  pkgPath: string,
) {
  if (encodedSepRegEx.test(resolved)) {
    throw new ERR_INVALID_MODULE_SPECIFIER(
      resolved,
      'must not include encoded "/" or "\\" characters',
      parentPath,
    );
  }
  const filename = fileURLToPath(resolved);
  const actual = tryFile(filename, false);
  if (actual) {
    return actual;
  }
  throw new ERR_MODULE_NOT_FOUND(
    filename,
    path.resolve(pkgPath, "package.json"),
  );
}

function createEsmNotFoundErr(request: string, path?: string) {
  const err = new Error(`Cannot find module '${request}'`) as Error & {
    code: string;
    path?: string;
  };
  err.code = "MODULE_NOT_FOUND";
  if (path) {
    err.path = path;
  }
  return err;
}

function trySelfParentPath(parent: Module | undefined): string | undefined {
  if (!parent) return undefined;

  if (parent.filename) {
    return parent.filename;
  } else if (parent.id === "<repl>" || parent.id === "internal/preload") {
    try {
      return process.cwd() + path.sep;
    } catch {
      return undefined;
    }
  }

  return undefined;
}

function trySelf(parentPath: string | undefined, request: string) {
  if (!parentPath) return false;

  const { data: pkg, path: pkgPath } = readPackageScope(parentPath) ||
    { data: {}, path: "" };
  if (!pkg || pkg.exports === undefined) return false;
  if (typeof pkg.name !== "string") return false;

  let expansion;
  if (request === pkg.name) {
    expansion = ".";
  } else if (request.startsWith(`${pkg.name}/`)) {
    expansion = "." + request.slice(pkg.name.length);
  } else {
    return false;
  }

  try {
    return finalizeEsmResolution(
      packageExportsResolve(
        pathToFileURL(pkgPath + "/package.json").toString(),
        expansion,
        pkg as PackageConfig,
        pathToFileURL(parentPath).toString(),
        cjsConditions,
      ).toString(),
      parentPath,
      pkgPath,
    );
  } catch (e) {
    if (e instanceof NodeError && e.code === "ERR_MODULE_NOT_FOUND") {
      throw createEsmNotFoundErr(request, pkgPath + "/package.json");
    }
    throw e;
  }
}
class Module {
  id: string;
  // deno-lint-ignore no-explicit-any
  exports: any;
  parent: Module | null;
  filename: string | null;
  loaded: boolean;
  children: Module[];
  paths: string[];
  path: string;
  constructor(id = "", parent?: Module | null) {
    this.id = id;
    this.exports = {};
    this.parent = parent || null;
    updateChildren(parent || null, this, false);
    this.filename = null;
    this.loaded = false;
    this.children = [];
    this.paths = [];
    this.path = path.dirname(id);
  }
  static builtinModules: string[] = [];
  static _extensions: {
    // deno-lint-ignore no-explicit-any
    [key: string]: (module: Module, filename: string) => any;
  } = Object.create(null);
  static _cache: { [key: string]: Module } = Object.create(null);
  static _pathCache = Object.create(null);
  static globalPaths: string[] = [];
  static wrapper = [
    // We provide non standard timer APIs in the CommonJS wrapper
    // to avoid exposing them in global namespace.
    "(function (exports, require, module, __filename, __dirname, setTimeout, clearTimeout, setInterval, clearInterval) { (function (exports, require, module, __filename, __dirname) {",
    "\n}).call(this, exports, require, module, __filename, __dirname); })",
  ];
  // Loads a module at the given file path. Returns that module's
  // `exports` property.
  // deno-lint-ignore no-explicit-any
  require(id: string): any {
    if (id === "") {
      throw new Error(`id '${id}' must be a non-empty string`);
    }
    requireDepth++;
    try {
      return Module._load(id, this, /* isMain */ false);
    } finally {
      requireDepth--;
    }
  }

  // Given a file name, pass it to the proper extension handler.
  load(filename: string) {
    assert(!this.loaded);
    this.filename = filename;
    this.paths = Module._nodeModulePaths(path.dirname(filename));

    const extension = findLongestRegisteredExtension(filename);
    // Removed ESM code
    Module._extensions[extension](this, filename);
    this.loaded = true;
    // Removed ESM code
  }

  // Run the file contents in the correct scope or sandbox. Expose
  // the correct helper variables (require, module, exports) to
  // the file.
  // Returns exception, if any.
  // deno-lint-ignore no-explicit-any
  _compile(content: string, filename: string): any {
    // manifest code removed
    const compiledWrapper = wrapSafe(filename, content, this);
    // inspector code remove
    const dirname = path.dirname(filename);
    const require = makeRequireFunction(this);
    const exports = this.exports;
    const thisValue = exports;
    if (requireDepth === 0) {
      statCache = new Map();
    }
    const result = compiledWrapper.call(
      thisValue,
      exports,
      require,
      this,
      filename,
      dirname,
      setTimeout,
      clearTimeout,
      setInterval,
      clearInterval,
    );
    if (requireDepth === 0) {
      statCache = null;
    }
    return result;
  }

  /*
   * Check for node modules paths.
   */
  static _resolveLookupPaths(
    request: string,
    parent: Module | null,
  ): string[] | null {
    if (
      request.charAt(0) !== "." ||
      (request.length > 1 &&
        request.charAt(1) !== "." &&
        request.charAt(1) !== "/" &&
        (!isWindows || request.charAt(1) !== "\\"))
    ) {
      let paths = modulePaths;
      if (parent !== null && parent.paths && parent.paths.length) {
        paths = parent.paths.concat(paths);
      }

      return paths.length > 0 ? paths : null;
    }

    // With --eval, parent.id is not set and parent.filename is null.
    if (!parent || !parent.id || !parent.filename) {
      // Make require('./path/to/foo') work - normally the path is taken
      // from realpath(__filename) but with eval there is no filename
      return ["."].concat(Module._nodeModulePaths("."), modulePaths);
    }
    // Returns the parent path of the file
    return [path.dirname(parent.filename)];
  }

  static _resolveFilename(
    request: string,
    parent: Module,
    isMain: boolean,
    options?: { paths: string[] },
  ): string {
    // Polyfills.
    if (
      request.startsWith("node:") ||
      nativeModuleCanBeRequiredByUsers(request)
    ) {
      return request;
    }

    let paths: string[];

    if (typeof options === "object" && options !== null) {
      if (Array.isArray(options.paths)) {
        const isRelative = request.startsWith("./") ||
          request.startsWith("../") ||
          (isWindows && request.startsWith(".\\")) ||
          request.startsWith("..\\");

        if (isRelative) {
          paths = options.paths;
        } else {
          const fakeParent = new Module("", null);

          paths = [];

          for (let i = 0; i < options.paths.length; i++) {
            const path = options.paths[i];
            fakeParent.paths = Module._nodeModulePaths(path);
            const lookupPaths = Module._resolveLookupPaths(request, fakeParent);

            for (let j = 0; j < lookupPaths!.length; j++) {
              if (!paths.includes(lookupPaths![j])) {
                paths.push(lookupPaths![j]);
              }
            }
          }
        }
      } else if (options.paths === undefined) {
        paths = Module._resolveLookupPaths(request, parent)!;
      } else {
        throw new Error("options.paths is invalid");
      }
    } else {
      paths = Module._resolveLookupPaths(request, parent)!;
    }

    if (parent?.filename) {
      if (request[0] === "#") {
        const pkg = readPackageScope(parent.filename) ||
          { path: "", data: {} as PackageInfo };
        if (pkg.data?.imports != null) {
          try {
            return finalizeEsmResolution(
              packageImportsResolve(
                request,
                pathToFileURL(parent.filename).toString(),
                cjsConditions,
              ).toString(),
              parent.filename,
              pkg.path,
            );
          } catch (e) {
            if (e instanceof NodeError && e.code === "ERR_MODULE_NOT_FOUND") {
              throw createEsmNotFoundErr(request);
            }
            throw e;
          }
        }
      }
    }

    // Try module self resolution first
    const parentPath = trySelfParentPath(parent);
    const selfResolved = trySelf(parentPath, request);
    if (selfResolved) {
      const cacheKey = request + "\x00" +
        (paths.length === 1 ? paths[0] : paths.join("\x00"));
      Module._pathCache[cacheKey] = selfResolved;
      return selfResolved;
    }

    // Look up the filename first, since that's the cache key.
    const filename = Module._findPath(request, paths, isMain);
    if (!filename) {
      const requireStack = [];
      for (let cursor: Module | null = parent; cursor; cursor = cursor.parent) {
        requireStack.push(cursor.filename || cursor.id);
      }
      let message = `Cannot find module '${request}'`;
      if (requireStack.length > 0) {
        message = message + "\nRequire stack:\n- " + requireStack.join("\n- ");
      }
      const err = new Error(message) as Error & {
        code: string;
        requireStack: string[];
      };
      err.code = "MODULE_NOT_FOUND";
      err.requireStack = requireStack;
      throw err;
    }
    return filename as string;
  }

  static _findPath(
    request: string,
    paths: string[],
    isMain: boolean,
  ): string | boolean {
    const absoluteRequest = path.isAbsolute(request);
    if (absoluteRequest) {
      paths = [""];
    } else if (!paths || paths.length === 0) {
      return false;
    }

    const cacheKey = request + "\x00" +
      (paths.length === 1 ? paths[0] : paths.join("\x00"));
    const entry = Module._pathCache[cacheKey];
    if (entry) {
      return entry;
    }

    let exts;
    let trailingSlash = request.length > 0 &&
      request.charCodeAt(request.length - 1) === CHAR_FORWARD_SLASH;
    if (!trailingSlash) {
      trailingSlash = /(?:^|\/)\.?\.$/.test(request);
    }

    // For each path
    for (let i = 0; i < paths.length; i++) {
      // Don't search further if path doesn't exist
      const curPath = paths[i];

      if (curPath && stat(curPath) < 1) continue;
      const basePath = resolveExports(curPath, request, absoluteRequest);
      let filename;

      const rc = stat(basePath);
      if (!trailingSlash) {
        if (rc === 0) {
          // File.
          // preserveSymlinks removed
          filename = toRealPath(basePath);
        }

        if (!filename) {
          // Try it with each of the extensions
          if (exts === undefined) exts = Object.keys(Module._extensions);
          filename = tryExtensions(basePath, exts, isMain);
        }
      }

      if (!filename && rc === 1) {
        // Directory.
        // try it with each of the extensions at "index"
        if (exts === undefined) exts = Object.keys(Module._extensions);
        filename = tryPackage(basePath, exts, isMain, request);
      }

      if (filename) {
        Module._pathCache[cacheKey] = filename;
        return filename;
      }
    }
    // trySelf removed.

    return false;
  }

  // Check the cache for the requested file.
  // 1. If a module already exists in the cache: return its exports object.
  // 2. If the module is native: call
  //    `NativeModule.prototype.compileForPublicLoader()` and return the exports.
  // 3. Otherwise, create a new module for the file and save it to the cache.
  //    Then have it load  the file contents before returning its exports
  //    object.
  // deno-lint-ignore no-explicit-any
  static _load(request: string, parent: Module, isMain: boolean): any {
    let relResolveCacheIdentifier: string | undefined;
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

    // NOTE(@bartlomieju): this is a temporary solution. We provide some
    // npm modules with fixes in inconsistencies between Deno and Node.js.
    const upstreamMod = loadUpstreamModule(request, parent, request);
    if (upstreamMod) return upstreamMod.exports;

    const filename = Module._resolveFilename(request, parent, isMain);
    if (filename.startsWith("node:")) {
      // Slice 'node:' prefix
      const id = filename.slice(5);
      const module = loadNativeModule(id, id);
      // NOTE: Skip checking if can be required by user,
      // because we don't support internal modules anyway.
      return module?.exports;
    }

    const cachedModule = Module._cache[filename];
    if (cachedModule !== undefined) {
      updateChildren(parent, cachedModule, true);
      if (!cachedModule.loaded) {
        return getExportsForCircularRequire(cachedModule);
      }
      return cachedModule.exports;
    }

    // Native module polyfills
    const mod = loadNativeModule(filename, request);
    if (mod) return mod.exports;

    // Don't call updateChildren(), Module constructor already does.
    const module = new Module(filename, parent);

    if (isMain) {
      process.mainModule = module;
      module.id = ".";
    }

    Module._cache[filename] = module;
    if (parent !== undefined) {
      relativeResolveCache[relResolveCacheIdentifier!] = filename;
    }

    let threw = true;
    try {
      // Source map code removed
      module.load(filename);
      threw = false;
    } finally {
      if (threw) {
        delete Module._cache[filename];
        if (parent !== undefined) {
          delete relativeResolveCache[relResolveCacheIdentifier!];
        }
      } else if (
        module.exports &&
        Object.getPrototypeOf(module.exports) ===
          CircularRequirePrototypeWarningProxy
      ) {
        Object.setPrototypeOf(module.exports, PublicObjectPrototype);
      }
    }

    return module.exports;
  }

  static wrap(script: string): string {
    script = script.replace(/^#!.*?\n/, "");
    return `${Module.wrapper[0]}${script}${Module.wrapper[1]}`;
  }

  static _nodeModulePaths(from: string): string[] {
    if (isWindows) {
      // Guarantee that 'from' is absolute.
      from = path.resolve(from);

      // note: this approach *only* works when the path is guaranteed
      // to be absolute.  Doing a fully-edge-case-correct path.split
      // that works on both Windows and Posix is non-trivial.

      // return root node_modules when path is 'D:\\'.
      // path.resolve will make sure from.length >=3 in Windows.
      if (
        from.charCodeAt(from.length - 1) === CHAR_BACKWARD_SLASH &&
        from.charCodeAt(from.length - 2) === CHAR_COLON
      ) {
        return [from + "node_modules"];
      }

      const paths = [];
      for (let i = from.length - 1, p = 0, last = from.length; i >= 0; --i) {
        const code = from.charCodeAt(i);
        // The path segment separator check ('\' and '/') was used to get
        // node_modules path for every path segment.
        // Use colon as an extra condition since we can get node_modules
        // path for drive root like 'C:\node_modules' and don't need to
        // parse drive name.
        if (
          code === CHAR_BACKWARD_SLASH ||
          code === CHAR_FORWARD_SLASH ||
          code === CHAR_COLON
        ) {
          if (p !== nmLen) paths.push(from.slice(0, last) + "\\node_modules");
          last = i;
          p = 0;
        } else if (p !== -1) {
          if (nmChars[p] === code) {
            ++p;
          } else {
            p = -1;
          }
        }
      }

      return paths;
    } else {
      // posix
      // Guarantee that 'from' is absolute.
      from = path.resolve(from);
      // Return early not only to avoid unnecessary work, but to *avoid* returning
      // an array of two items for a root: [ '//node_modules', '/node_modules' ]
      if (from === "/") return ["/node_modules"];

      // note: this approach *only* works when the path is guaranteed
      // to be absolute.  Doing a fully-edge-case-correct path.split
      // that works on both Windows and Posix is non-trivial.
      const paths = [];
      for (let i = from.length - 1, p = 0, last = from.length; i >= 0; --i) {
        const code = from.charCodeAt(i);
        if (code === CHAR_FORWARD_SLASH) {
          if (p !== nmLen) paths.push(from.slice(0, last) + "/node_modules");
          last = i;
          p = 0;
        } else if (p !== -1) {
          if (nmChars[p] === code) {
            ++p;
          } else {
            p = -1;
          }
        }
      }

      // Append /node_modules to handle root paths.
      paths.push("/node_modules");

      return paths;
    }
  }

  /**
   * Create a `require` function that can be used to import CJS modules.
   * Follows CommonJS resolution similar to that of Node.js,
   * with `node_modules` lookup and `index.js` lookup support.
   * Also injects available Node.js builtin module polyfills.
   *
   * @param filename path or URL to current module
   * @return Require function to import CJS modules
   */
  static createRequire(filename: string | URL): RequireFunction {
    let filepath: string;
    if (
      filename instanceof URL ||
      (typeof filename === "string" && !path.isAbsolute(filename))
    ) {
      try {
        filepath = fileURLToPath(filename);
      } catch (err) {
        // deno-lint-ignore no-explicit-any
        if ((err as any).code === "ERR_INVALID_URL_SCHEME") {
          // Provide a descriptive error when url scheme is invalid.
          throw new Error(
            `${createRequire.name} only supports 'file://' URLs for the 'filename' parameter. Received '${filename}'`,
          );
        } else {
          throw err;
        }
      }
    } else if (typeof filename !== "string") {
      throw new Error("filename should be a string");
    } else {
      filepath = filename;
    }
    return createRequireFromPath(filepath);
  }

  static _initPaths() {
    const homeDir = Deno.env.get("HOME");
    const nodePath = Deno.env.get("NODE_PATH");

    // Removed $PREFIX/bin/node case

    let paths = [];

    if (homeDir) {
      paths.unshift(path.resolve(homeDir, ".node_libraries"));
      paths.unshift(path.resolve(homeDir, ".node_modules"));
    }

    if (nodePath) {
      paths = nodePath
        .split(path.delimiter)
        .filter(function pathsFilterCB(path) {
          return !!path;
        })
        .concat(paths);
    }

    modulePaths = paths;

    // Clone as a shallow copy, for introspection.
    Module.globalPaths = modulePaths.slice(0);
  }

  static _preloadModules(requests: string[]) {
    if (!Array.isArray(requests)) {
      return;
    }

    // Preloaded modules have a dummy parent module which is deemed to exist
    // in the current working directory. This seeds the search path for
    // preloaded modules.
    const parent = new Module("internal/preload", null);
    try {
      parent.paths = Module._nodeModulePaths(Deno.cwd());
    } catch (e) {
      if (
        !(e instanceof Error) ||
        (e as Error & { code?: string }).code !== "ENOENT"
      ) {
        throw e;
      }
    }
    for (let n = 0; n < requests.length; n++) {
      parent.require(requests[n]);
    }
  }
}

// Polyfills.
const nativeModulePolyfill = new Map<string, Module>();
// deno-lint-ignore no-explicit-any
function createNativeModule(id: string, exports: any): Module {
  const mod = new Module(id);
  mod.exports = exports;
  mod.loaded = true;
  return mod;
}

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

Object.setPrototypeOf(m, Module);
nodeMods.module = m;

function loadNativeModule(
  _filename: string,
  request: string,
): Module | undefined {
  if (nativeModulePolyfill.has(request)) {
    return nativeModulePolyfill.get(request);
  }
  const mod = nodeMods[request];
  if (mod) {
    const nodeMod = createNativeModule(request, mod);
    nativeModulePolyfill.set(request, nodeMod);
    return nodeMod;
  }
  return undefined;
}
function nativeModuleCanBeRequiredByUsers(request: string): boolean {
  return hasOwn(nodeMods, request);
}
// Populate with polyfill names
Module.builtinModules.push(...Object.keys(nodeMods));

// NOTE(@bartlomieju): temporary solution, to smooth out inconsistencies between
// Deno and Node.js.
const upstreamModules = new Map<string, Module>();

function loadUpstreamModule(
  filename: string,
  parent: Module | null,
  request: string,
): Module | undefined {
  if (typeof upstreamMods[request] !== "undefined") {
    if (!upstreamModules.has(filename)) {
      upstreamModules.set(
        filename,
        createUpstreamModule(filename, parent, upstreamMods[request]),
      );
    }
    return upstreamModules.get(filename);
  }
}
function createUpstreamModule(
  filename: string,
  parent: Module | null,
  content: string,
): Module {
  const mod = new Module(filename, parent);
  mod.filename = filename;
  mod.paths = Module._nodeModulePaths(path.dirname(filename));
  mod._compile(content, filename);
  mod.loaded = true;
  return mod;
}

let modulePaths: string[] = [];

// Given a module name, and a list of paths to test, returns the first
// matching file in the following precedence.
//
// require("a.<ext>")
//   -> a.<ext>
//
// require("a")
//   -> a
//   -> a.<ext>
//   -> a/index.<ext>

const packageJsonCache = new Map<string, PackageInfo | null>();

interface PackageInfo {
  name?: string;
  main?: string;
  // deno-lint-ignore no-explicit-any
  exports?: any;
  // deno-lint-ignore no-explicit-any
  imports?: any;
  // deno-lint-ignore no-explicit-any
  type?: any;
}

function readPackage(requestPath: string): PackageInfo | null {
  const jsonPath = path.resolve(requestPath, "package.json");

  const existing = packageJsonCache.get(jsonPath);
  if (existing !== undefined) {
    return existing;
  }

  let json: string | undefined;
  try {
    json = new TextDecoder().decode(
      Deno.readFileSync(path.toNamespacedPath(jsonPath)),
    );
  } catch {
    // pass
  }

  if (json === undefined) {
    packageJsonCache.set(jsonPath, null);
    return null;
  }

  try {
    const parsed = JSON.parse(json);
    const filtered = {
      name: parsed.name,
      main: parsed.main,
      path: jsonPath,
      exports: parsed.exports,
      imports: parsed.imports,
      type: parsed.type,
    };
    packageJsonCache.set(jsonPath, filtered);
    return filtered;
  } catch (e) {
    const err = (e instanceof Error ? e : new Error("[non-error thrown]")) as
      & Error
      & { path?: string };
    err.path = jsonPath;
    err.message = "Error parsing " + jsonPath + ": " + err.message;
    throw e;
  }
}

function readPackageScope(
  checkPath: string,
): { path: string; data: PackageInfo } | false {
  const rootSeparatorIndex = checkPath.indexOf(path.sep);
  let separatorIndex;
  while (
    (separatorIndex = checkPath.lastIndexOf(path.sep)) > rootSeparatorIndex
  ) {
    checkPath = checkPath.slice(0, separatorIndex);
    if (checkPath.endsWith(path.sep + "node_modules")) return false;
    const pjson = readPackage(checkPath);
    if (pjson) {
      return {
        path: checkPath,
        data: pjson,
      };
    }
  }
  return false;
}

function readPackageMain(requestPath: string): string | undefined {
  const pkg = readPackage(requestPath);
  return pkg ? pkg.main : undefined;
}

// deno-lint-ignore no-explicit-any
function readPackageExports(requestPath: string): any | undefined {
  const pkg = readPackage(requestPath);
  return pkg ? pkg.exports : undefined;
}

function tryPackage(
  requestPath: string,
  exts: string[],
  isMain: boolean,
  _originalPath: string,
): string | false {
  const pkg = readPackageMain(requestPath);

  if (!pkg) {
    return tryExtensions(path.resolve(requestPath, "index"), exts, isMain);
  }

  const filename = path.resolve(requestPath, pkg);
  let actual = tryFile(filename, isMain) ||
    tryExtensions(filename, exts, isMain) ||
    tryExtensions(path.resolve(filename, "index"), exts, isMain);
  if (actual === false) {
    actual = tryExtensions(path.resolve(requestPath, "index"), exts, isMain);
    if (!actual) {
      const err = new Error(
        `Cannot find module '${filename}'. ` +
          'Please verify that the package.json has a valid "main" entry',
      ) as Error & { code: string };
      err.code = "MODULE_NOT_FOUND";
      throw err;
    }
  }
  return actual;
}

// Check if the file exists and is not a directory
// if using --preserve-symlinks and isMain is false,
// keep symlinks intact, otherwise resolve to the
// absolute realpath.
function tryFile(requestPath: string, _isMain: boolean): string | false {
  const rc = stat(requestPath);
  return rc === 0 && toRealPath(requestPath);
}

function toRealPath(requestPath: string): string {
  return Deno.realPathSync(requestPath);
}

// Given a path, check if the file exists with any of the set extensions
function tryExtensions(
  p: string,
  exts: string[],
  isMain: boolean,
): string | false {
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
function findLongestRegisteredExtension(filename: string): string {
  const name = path.basename(filename);
  let currentExtension;
  let index;
  let startIndex = 0;
  while ((index = name.indexOf(".", startIndex)) !== -1) {
    startIndex = index + 1;
    if (index === 0) continue; // Skip dotfiles like .gitignore
    currentExtension = name.slice(index);
    if (Module._extensions[currentExtension]) return currentExtension;
  }
  return ".js";
}

// deno-lint-ignore no-explicit-any
function isConditionalDotExportSugar(exports: any, _basePath: string): boolean {
  if (typeof exports === "string") return true;
  if (Array.isArray(exports)) return true;
  if (typeof exports !== "object") return false;
  let isConditional = false;
  let firstCheck = true;
  for (const key of Object.keys(exports)) {
    const curIsConditional = key[0] !== ".";
    if (firstCheck) {
      firstCheck = false;
      isConditional = curIsConditional;
    } else if (isConditional !== curIsConditional) {
      throw new Error(
        '"exports" cannot ' +
          "contain some keys starting with '.' and some not. The exports " +
          "object must either be an object of package subpath keys or an " +
          "object of main entry condition name keys only.",
      );
    }
  }
  return isConditional;
}

function applyExports(basePath: string, expansion: string): string {
  const mappingKey = `.${expansion}`;

  let pkgExports = readPackageExports(basePath);
  if (pkgExports === undefined || pkgExports === null) {
    return path.resolve(basePath, mappingKey);
  }

  if (isConditionalDotExportSugar(pkgExports, basePath)) {
    pkgExports = { ".": pkgExports };
  }

  if (typeof pkgExports === "object") {
    if (hasOwn(pkgExports, mappingKey)) {
      const mapping = pkgExports[mappingKey];
      return resolveExportsTarget(
        pathToFileURL(basePath + "/"),
        mapping,
        "",
        basePath,
        mappingKey,
      );
    }

    // Fallback to CJS main lookup when no main export is defined
    if (mappingKey === ".") return basePath;

    let dirMatch = "";
    for (const candidateKey of Object.keys(pkgExports)) {
      if (candidateKey[candidateKey.length - 1] !== "/") continue;
      if (
        candidateKey.length > dirMatch.length &&
        mappingKey.startsWith(candidateKey)
      ) {
        dirMatch = candidateKey;
      }
    }

    if (dirMatch !== "") {
      const mapping = pkgExports[dirMatch];
      const subpath = mappingKey.slice(dirMatch.length);
      return resolveExportsTarget(
        pathToFileURL(basePath + "/"),
        mapping,
        subpath,
        basePath,
        mappingKey,
      );
    }
  }
  // Fallback to CJS main lookup when no main export is defined
  if (mappingKey === ".") return basePath;

  const e = new Error(
    `Package exports for '${basePath}' do not define ` +
      `a '${mappingKey}' subpath`,
  ) as Error & { code?: string };
  e.code = "MODULE_NOT_FOUND";
  throw e;
}

// This only applies to requests of a specific form:
// 1. name/.*
// 2. @scope/name/.*
const EXPORTS_PATTERN = /^((?:@[^/\\%]+\/)?[^./\\%][^/\\%]*)(\/.*)?$/;
function resolveExports(
  nmPath: string,
  request: string,
  absoluteRequest: boolean,
): string {
  // The implementation's behavior is meant to mirror resolution in ESM.
  if (!absoluteRequest) {
    const [, name, expansion = ""] = request.match(EXPORTS_PATTERN) || [];
    if (!name) {
      return path.resolve(nmPath, request);
    }

    const basePath = path.resolve(nmPath, name);
    return applyExports(basePath, expansion);
  }

  return path.resolve(nmPath, request);
}

// Node.js uses these keys for resolving conditional exports.
// ref: https://nodejs.org/api/packages.html#packages_conditional_exports
// ref: https://github.com/nodejs/node/blob/2c77fe1/lib/internal/modules/cjs/helpers.js#L33
const cjsConditions = new Set(["deno", "require", "node"]);

function resolveExportsTarget(
  pkgPath: URL,
  // deno-lint-ignore no-explicit-any
  target: any,
  subpath: string,
  basePath: string,
  mappingKey: string,
): string {
  if (typeof target === "string") {
    if (
      target.startsWith("./") &&
      (subpath.length === 0 || target.endsWith("/"))
    ) {
      const resolvedTarget = new URL(target, pkgPath);
      const pkgPathPath = pkgPath.pathname;
      const resolvedTargetPath = resolvedTarget.pathname;
      if (
        resolvedTargetPath.startsWith(pkgPathPath) &&
        resolvedTargetPath.indexOf("/node_modules/", pkgPathPath.length - 1) ===
          -1
      ) {
        const resolved = new URL(subpath, resolvedTarget);
        const resolvedPath = resolved.pathname;
        if (
          resolvedPath.startsWith(resolvedTargetPath) &&
          resolvedPath.indexOf("/node_modules/", pkgPathPath.length - 1) === -1
        ) {
          return fileURLToPath(resolved);
        }
      }
    }
  } else if (Array.isArray(target)) {
    for (const targetValue of target) {
      if (Array.isArray(targetValue)) continue;
      try {
        return resolveExportsTarget(
          pkgPath,
          targetValue,
          subpath,
          basePath,
          mappingKey,
        );
      } catch (e) {
        if (
          !(e instanceof Error) ||
          (e as Error & { code?: string }).code !== "MODULE_NOT_FOUND"
        ) {
          throw e;
        }
      }
    }
  } else if (typeof target === "object" && target !== null) {
    for (const key of Object.keys(target)) {
      if (key !== "default" && !cjsConditions.has(key)) {
        continue;
      }
      if (hasOwn(target, key)) {
        try {
          return resolveExportsTarget(
            pkgPath,
            target[key],
            subpath,
            basePath,
            mappingKey,
          );
        } catch (e) {
          if (
            !(e instanceof Error) ||
            (e as Error & { code?: string }).code !== "MODULE_NOT_FOUND"
          ) {
            throw e;
          }
        }
      }
    }
  }
  let e: Error & { code?: string };
  if (mappingKey !== ".") {
    e = new Error(
      `Package exports for '${basePath}' do not define a ` +
        `valid '${mappingKey}' target${subpath ? " for " + subpath : ""}`,
    );
  } else {
    e = new Error(`No valid exports main found for '${basePath}'`);
  }
  e.code = "MODULE_NOT_FOUND";
  throw e;
}

// 'node_modules' character codes reversed
const nmChars = [115, 101, 108, 117, 100, 111, 109, 95, 101, 100, 111, 110];
const nmLen = nmChars.length;

// deno-lint-ignore no-explicit-any
function emitCircularRequireWarning(prop: any) {
  console.error(
    `Accessing non-existent property '${
      String(prop)
    }' of module exports inside circular dependency`,
  );
}

// A Proxy that can be used as the prototype of a module.exports object and
// warns when non-existent properties are accessed.
const CircularRequirePrototypeWarningProxy = new Proxy(
  {},
  {
    // deno-lint-ignore no-explicit-any
    get(target: Record<string, any>, prop: string): any {
      if (prop in target) return target[prop];
      emitCircularRequireWarning(prop);
      return undefined;
    },

    getOwnPropertyDescriptor(target, prop): PropertyDescriptor | undefined {
      if (hasOwn(target, prop)) {
        return Object.getOwnPropertyDescriptor(target, prop);
      }
      emitCircularRequireWarning(prop);
      return undefined;
    },
  },
);

// Object.prototype and ObjectPrototype refer to our 'primordials' versions
// and are not identical to the versions on the global object.
const PublicObjectPrototype = globalThis.Object.prototype;

// deno-lint-ignore no-explicit-any
function getExportsForCircularRequire(module: Module): any {
  if (
    module.exports &&
    Object.getPrototypeOf(module.exports) === PublicObjectPrototype &&
    // Exclude transpiled ES6 modules / TypeScript code because those may
    // employ unusual patterns for accessing 'module.exports'. That should be
    // okay because ES6 modules have a different approach to circular
    // dependencies anyway.
    !module.exports.__esModule
  ) {
    // This is later unset once the module is done loading.
    Object.setPrototypeOf(module.exports, CircularRequirePrototypeWarningProxy);
  }

  return module.exports;
}

type RequireWrapper = (
  // deno-lint-ignore no-explicit-any
  exports: any,
  // deno-lint-ignore no-explicit-any
  require: any,
  module: Module,
  __filename: string,
  __dirname: string,
  setTimeout_: typeof setTimeout,
  clearTimeout_: typeof clearTimeout,
  setInterval_: typeof setInterval,
  clearInterval_: typeof clearInterval,
) => void;

function enrichCJSError(error: Error) {
  if (error instanceof SyntaxError) {
    if (
      error.message.includes("Cannot use import statement outside a module") ||
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
  filename: string,
  content: string,
  cjsModuleInstance: Module,
): RequireWrapper {
  // TODO(bartlomieju): fix this
  const wrapper = Module.wrap(content);
  const [f, err] = core.evalContext(wrapper, filename);
  if (err) {
    if (process.mainModule === cjsModuleInstance) {
      enrichCJSError(err.thrown);
    }
    throw err.thrown;
  }
  return f;
}

// Native extension for .js
Module._extensions[".js"] = (module: Module, filename: string) => {
  if (filename.endsWith(".js")) {
    const pkg = readPackageScope(filename);
    if (pkg !== false && pkg.data && pkg.data.type === "module") {
      throw new Error(`Importing ESM module: ${filename}.`);
    }
  }
  const content = new TextDecoder().decode(Deno.readFileSync(filename));
  module._compile(content, filename);
};

// Native extension for .mjs
Module._extensions[".mjs"] = (_module: Module, filename: string) => {
  throw new Error(`Importing ESM module: ${filename}.`);
};

// Native extension for .json
Module._extensions[".json"] = (module: Module, filename: string) => {
  const content = new TextDecoder().decode(Deno.readFileSync(filename));
  // manifest code removed
  try {
    module.exports = JSON.parse(stripBOM(content));
  } catch (err) {
    const e = err instanceof Error ? err : new Error("[non-error thrown]");
    e.message = `${filename}: ${e.message}`;
    throw e;
  }
};

Module._extensions[".node"] = (module: Module, filename: string) => {
  module.exports = core.ops.op_napi_open(filename);
};

function createRequireFromPath(filename: string): RequireFunction {
  // Allow a directory to be passed as the filename
  const trailingSlash = filename.endsWith("/") ||
    (isWindows && filename.endsWith("\\"));

  const proxyPath = trailingSlash ? path.join(filename, "noop.js") : filename;

  const m = new Module(proxyPath);
  m.filename = proxyPath;

  m.paths = Module._nodeModulePaths(m.path);
  return makeRequireFunction(m);
}

// deno-lint-ignore no-explicit-any
type Require = (id: string) => any;
// deno-lint-ignore no-explicit-any
type RequireResolve = (request: string, options: any) => string;
interface RequireResolveFunction extends RequireResolve {
  paths: (request: string) => string[] | null;
}

interface RequireFunction extends Require {
  resolve: RequireResolveFunction;
  // deno-lint-ignore no-explicit-any
  extensions: { [key: string]: (module: Module, filename: string) => any };
  cache: { [key: string]: Module };
}

function makeRequireFunction(mod: Module): RequireFunction {
  // deno-lint-ignore no-explicit-any
  const require = function require(path: string): any {
    return mod.require(path);
  };

  function resolve(request: string, options?: { paths: string[] }): string {
    return Module._resolveFilename(request, mod, false, options);
  }

  require.resolve = resolve;

  function paths(request: string): string[] | null {
    return Module._resolveLookupPaths(request, mod);
  }

  resolve.paths = paths;

  require.main = process.mainModule;

  // Enable support to add extra extension types.
  require.extensions = Module._extensions;

  require.cache = Module._cache;

  return require;
}

/**
 * Remove byte order marker. This catches EF BB BF (the UTF-8 BOM)
 * because the buffer-to-string conversion in `fs.readFileSync()`
 * translates it to FEFF, the UTF-16 BOM.
 */
function stripBOM(content: string): string {
  if (content.charCodeAt(0) === 0xfeff) {
    content = content.slice(1);
  }
  return content;
}

// These two functions are not exported in Node, but it's very
// helpful to have them available for compat mode in CLI
export function resolveMainPath(main: string): undefined | string {
  // Note extension resolution for the main entry point can be deprecated in a
  // future major.
  // Module._findPath is monkey-patchable here.
  const mainPath = Module._findPath(path.resolve(main), [], true);
  if (!mainPath) {
    return;
  }

  // NOTE(bartlomieju): checking for `--preserve-symlinks-main` flag
  // is skipped as this flag is not supported by Deno CLI.
  // const preserveSymlinksMain = getOptionValue('--preserve-symlinks-main');
  // if (!preserveSymlinksMain)
  //   mainPath = toRealPath(mainPath);

  return mainPath as string;
}

export function shouldUseESMLoader(mainPath: string): boolean {
  // NOTE(bartlomieju): these two are skipped, because Deno CLI
  // doesn't suport these flags
  // const userLoader = getOptionValue('--experimental-loader');
  // if (userLoader)
  //   return true;
  // const esModuleSpecifierResolution =
  //   getOptionValue('--experimental-specifier-resolution');
  // if (esModuleSpecifierResolution === 'node')
  //   return true;

  // Determine the module format of the main
  if (mainPath && mainPath.endsWith(".mjs")) {
    return true;
  }
  if (!mainPath || mainPath.endsWith(".cjs")) {
    return false;
  }
  const pkg = readPackageScope(mainPath);
  return pkg && pkg.data.type === "module";
}

export const builtinModules = Module.builtinModules;
export const createRequire = Module.createRequire;
export default Module;
