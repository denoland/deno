console.log("require_first.js loading");

const os = require("node:os");

console.log(`require_first.js platform: ${os.platform()}`);

globalThis.__first__ = "first";
globalThis.__first_filename__ = __filename;
globalThis.__first_dirname__ = __dirname;
