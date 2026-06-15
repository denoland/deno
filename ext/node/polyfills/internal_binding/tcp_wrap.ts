// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const {
  op_node_internal_binding_tcp_wrap,
  TCPConnectWrap,
  TCPWrap,
} = __bootstrap.core.ops;

return op_node_internal_binding_tcp_wrap(TCPWrap, TCPConnectWrap);
})();
