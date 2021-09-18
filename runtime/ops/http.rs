use std::rc::Rc;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_net::io::TcpStreamResource;
use deno_net::io::TlsStreamResource;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![("op_http_start", op_sync(op_http_start))])
    .build()
}

fn op_http_start(
  state: &mut OpState,
  tcp_stream_rid: ResourceId,
  _: (),
) -> Result<ResourceId, AnyError> {
  if let Ok(resource_rc) = state
    .resource_table
    .take::<TcpStreamResource>(tcp_stream_rid)
  {
    let resource = Rc::try_unwrap(resource_rc)
      .expect("Only a single use of this resource should happen");
    let (read_half, write_half) = resource.into_inner();
    let tcp_stream = read_half.reunite(write_half)?;
    let addr = tcp_stream.local_addr()?;
    return deno_http::start_http(state, tcp_stream, addr, "http");
  }

  if let Ok(resource_rc) = state
    .resource_table
    .take::<TlsStreamResource>(tcp_stream_rid)
  {
    let resource = Rc::try_unwrap(resource_rc)
      .expect("Only a single use of this resource should happen");
    let (read_half, write_half) = resource.into_inner();
    let tls_stream = read_half.reunite(write_half);
    let addr = tls_stream.get_ref().0.local_addr()?;
    return deno_http::start_http(state, tls_stream, addr, "https");
  }

  Err(bad_resource_id())
}
