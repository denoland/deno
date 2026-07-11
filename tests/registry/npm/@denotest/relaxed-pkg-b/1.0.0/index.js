const fs = require("fs");
const path = require("path");

const dataPath = path.join(__dirname, "data.txt");

module.exports.dataPath = dataPath;
module.exports.readOwnData = function readOwnData() {
  return fs.readFileSync(dataPath, "utf8");
};
