// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const {
  op_node_internal_binding_stream_wrap,
  ShutdownWrap,
  WriteWrap,
} = __bootstrap.core.ops;

return op_node_internal_binding_stream_wrap(WriteWrap, ShutdownWrap);
})();
