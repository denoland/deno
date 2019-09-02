// A very very basic AMD preamble to support the output of TypeScript outFile
// bundles.
let require, define;

(function() {
  const modules = new Map();

  function println(first, ...s) {
    Deno.core.print(first + " " + s.map(JSON.stringify).join(" ") + "\n");
  }

  function createOrLoadModule(name) {
    if (!modules.has(name)) {
      const m = { name, exports: {} };
      modules.set(name, m);
    }
    return modules.get(name);
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
