// The entry module is a plain `module.exports = require(X)` re-export
// of a wrapper module that itself uses the member-shape re-export.
// Exercises the recursive path: member-shaped re-exports reached
// through normal CJS re-exports must still resolve to the inner
// member's properties.
module.exports = require("./lib/wrapper.js");
