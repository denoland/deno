// `Object.prototype.__proto__` is restored only during CJS module
// evaluation, so that load-time inheritance setup like
// `Boolean.prototype.__proto__ = Node.prototype` actually mutates the
// real [[Prototype]]. Once the CJS module returns, the accessor is
// deleted again, so non-CJS code keeps the hardened default.
import { createRequire } from "node:module";
console.log("before require:", Object.hasOwn(Object.prototype, "__proto__"));
const require = createRequire(import.meta.url);
require("./inheritance.cjs");
console.log("after require:", Object.hasOwn(Object.prototype, "__proto__"));
