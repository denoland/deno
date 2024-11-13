module.exports.add = function (a, b) {
  return require("./subtract.ts").subtract(a, -b);
};
