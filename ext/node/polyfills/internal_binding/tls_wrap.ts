// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = __bootstrap;
const { PipeWrap, TLSWrap, op_node_internal_binding_tls_wrap } = core.ops;
const { streamBaseState } = core.loadExtScript(
  "ext:deno_node/internal_binding/stream_wrap.ts",
);

return op_node_internal_binding_tls_wrap(TLSWrap, PipeWrap, streamBaseState);
})();
