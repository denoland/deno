const process = require("process");

console.log(process.cwd());

module.exports = {
  cwd: process.cwd,
};
