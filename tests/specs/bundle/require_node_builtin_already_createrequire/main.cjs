const { createRequire } = require("node:module");
const os = require("node:os");
console.log(os.freemem.name);
const require2 = createRequire(import.meta.url);
console.log(require2.name);
