"use strict";
exports.__esModule = true;
var _exportNames = { greet: true };
// Re-export everything from a module two directories up via a
// forward-slash relative specifier. On Windows this used to collapse
// to the bare specifier "types" during static analysis because the
// joined path mixed `\` and `/` separators. Regression test for #29910.
var _types = require("../../types");
Object.keys(_types).forEach(function (key) {
  if (key === "default" || key === "__esModule") return;
  if (Object.prototype.hasOwnProperty.call(_exportNames, key)) return;
  if (key in exports && exports[key] === _types[key]) return;
  exports[key] = _types[key];
});
function greet() {
  return "hello " + _types.IMAGE_TYPE;
}
exports.greet = greet;
