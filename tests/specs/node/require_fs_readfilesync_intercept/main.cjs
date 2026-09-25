const fs = require("fs");
const path = require("path");

const origReadFileSync = fs.readFileSync;
const targetPath = path.resolve(__dirname, "target.cjs");

// Monkey-patch fs.readFileSync to return different content for target.cjs.
// This pattern is used by tools like @volar/typescript (vue-tsc) to
// transform source files during require().
fs.readFileSync = function (...args) {
  if (args[0] === targetPath) {
    return 'module.exports = "patched";';
  }
  return origReadFileSync.apply(this, args);
};

const result = require("./target.cjs");
console.log(result);

fs.readFileSync = origReadFileSync;
