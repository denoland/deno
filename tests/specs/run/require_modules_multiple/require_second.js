console.log("require_second.js loading");

const path = require("node:path");

console.log(`require_second.js path separator: ${path.sep}`);

globalThis.__second__ = "second";
globalThis.__second_filename__ = __filename;
globalThis.__second_dirname__ = __dirname;
