exports = {
  a: "A",
  b: "B",
};
exports.foo = "foo";
exports.bar = "bar";
exports.fizz = require("./reexports.js");

console.log(exports);
