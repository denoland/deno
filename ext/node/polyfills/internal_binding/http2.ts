// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = __bootstrap;
const {
  op_http2_error_string,
  op_node_internal_binding_http2,
} = core.ops;
const constants = core.loadExtScript(
  "ext:deno_node/internal/http2/constants.ts",
);

return op_node_internal_binding_http2(constants, op_http2_error_string);
})();
