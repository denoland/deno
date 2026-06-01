function gql() {
  return "gql-result";
}
function resetCaches() {
  return "reset";
}
function disableFragmentWarnings() {
  return "disabled";
}

// Attach the named helpers as properties of `gql` so that
// `require('./inner.js').gql` is a function carrying all named exports.
gql.gql = gql;
gql.resetCaches = resetCaches;
gql.disableFragmentWarnings = disableFragmentWarnings;
gql.default = gql;

exports.default = gql;
exports.gql = gql;
exports.resetCaches = resetCaches;
exports.disableFragmentWarnings = disableFragmentWarnings;

Object.defineProperty(exports, '__esModule', { value: true });
