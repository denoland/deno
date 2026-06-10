// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = __bootstrap;
const {
  HTTPParser,
  op_node_internal_binding_http_parser,
} = core.ops;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { AsyncResource } = core.loadExtScript("ext:deno_node/async_hooks.ts");

return op_node_internal_binding_http_parser(HTTPParser, Buffer, AsyncResource);
})();
