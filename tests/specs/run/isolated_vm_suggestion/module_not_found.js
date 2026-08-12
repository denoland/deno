// Reproduces the error reported when importing `npm:isolated-vm` without the
// native addon built (the most commonly reported case): the package's entry
// point does `require('./out/isolated_vm')`, which fails to resolve. The addon
// is built directly on V8's C++ internals and cannot run in Deno regardless.
// See denoland/deno#25130.
const err = new Error("Cannot find module './out/isolated_vm'");
err.stack = "Error: Cannot find module './out/isolated_vm'\n" +
  "Require stack:\n" +
  "- /home/me/.cache/deno/npm/registry.npmjs.org/isolated-vm/4.7.2/isolated-vm.js";
throw err;
