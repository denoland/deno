use super::dispatch_minimal::wrap_minimal_op;
use crate::deno_error;
use crate::resources;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use crate::tokio_write;
use deno::CoreOp;
use deno::ErrBox;
use deno::PinnedBuf;
use futures::Future;

// Read

pub struct OpRead;

impl DenoOpDispatcher for OpRead {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_minimal_op(
      move |rid, zero_copy| {
        debug!("read rid={}", rid);
        let zero_copy = match zero_copy {
          None => {
            return Box::new(futures::future::err(
              deno_error::no_buffer_specified(),
            ))
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
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "read";
}

// Write

pub struct OpWrite;

impl DenoOpDispatcher for OpWrite {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_minimal_op(
      move |rid, zero_copy| {
        debug!("write rid={}", rid);
        let zero_copy = match zero_copy {
          None => {
            return Box::new(futures::future::err(
              deno_error::no_buffer_specified(),
            ))
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
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "write";
}
