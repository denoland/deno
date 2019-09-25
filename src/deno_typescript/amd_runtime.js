// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// A very very basic AMD preamble to support the output of TypeScript outFile
// bundles.

/**
 * @type {(name: string) => any}
 */
let require;

/**
 * @type {(name: string, deps: ReadonlyArray<string>, factory: (...deps: any[]) => void) => void}
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
let define;

(function() {
  /**
   * @type {Map<string, { name: string, exports: any }>}
   */
  const modules = new Map();

  /**
   * @param {string} name
   */
  function createOrLoadModule(name) {
    let m = modules.get(name);
    if (!m) {
      m = { name, exports: {} };
      modules.set(name, m);
    }
    return m;
  }

  require = name => {
    return createOrLoadModule(name).exports;
  };

  define = (name, deps, factory) => {
    const currentModule = createOrLoadModule(name);
    const localExports = currentModule.exports;
    const args = deps.map(dep => {
      if (dep === "require") {
        return require;
      } else if (dep === "exports") {
        return localExports;
      } else {
        const depModule = createOrLoadModule(dep);
        return depModule.exports;
      }
    });
    factory(...args);
  };
})();
