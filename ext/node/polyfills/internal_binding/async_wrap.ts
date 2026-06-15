// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const {
  AsyncWrap,
  op_node_internal_binding_async_wrap,
  op_node_new_async_id,
} = __bootstrap.core.ops;

return op_node_internal_binding_async_wrap(AsyncWrap, op_node_new_async_id);
})();
