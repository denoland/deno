const fs = require("fs");
const path = require("path");

const origReadFileSync = fs.readFileSync;
const targetPath = path.resolve(__dirname, "data.json");

fs.readFileSync = function (...args) {
  if (args[0] === targetPath) {
    return '{"value": "patched"}';
  }
  return origReadFileSync.apply(this, args);
};

const result = require("./data.json");
console.log(result.value);

fs.readFileSync = origReadFileSync;
