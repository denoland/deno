// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/* eslint-disable */

// A script preamble that provides the ability to load a single outfile
// TypeScript "bundle" where a main module is loaded which recursively
// instantiates all the other modules in the bundle.  This code is used to load
// bundles when creating snapshots, but is also used when emitting bundles from
// Deno cli.

// @ts-nocheck

/**
 * @type {(name: string, deps: ReadonlyArray<string>, factory: (...deps: any[]) => void) => void=}
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
let define;

/**
 * @type {(mod: string) => any=}
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
let instantiate;

/**
 * @callback Factory
 * @argument {...any[]} args
 * @returns {object | void}
 */

/**
 * @typedef ModuleMetaData
 * @property {ReadonlyArray<string>} dependencies
 * @property {(Factory | object)=} factory
 * @property {object} exports
 */

(function() {
  /**
   * @type {Map<string, ModuleMetaData>}
   */
  const modules = new Map();

  /**
   * Bundles in theory can support "dynamic" imports, but for internal bundles
   * we can't go outside to fetch any modules that haven't been statically
   * defined.
   * @param {string[]} deps
   * @param {(...deps: any[]) => void} resolve
   * @param {(err: any) => void} reject
   */
  const require = (deps, resolve, reject) => {
    try {
      if (deps.length !== 1) {
        throw new TypeError("Expected only a single module specifier.");
      }
      if (!modules.has(deps[0])) {
        throw new RangeError(`Module "${deps[0]}" not defined.`);
      }
      resolve(getExports(deps[0]));
    } catch (e) {
      if (reject) {
        reject(e);
      } else {
        throw e;
      }
    }
  };

  define = (id, dependencies, factory) => {
    if (modules.has(id)) {
      throw new RangeError(`Module "${id}" has already been defined.`);
    }
    modules.set(id, {
      dependencies,
      factory,
      exports: {}
    });
  };

  /**
   * @param {string} id
   * @returns {any}
   */
  function getExports(id) {
    const module = modules.get(id);
    if (!module) {
      // because `$deno$/ts_global.d.ts` looks like a real script, it doesn't
      // get erased from output as an import, but it doesn't get defined, so
      // we don't have a cache for it, so because this is an internal bundle
      // we can just safely return an empty object literal.
      return {};
    }
    if (!module.factory) {
      return module.exports;
    } else if (module.factory) {
      const { factory, exports } = module;
      delete module.factory;
      if (typeof factory === "function") {
        const dependencies = module.dependencies.map(id => {
          if (id === "require") {
            return require;
          } else if (id === "exports") {
            return exports;
          }
          return getExports(id);
        });
        factory(...dependencies);
      } else {
        Object.assign(exports, factory);
      }
      return exports;
    }
  }

  instantiate = dep => {
    define = undefined;
    const result = getExports(dep);
    // clean up, or otherwise these end up in the runtime environment
    instantiate = undefined;
    return result;
  };
})();

define("print_hello", ["require", "exports"], function(require, exports) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function printHello() {
    console.log("Hello");
  }
  exports.printHello = printHello;
});
define("subdir2/mod2", ["require", "exports", "print_hello"], function(
  require,
  exports,
  print_hello_ts_1
) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function returnsFoo() {
    return "Foo";
  }
  exports.returnsFoo = returnsFoo;
  function printHello2() {
    print_hello_ts_1.printHello();
  }
  exports.printHello2 = printHello2;
});
define("mod1", ["require", "exports", "subdir2/mod2"], function(
  require,
  exports,
  mod2_ts_1
) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function returnsHi() {
    return "Hi";
  }
  exports.returnsHi = returnsHi;
  function returnsFoo2() {
    return mod2_ts_1.returnsFoo();
  }
  exports.returnsFoo2 = returnsFoo2;
  function printHello3() {
    mod2_ts_1.printHello2();
  }
  exports.printHello3 = printHello3;
  function throwsError() {
    throw Error("exception from mod1");
  }
  exports.throwsError = throwsError;
});

const __rootExports = instantiate("mod1");
export const returnsHi = __rootExports["returnsHi"];
export const returnsFoo2 = __rootExports["returnsFoo2"];
export const printHello3 = __rootExports["printHello3"];
export const throwsError = __rootExports["throwsError"];
