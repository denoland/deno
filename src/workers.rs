// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::cli::Buf;
use crate::cli::Cli;
use crate::flags::DenoFlags;
use crate::isolate::Isolate;
use crate::isolate_state::IsolateState;
use crate::isolate_state::WorkerChannels;
use crate::js_errors::JSErrorColor;
use crate::permissions::DenoPermissions;
use crate::resources;
use crate::tokio_util;
use deno_core::JSError;
use deno_core::StartupData;
use futures::future::lazy;
use futures::sync::mpsc;
use futures::sync::oneshot;
use futures::Future;
use futures::Poll;
use std::sync::Arc;
use std::thread;

/// Rust interface for WebWorkers.
pub struct Worker {
  isolate: Isolate,
}

impl Worker {
  pub fn new(
    startup_data: Option<StartupData>,
    flags: DenoFlags,
    argv: Vec<String>,
    permissions: DenoPermissions,
  ) -> (Self, WorkerChannels) {
    let (worker_in_tx, worker_in_rx) = mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = mpsc::channel::<Buf>(1);

    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);

    let state =
      Arc::new(IsolateState::new(flags, argv, Some(internal_channels)));

    let cli = Cli::new(startup_data, state, permissions);
    let isolate = Isolate::new(cli);

    let worker = Worker { isolate };
    (worker, external_channels)
  }

  pub fn execute(&mut self, js_source: &str) -> Result<(), JSError> {
    self.isolate.execute(js_source)
  }
}

impl Future for Worker {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    self.isolate.poll()
  }
}

pub fn spawn(
  startup_data: Option<StartupData>,
  state: &IsolateState,
  js_source: String,
  permissions: DenoPermissions,
) -> resources::Resource {
  // TODO This function should return a Future, so that the caller can retrieve
  // the JSError if one is thrown. Currently it just prints to stderr and calls
  // exit(1).
  // let (js_error_tx, js_error_rx) = oneshot::channel::<JSError>();
  let (p, c) = oneshot::channel::<resources::Resource>();
  let builder = thread::Builder::new().name("worker".to_string());

  let flags = state.flags.clone();
  let argv = state.argv.clone();

  let _tid = builder
    .spawn(move || {
      tokio_util::run(lazy(move || {
        let (mut worker, external_channels) =
          Worker::new(startup_data, flags, argv, permissions);
        let resource = resources::add_worker(external_channels);
        p.send(resource.clone()).unwrap();

        worker
          .execute("denoMain()")
          .expect("worker denoMain failed");
        worker
          .execute("workerMain()")
          .expect("worker workerMain failed");
        worker.execute(&js_source).expect("worker js_source failed");

        worker.then(move |r| -> Result<(), ()> {
          resource.close();
          debug!("workers.rs after resource close");
          if let Err(err) = r {
            eprintln!("{}", JSErrorColor(&err).to_string());
            std::process::exit(1);
          }
          Ok(())
        })
      }));

      debug!("workers.rs after spawn");
    }).unwrap();

  c.wait().unwrap()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::startup_data;

  #[test]
  fn test_spawn() {
    let startup_data = startup_data::compiler_isolate_init();
    let resource = spawn(
      Some(startup_data),
      &IsolateState::mock(),
      r#"
      onmessage = function(e) {
        let s = new TextDecoder().decode(e.data);;
        console.log("msg from main script", s);
        if (s == "exit") {
          close();
          return;
        } else {
          console.assert(s === "hi");
        }
        postMessage(new Uint8Array([1, 2, 3]));
        console.log("after postMessage");
      }
    "#.into(),
      DenoPermissions::default(),
    );
    let msg = String::from("hi").into_boxed_str().into_boxed_bytes();

    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());

    let maybe_msg =
      resources::worker_recv_message(resource.rid).wait().unwrap();
    assert!(maybe_msg.is_some());
    assert_eq!(*maybe_msg.unwrap(), [1, 2, 3]);

    let msg = String::from("exit").into_boxed_str().into_boxed_bytes();
    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let startup_data = startup_data::compiler_isolate_init();
    let resource = spawn(
      Some(startup_data),
      &IsolateState::mock(),
      "onmessage = () => close();".into(),
      DenoPermissions::default(),
    );

    assert_eq!(
      resources::get_type(resource.rid),
      Some("worker".to_string())
    );

    let msg = String::from("hi").into_boxed_str().into_boxed_bytes();
    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());
    println!("rid {:?}", resource.rid);

    // TODO Need a way to get a future for when a resource closes.
    // For now, just sleep for a bit.
    // resource.close();
    thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(resources::get_type(resource.rid), None);
  }
}
