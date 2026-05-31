import { createRequire } from "node:module";
import value from "module-sync-pkg";
import nested from "module-sync-pkg/nested";

const require = createRequire(import.meta.url);
const required = require("module-sync-pkg");
const requiredNested = require("module-sync-pkg/nested");

console.log(value.source);
console.log(nested.source);
console.log(required.default.source);
console.log(requiredNested.default.source);
