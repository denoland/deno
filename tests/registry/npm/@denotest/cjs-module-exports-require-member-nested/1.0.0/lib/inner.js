function safe() {
  return "safe";
}
function unrelated() {
  return "unrelated";
}

// Only `attached` is attached to `safe`. `loose` is exported by
// inner.js but NOT a property of `safe`. The wrapper does
// `module.exports = require('./inner.js').safe`, so when the entry
// module re-exports the wrapper, only `safe`'s own properties should
// be advertised on the entry.
safe.attached = function attached() {
  return "attached";
};

exports.safe = safe;
exports.loose = unrelated;
