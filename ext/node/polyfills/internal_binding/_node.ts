// Copyright 2018-2026 the Deno authors. MIT license.
// This file contains C++ node globals accessed in internal binding calls

/**
 * Adapted from
 * https://github.com/nodejs/node/blob/3b72788afb7365e10ae1e97c71d1f60ee29f09f2/src/node.h#L728-L738
 */
(function () {
const { op_node_internal_binding_encodings } = __bootstrap.core.ops;
const Encodings = op_node_internal_binding_encodings();

return {
  Encodings,
};
})();
