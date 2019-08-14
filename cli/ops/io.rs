use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::resources;
use crate::tokio_write;
use deno::ErrBox;
use deno::PinnedBuf;
use futures::Future;

pub fn op_read(rid: i32, zero_copy: Option<PinnedBuf>) -> Box<MinimalOp> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return Box::new(futures::future::err(deno_error::no_buffer_specified()))
    }
    Some(buf) => buf,
  };
  match resources::lookup(rid as u32) {
    None => Box::new(futures::future::err(deno_error::bad_resource())),
    Some(resource) => Box::new(
      tokio::io::read(resource, zero_copy)
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
    None => Box::new(futures::future::err(deno_error::bad_resource())),
    Some(resource) => Box::new(
      tokio_write::write(resource, zero_copy)
        .map_err(ErrBox::from)
        .and_then(move |(_resource, _buf, nwritten)| Ok(nwritten as i32)),
    ),
  }
}
