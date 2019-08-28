// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::deno_error;
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::resources::Resource;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use std;
use std::convert::From;
use std::net::Shutdown;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

// Accept

pub struct OpAccept;

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
}

impl DenoOpDispatcher for OpAccept {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: AcceptArgs = serde_json::from_value(args)?;
        let server_rid = args.rid as u32;

        match resources::lookup(server_rid) {
          None => Err(deno_error::bad_resource()),
          Some(server_resource) => {
            let op = tokio_util::accept(server_resource)
              .and_then(move |(tcp_stream, _socket_addr)| {
                let local_addr = tcp_stream.local_addr()?;
                let remote_addr = tcp_stream.peer_addr()?;
                let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
                Ok((tcp_stream_resource, local_addr, remote_addr))
              })
              .map_err(ErrBox::from)
              .and_then(
                move |(tcp_stream_resource, local_addr, remote_addr)| {
                  futures::future::ok(json!({
                    "rid": tcp_stream_resource.rid,
                    "localAddr": local_addr.to_string(),
                    "remoteAddr": remote_addr.to_string(),
                  }))
                },
              );

            Ok(JsonOp::Async(Box::new(op)))
          }
        }
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "accept";
}

// Dial

pub struct OpDial;

#[derive(Deserialize)]
struct DialArgs {
  network: String,
  address: String,
}

impl DenoOpDispatcher for OpDial {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: DialArgs = serde_json::from_value(args)?;
        let network = args.network;
        assert_eq!(network, "tcp"); // TODO Support others.
        let address = args.address;

        state.check_net(&address)?;

        let op = resolve_addr(&address).and_then(move |addr| {
          TcpStream::connect(&addr)
            .map_err(ErrBox::from)
            .and_then(move |tcp_stream| {
              let local_addr = tcp_stream.local_addr()?;
              let remote_addr = tcp_stream.peer_addr()?;
              let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
              Ok((tcp_stream_resource, local_addr, remote_addr))
            })
            .map_err(ErrBox::from)
            .and_then(move |(tcp_stream_resource, local_addr, remote_addr)| {
              futures::future::ok(json!({
                "rid": tcp_stream_resource.rid,
                "localAddr": local_addr.to_string(),
                "remoteAddr": remote_addr.to_string(),
              }))
            })
        });

        Ok(JsonOp::Async(Box::new(op)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "dial";
}

// Shutdown

pub struct OpShutdown;

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

impl DenoOpDispatcher for OpShutdown {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ShutdownArgs = serde_json::from_value(args)?;

        let rid = args.rid;
        let how = args.how;
        match resources::lookup(rid as u32) {
          None => Err(deno_error::bad_resource()),
          Some(mut resource) => {
            let shutdown_mode = match how {
              0 => Shutdown::Read,
              1 => Shutdown::Write,
              _ => unimplemented!(),
            };

            // Use UFCS for disambiguation
            Resource::shutdown(&mut resource, shutdown_mode)?;
            Ok(JsonOp::Sync(json!({})))
          }
        }
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "shutdown";
}

// Listen

pub struct OpListen;

#[derive(Deserialize)]
struct ListenArgs {
  network: String,
  address: String,
}

impl DenoOpDispatcher for OpListen {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ListenArgs = serde_json::from_value(args)?;

        let network = args.network;
        assert_eq!(network, "tcp");
        let address = args.address;

        state.check_net(&address)?;

        let addr = resolve_addr(&address).wait()?;
        let listener = TcpListener::bind(&addr)?;
        let local_addr = listener.local_addr()?;
        let resource = resources::add_tcp_listener(listener);

        Ok(JsonOp::Sync(json!({
          "rid": resource.rid,
          "localAddr": local_addr.to_string()
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "listen";
}
