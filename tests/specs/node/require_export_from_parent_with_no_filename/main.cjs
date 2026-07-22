const path = require("node:path");
const Module = require("node:module");
function requireFromString(code, filename) {
  const paths = Module._nodeModulePaths((0, path.dirname)(filename));
  const m = new Module(filename, module.parent);
  m.paths = paths;
  m._compile(code, filename);
  return m.exports;
}

const code = `
const add = require("@denotest/cjs-multiple-exports/add");

console.log(add(1, 2));
`;
requireFromString(code, "fake.js");
