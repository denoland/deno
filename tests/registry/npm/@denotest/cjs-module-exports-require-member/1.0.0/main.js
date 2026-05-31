// Mirrors the shape of graphql-tag@2's main entry: re-exports a single
// named property of an inner module as the whole `module.exports`. The
// inner module attaches the other named exports as properties of that
// same function value.
module.exports = require('./lib/inner.js').gql;
