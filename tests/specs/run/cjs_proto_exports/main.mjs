import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

// Test that CJS modules using exports.__proto__ to set up prototype chains
// work correctly even when Object.prototype.__proto__ has been deleted.
const mod = require("./mod.js");

// Direct export should work
console.log("required:", typeof mod.required);

// Inherited exports via __proto__ should also work
console.log("obj:", JSON.stringify(mod.obj !== undefined));
console.log("obj.isplain:", typeof mod.obj.isplain);
console.log("num:", JSON.stringify(mod.num !== undefined));
console.log("num.is:", typeof mod.num.is);
console.log("str:", JSON.stringify(mod.str !== undefined));
console.log("str.is:", typeof mod.str.is);

// Default import should also work
const toi = await import("./mod.js");
console.log("default.required:", typeof toi.default.required);
console.log("default.obj:", JSON.stringify(toi.default.obj !== undefined));
console.log("default.obj.isplain:", typeof toi.default.obj.isplain);
