use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::ops::minimal_op;
use crate::resources;
use crate::state::ThreadSafeState;
use crate::tokio_read;
use crate::tokio_write;
use deno::*;
use futures::Future;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("read", s.core_op(minimal_op(op_read)));
  i.register_op("write", s.core_op(minimal_op(op_write)));
}

pub fn op_read(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return Box::new(futures::future::err(deno_error::no_buffer_specified()))
    }
    Some(buf) => buf,
  };

  match resources::lookup(rid as u32) {
    Err(e) => Box::new(futures::future::err(e)),
    Ok(resource) => Box::new(
      tokio_read::read(resource, zero_copy)
        .map_err(ErrBox::from)
        .and_then(move |(_resource, _buf, nread)| Ok(nread as i32)),
    ),
  }
}

pub fn op_write(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
  debug!("write rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return Box::new(futures::future::err(deno_error::no_buffer_specified()))
    }
    Some(buf) => buf,
  };

  match resources::lookup(rid as u32) {
    Err(e) => Box::new(futures::future::err(e)),
    Ok(resource) => Box::new(
      tokio_write::write(resource, zero_copy)
        .map_err(ErrBox::from)
        .and_then(move |(_resource, _buf, nwritten)| Ok(nwritten as i32)),
    ),
  }
}
