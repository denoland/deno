module.exports.add = (a, b) => {
  const { subtract } = require("@denotest/subtract");
  // adds wrong
  return subtract(a, -b) * 2;
};
