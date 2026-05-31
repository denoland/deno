import { createRequire } from "node:module";
import value from "module-sync-pkg";

const require = createRequire(import.meta.url);
const required = require("module-sync-pkg");

console.log(value.source);
console.log(required.default.source);
