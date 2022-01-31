import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const mod = require("./imported.js");

export default mod;
export const foo = mod.foo;
export const bar = mod.bar;