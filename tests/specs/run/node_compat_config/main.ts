// Tests that `"unstable": ["node-compat"]` in deno.json behaves the same as
// `DENO_COMPAT=1`: enables bare-node-builtins, sloppy-imports, detect-cjs,
// and node-globals.
import fs from "fs"; // bare-node-builtins
import * as a from "./a.js"; // sloppy-imports (resolves to ./a.ts)

console.log(typeof fs.writeFile);
console.log(a.A);

// node-globals: setTimeout should be the Node.js version that returns a
// Timeout object (not a number, which is the Deno default).
const t = setTimeout(() => {}, 1_000);
console.log(typeof t);
clearTimeout(t);
