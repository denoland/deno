// Mirrors graphql-tag@2's main entry exactly: re-export a single named
// property of an inner UMD module as the whole `module.exports`.
module.exports = require('./lib/inner.umd.js').gql;
