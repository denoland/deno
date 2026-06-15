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
#[op2(fast, stack_trace)]
pub fn op_node_http_check_proxy_net(
  state: &mut OpState,
  #[string] hostname: &str,
  port: u16,
  #[string] api_name: &str,
) -> Result<(), PermissionCheckError> {
  match state
    .borrow_mut::<PermissionsContainer>()
    .check_net(&(hostname, Some(port)), api_name)
  {
    // A malformed target host (e.g. invalid characters) cannot be a reachable
    // destination, so it should not surface here as a permission/parse error.
    // Let the request proceed and have node:http's own validation reject it
    // with the proper error (ERR_INVALID_CHAR) instead of masking it.
    Err(PermissionCheckError::HostParse(_)) => Ok(()),
    result => result,
  }
}
