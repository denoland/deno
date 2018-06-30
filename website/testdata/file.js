/**
 * Some comments
 */
function foo() {}

function youWillNeverSeeThis() {}

/**
 * nothing.
 */
function defaultExport() {}

exports.foo = foo;
exports = youWillNeverSeeThis;
module.exports = defaultExport;
