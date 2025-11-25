import { createRequire as __deno_internal_createRequire } from "node:module";
var __require = __deno_internal_createRequire(import.meta.url);

// main.cjs
var { createRequire } = __require("node:module");
var os = __require("node:os");
console.log(os.freemem.name);
var require2 = createRequire(import.meta.url);
console.log(require2.name);
