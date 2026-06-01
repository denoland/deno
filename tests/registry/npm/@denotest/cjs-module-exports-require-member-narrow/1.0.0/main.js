// `module.exports = require(X).MEMBER` shape, but inner.js
// deliberately exposes a named export (`loose`) that is NOT a
// property of the re-exported member (`safe`). The wrapper must
// not advertise `loose` since `mod.loose` would be undefined at
// runtime.
module.exports = require("./lib/inner.js").safe;
