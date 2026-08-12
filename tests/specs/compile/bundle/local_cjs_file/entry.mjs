// Regression test: a local CJS file imported from ESM. The CJS-from-ESM
// wrapper turns the import into a runtime require() of the file's path
// (externalized rather than inlined), so the file itself must be embedded
// in the VFS or the compiled binary fails at runtime with
// "Cannot find module .../local.js".
import value from "./local.js";

console.log(value());
