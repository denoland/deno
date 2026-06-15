// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const {
  op_node_create_pipe,
  op_node_internal_binding_pipe_wrap,
  PipeConnectWrap,
  PipeWrap,
} = __bootstrap.core.ops;

return op_node_internal_binding_pipe_wrap(
  PipeWrap,
  PipeConnectWrap,
  op_node_create_pipe,
);
})();
