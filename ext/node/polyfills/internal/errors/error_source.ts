// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.
// deno-fmt-ignore-file

(function () {
const { core } = globalThis.__bootstrap;
const { op_node_get_first_expression } = core.ops;

return { getErrorSourceExpression: op_node_get_first_expression };
})()
