const fs = require("node:fs");
const path = require("node:path");

// Reads a data file bundled inside this npm package. This should not require
// `--allow-read` because the file lives inside the package's own directory.
module.exports.readData = function () {
  return fs.readFileSync(path.join(__dirname, "data.txt"), "utf8").trim();
};
