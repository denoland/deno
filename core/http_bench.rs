/// To run this benchmark:
///
/// > DENO_BUILD_MODE=release ./tools/build.py && \
///   ./target/release/deno_core_http_bench --multi-thread
extern crate deno_core;
extern crate futures;
extern crate libc;
extern crate tokio;

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use deno_core::deno_buf;
use deno_core::AsyncResult;
use deno_core::Isolate;
use deno_core::JSError;
use deno_core::Op;
use deno_core::RECORD_OFFSET_ARG;
use deno_core::RECORD_OFFSET_OP;
use deno_core::RECORD_OFFSET_PROMISE_ID;
use deno_core::RECORD_OFFSET_RESULT;
use futures::future::lazy;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tokio::prelude::*;

const OP_LISTEN: i32 = 1;
const OP_ACCEPT: i32 = 2;
const OP_READ: i32 = 3;
const OP_WRITE: i32 = 4;
const OP_CLOSE: i32 = 5;

fn main() {
  let js_source = include_str!("http_bench.js");
  let isolate = deno_core::Isolate::new(recv_cb);

  let main_future = lazy(move || {
    // TODO currently isolate.execute() must be run inside tokio, hence the
    // lazy(). It would be nice to not have that contraint. Probably requires
    // using v8::MicrotasksPolicy::kExplicit
    js_check(isolate.execute("http_bench.js", js_source));
    isolate.then(|r| {
      js_check(r);
      Ok(())
    })
  });

  let args: Vec<String> = env::args().collect();
  if args.len() > 1 && args[1] == "--multi-thread" {
    println!("multi-thread");
    tokio::run(main_future);
  } else {
    println!("single-thread");
    tokio::runtime::current_thread::run(main_future);
  }
}

enum Repr {
  TcpListener(tokio::net::TcpListener),
  TcpStream(tokio::net::TcpStream),
}

type ResourceTable = HashMap<i32, Repr>;
lazy_static! {
  static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new(HashMap::new());
  static ref NEXT_RID: AtomicUsize = AtomicUsize::new(3);
}

fn new_rid() -> i32 {
  let rid = NEXT_RID.fetch_add(1, Ordering::SeqCst);
  rid as i32
}

fn recv_cb(isolate: &mut Isolate, zero_copy_buf: deno_buf) {
  isolate.test_send_counter += 1; // TODO ideally store this in isolate.state?

  let promise_id = isolate.shared.get_record(0, RECORD_OFFSET_PROMISE_ID);
  let op_id = isolate.shared.get_record(0, RECORD_OFFSET_OP);
  let arg = isolate.shared.get_record(0, RECORD_OFFSET_ARG);

  // dbg!(promise_id);
  // dbg!(op_id);
  // dbg!(arg);

  let is_sync = promise_id == 0;

  if is_sync {
    // sync ops
    match op_id {
      OP_CLOSE => {
        debug!("close");
        assert!(is_sync);
        let mut table = RESOURCE_TABLE.lock().unwrap();
        let r = table.remove(&arg);
        isolate.shared.set_record(
          0,
          RECORD_OFFSET_RESULT,
          if r.is_some() { 0 } else { -1 },
        );
      }
      OP_LISTEN => {
        debug!("listen");
        assert!(is_sync);

        let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
        let listener = tokio::net::TcpListener::bind(&addr).unwrap();
        let rid = new_rid();
        isolate.shared.set_record(0, RECORD_OFFSET_RESULT, rid);
        let mut guard = RESOURCE_TABLE.lock().unwrap();
        guard.insert(rid, Repr::TcpListener(listener));
      }
      _ => panic!("bad op"),
    }
  } else {
    // async ops
    let zero_copy_id = zero_copy_buf.zero_copy_id;
    let op = match op_id {
      OP_ACCEPT => {
        let listener_rid = arg;
        op_accept(listener_rid)
      }
      OP_READ => {
        let rid = arg;
        op_read(rid, zero_copy_buf)
      }
      OP_WRITE => {
        let rid = arg;
        op_write(rid, zero_copy_buf)
      }
      _ => panic!("bad op"),
    };
    isolate.add_op(promise_id, op, zero_copy_id);
  }
}

fn op_accept(listener_rid: i32) -> Box<Op> {
  debug!("accept {}", listener_rid);
  Box::new(
    futures::future::poll_fn(move || {
      let mut table = RESOURCE_TABLE.lock().unwrap();
      let maybe_repr = table.get_mut(&listener_rid);
      match maybe_repr {
        Some(Repr::TcpListener(ref mut listener)) => listener.poll_accept(),
        _ => panic!("bad rid"),
      }
    }).and_then(move |(stream, addr)| {
      debug!("accept success {}", addr);
      let rid = new_rid();

      let mut guard = RESOURCE_TABLE.lock().unwrap();
      guard.insert(rid, Repr::TcpStream(stream));

      Ok(AsyncResult { result: rid })
    }),
  )
}

fn op_read(rid: i32, mut zero_copy_buf: deno_buf) -> Box<Op> {
  debug!("read rid={}", rid);
  Box::new(
    futures::future::poll_fn(move || {
      let mut table = RESOURCE_TABLE.lock().unwrap();
      let maybe_repr = table.get_mut(&rid);
      match maybe_repr {
        Some(Repr::TcpStream(ref mut stream)) => {
          stream.poll_read(&mut zero_copy_buf)
        }
        _ => panic!("bad rid"),
      }
    }).and_then(move |nread| {
      debug!("read success {}", nread);
      Ok(AsyncResult {
        result: nread as i32,
      })
    }),
  )
}

fn op_write(rid: i32, zero_copy_buf: deno_buf) -> Box<Op> {
  debug!("write rid={}", rid);
  Box::new(
    futures::future::poll_fn(move || {
      let mut table = RESOURCE_TABLE.lock().unwrap();
      let maybe_repr = table.get_mut(&rid);
      match maybe_repr {
        Some(Repr::TcpStream(ref mut stream)) => {
          stream.poll_write(&zero_copy_buf)
        }
        _ => panic!("bad rid"),
      }
    }).and_then(move |nwritten| {
      debug!("write success {}", nwritten);
      Ok(AsyncResult {
        result: nwritten as i32,
      })
    }),
  )
}

fn js_check(r: Result<(), JSError>) {
  if let Err(e) = r {
    panic!(e.to_string());
  }
}
