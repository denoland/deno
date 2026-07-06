// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;

// When a node:http / node:https request is routed through a proxy, the socket
// is connected to the proxy endpoint, so the proxy is the only host the connect
// op permission-checks. Without this, `--allow-net=<proxy>` alone would let a
// request reach a target host that is outside `--allow-net` or explicitly in
// `--deny-net`. Enforce `--allow-net` for the request target here, mirroring
// the target check fetch() performs, before the connection to the proxy is
// established.
//
// This fails closed: a denied or unparseable target propagates its error,
// matching check_net_url() in fetch(). Targets node:http rejects on its own
// (invalid header characters such as CR/LF, which would also fail check_net's
// host parser) are filtered out before this op is called, so they surface as
// ERR_INVALID_CHAR rather than being masked here.
#[op2(fast, stack_trace)]
pub fn op_node_http_check_proxy_net(
  state: &mut OpState,
  #[string] hostname: &str,
  port: u16,
  #[string] api_name: &str,
) -> Result<(), PermissionCheckError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_net(&(hostname, Some(port)), api_name)
}
