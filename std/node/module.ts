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

import * as nodeBuffer from "./buffer.ts";
import * as nodeEvents from "./events.ts";
import * as nodeFS from "./fs.ts";
import * as nodeOs from "./os.ts";
import * as nodePath from "./path.ts";
import * as nodeTimers from "./timers.ts";
import * as nodeQueryString from "./querystring.ts";
import * as nodeStringDecoder from "./string_decoder.ts";
import * as nodeUtil from "./util.ts";

import * as path from "../path/mod.ts";
import { assert } from "../_util/assert.ts";
import { pathToFileURL, fileURLToPath } from "./url.ts";

const CHAR_FORWARD_SLASH = "/".charCodeAt(0);
const CHAR_BACKWARD_SLASH = "\\".charCodeAt(0);
const CHAR_COLON = ":".charCodeAt(0);

const isWindows = Deno.build.os == "windows";

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
): void {
  const children = parent && parent.children;
  if (children && !(scan && children.includes(child))) {
    children.push(child);
  }
}

class Module {
  id: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    [key: string]: (module: Module, filename: string) => any;
  } = Object.create(null);
  static _cache: { [key: string]: Module } = Object.create(null);
  static _pathCache = Object.create(null);
  static globalPaths: string[] = [];
  // Proxy related code removed.
  static wrapper = [
    "(function (exports, require, module, __filename, __dirname) { ",
    "\n});",
  ];

  // Loads a module at the given file path. Returns that module's
  // `exports` property.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  load(filename: string): void {
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  _compile(content: string, filename: string): any {
    // manifest code removed
    const compiledWrapper = wrapSafe(filename, content);
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
    );
    if (requireDepth === 0) {
      statCache = null;
    }
    return result;
  }

  /*
   * Check for node modules paths.
   * */
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
    if (nativeModuleCanBeRequiredByUsers(request)) {
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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

    const filename = Module._resolveFilename(request, parent, isMain);

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
      // TODO: set process info
      // process.mainModule = module;
      module.id = ".";
    }

    Module._cache[filename] = module;
    if (parent !== undefined) {
      assert(relResolveCacheIdentifier);
      relativeResolveCache[relResolveCacheIdentifier] = filename;
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
          assert(relResolveCacheIdentifier);
          delete relativeResolveCache[relResolveCacheIdentifier];
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
   *     const require = createRequire(import.meta.url);
   *     const fs = require("fs");
   *     const leftPad = require("left-pad");
   *     const cjsModule = require("./cjs_mod");
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
      filepath = fileURLToPath(filename);
    } else if (typeof filename !== "string") {
      throw new Error("filename should be a string");
    } else {
      filepath = filename;
    }
    return createRequireFromPath(filepath);
  }

  static _initPaths(): void {
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

  static _preloadModules(requests: string[]): void {
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
      if (e.code !== "ENOENT") {
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
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createNativeModule(id: string, exports: any): Module {
  const mod = new Module(id);
  mod.exports = exports;
  mod.loaded = true;
  return mod;
}

nativeModulePolyfill.set("buffer", createNativeModule("buffer", nodeBuffer));
nativeModulePolyfill.set("events", createNativeModule("events", nodeEvents));
nativeModulePolyfill.set("fs", createNativeModule("fs", nodeFS));
nativeModulePolyfill.set("os", createNativeModule("os", nodeOs));
nativeModulePolyfill.set("path", createNativeModule("path", nodePath));
nativeModulePolyfill.set(
  "querystring",
  createNativeModule("querystring", nodeQueryString),
);
nativeModulePolyfill.set(
  "string_decoder",
  createNativeModule("string_decoder", nodeStringDecoder),
);
nativeModulePolyfill.set("timers", createNativeModule("timers", nodeTimers));
nativeModulePolyfill.set("util", createNativeModule("util", nodeUtil));

function loadNativeModule(
  _filename: string,
  request: string,
): Module | undefined {
  return nativeModulePolyfill.get(request);
}
function nativeModuleCanBeRequiredByUsers(request: string): boolean {
  return nativeModulePolyfill.has(request);
}
// Populate with polyfill names
for (const id of nativeModulePolyfill.keys()) {
  Module.builtinModules.push(id);
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  exports?: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
      exports: parsed.exports,
      type: parsed.type,
    };
    packageJsonCache.set(jsonPath, filtered);
    return filtered;
  } catch (e) {
    e.path = jsonPath;
    e.message = "Error parsing " + jsonPath + ": " + e.message;
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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  // Deno does not have realpath implemented yet.
  let fullPath = requestPath;
  while (true) {
    try {
      fullPath = Deno.readLinkSync(fullPath);
    } catch {
      break;
    }
  }
  return path.resolve(requestPath);
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

// --experimental-resolve-self trySelf() support removed.

// eslint-disable-next-line @typescript-eslint/no-explicit-any
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
    if (Object.prototype.hasOwnProperty.call(pkgExports, mappingKey)) {
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

function resolveExportsTarget(
  pkgPath: URL,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
        if (e.code !== "MODULE_NOT_FOUND") throw e;
      }
    }
  } else if (typeof target === "object" && target !== null) {
    // removed experimentalConditionalExports
    if (Object.prototype.hasOwnProperty.call(target, "default")) {
      try {
        return resolveExportsTarget(
          pkgPath,
          target.default,
          subpath,
          basePath,
          mappingKey,
        );
      } catch (e) {
        if (e.code !== "MODULE_NOT_FOUND") throw e;
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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function emitCircularRequireWarning(prop: any): void {
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
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    get(target: Record<string, any>, prop: string): any {
      if (prop in target) return target[prop];
      emitCircularRequireWarning(prop);
      return undefined;
    },

    getOwnPropertyDescriptor(target, prop): PropertyDescriptor | undefined {
      if (Object.prototype.hasOwnProperty.call(target, prop)) {
        return Object.getOwnPropertyDescriptor(target, prop);
      }
      emitCircularRequireWarning(prop);
      return undefined;
    },
  },
);

// Object.prototype and ObjectProtoype refer to our 'primordials' versions
// and are not identical to the versions on the global object.
const PublicObjectPrototype = window.Object.prototype;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  exports: any,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  require: any,
  module: Module,
  __filename: string,
  __dirname: string,
) => void;

function wrapSafe(filename: string, content: string): RequireWrapper {
  // TODO: fix this
  const wrapper = Module.wrap(content);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [f, err] = (Deno as any).core.evalContext(wrapper, filename);
  if (err) {
    throw err;
  }
  return f;
  // ESM code removed.
}

// Native extension for .js
Module._extensions[".js"] = (module: Module, filename: string): void => {
  if (filename.endsWith(".js")) {
    const pkg = readPackageScope(filename);
    if (pkg !== false && pkg.data && pkg.data.type === "module") {
      throw new Error("Importing ESM module");
    }
  }
  const content = new TextDecoder().decode(Deno.readFileSync(filename));
  module._compile(content, filename);
};

// Native extension for .json
Module._extensions[".json"] = (module: Module, filename: string): void => {
  const content = new TextDecoder().decode(Deno.readFileSync(filename));
  // manifest code removed
  try {
    module.exports = JSON.parse(stripBOM(content));
  } catch (err) {
    err.message = filename + ": " + err.message;
    throw err;
  }
};

// .node extension is not supported

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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Require = (id: string) => any;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type RequireResolve = (request: string, options: any) => string;
interface RequireResolveFunction extends RequireResolve {
  paths: (request: string) => string[] | null;
}

interface RequireFunction extends Require {
  resolve: RequireResolveFunction;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  extensions: { [key: string]: (module: Module, filename: string) => any };
  cache: { [key: string]: Module };
}

function makeRequireFunction(mod: Module): RequireFunction {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  // TODO: set main
  // require.main = process.mainModule;

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

export const builtinModules = Module.builtinModules;
export const createRequire = Module.createRequire;
export default Module;
