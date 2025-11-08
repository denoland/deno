const os = require("node:os");
const path = require("node:path");

console.log("require_node.js loading");
console.log(`Platform: ${os.platform()}`);
console.log(`Path separator: ${path.sep}`);

globalThis.__require_node_worked__ = true;
