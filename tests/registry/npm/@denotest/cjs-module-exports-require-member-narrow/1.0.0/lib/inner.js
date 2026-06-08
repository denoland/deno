function safe() {
  return "safe";
}
function unrelated() {
  return "unrelated";
}

// Only `attached` is attached to `safe`. `loose` is exported by
// inner.js but NOT a property of `safe`. The wrapper does
// `module.exports = require('./lib/inner.js').safe`, so the
// wrapper's named exports should be limited to `safe`'s own
// properties, not all of inner's named exports.
safe.attached = function attached() {
  return "attached";
};

exports.safe = safe;
exports.loose = unrelated;
