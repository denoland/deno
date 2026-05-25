// As soon as ESM code pulls in a CJS module via createRequire, the
// accessor is lazily restored so the CJS module behaves like it does
// under Node.
import { createRequire } from "node:module";
console.log("before require:", Object.hasOwn(Object.prototype, "__proto__"));
const require = createRequire(import.meta.url);
require("./inheritance.cjs");
console.log("after require:", Object.hasOwn(Object.prototype, "__proto__"));
