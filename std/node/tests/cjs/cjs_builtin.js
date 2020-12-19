// deno-lint-ignore-file no-undef
// deno-lint-ignore-file
const fs = require("fs");
const util = require("util");
const path = require("path");

module.exports = {
  readFileSync: fs.readFileSync,
  isNull: util.isNull,
  extname: path.extname,
};
