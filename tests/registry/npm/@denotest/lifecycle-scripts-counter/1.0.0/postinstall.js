const fs = require("fs");
const path = require("path");

const initCwd = process.env.INIT_CWD || process.cwd();
const counterPath = path.resolve(initCwd, "..", "lifecycle-counter.txt");
fs.appendFileSync(counterPath, "run\n");
fs.writeFileSync(
  path.join(__dirname, "message.js"),
  "module.exports = \"postinstall works\";\n",
);
