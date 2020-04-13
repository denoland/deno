/* eslint-disable */
const fs = require("fs");
const util = require("util");
const path = require("path");

module.exports = {
  readFileSync: fs.readFileSync,
  isNull: util.isNull,
  extname: path.extname,
};
