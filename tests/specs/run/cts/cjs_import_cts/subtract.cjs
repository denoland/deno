module.exports.subtract = function (a, b) {
  return require("./add.cts").add(a, -b);
};
