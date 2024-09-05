const process = require("process");
const a = require("./a");

console.log(process.cwd());

module.exports = {
  cwd: process.cwd,
  foobar: a.foobar,
};
