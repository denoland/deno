// CommonJS: require() of a transitive dep goes through node resolution
// (ext/node/ops/require.rs -> NodeResolver), NOT the import-map-aware
// RawDenoResolver. Re-export the child's value so the test can observe which
// version was resolved.
module.exports = require("@denotest/different-nested-dep-child").default;
