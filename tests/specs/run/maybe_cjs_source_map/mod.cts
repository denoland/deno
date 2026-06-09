function fail() {
  throw new Error("source map from cjs");
}

exports.run = function run() {
  fail();
};
