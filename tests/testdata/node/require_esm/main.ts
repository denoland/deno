import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

console.log(require("./esm.js"));
