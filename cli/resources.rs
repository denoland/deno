// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::deno_error::bad_resource;
use crate::http_body::HttpBody;
use crate::repl::Repl;
use crate::state::WorkerChannels;

use deno::Buf;
use deno::ErrBox;

use futures;
use futures::Future;
use futures::Poll;
use futures::Sink;
use futures::Stream;
use reqwest::r#async::Decoder as ReqwestDecoder;
use std;
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::io::{Error, Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::process::ExitStatus;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_process;
use tokio_rustls::client::TlsStream;

pub type ResourceId = u32; // Sometimes referred to RID.

// These store Deno's file descriptors. These are not necessarily the operating
// system ones.
type ResourceTable = BTreeMap<ResourceId, Box<Repr>>;
type NewResourceTable = BTreeMap<ResourceId, Box<dyn NewResource>>;

#[cfg(not(windows))]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

#[cfg(windows)]
extern crate winapi;

lazy_static! {
  // Starts at 3 because stdio is [0-2].
  static ref NEXT_RID: AtomicUsize = AtomicUsize::new(3);
  pub static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new({
    let mut m = BTreeMap::new();
    // TODO Load these lazily during lookup?
    m.insert(0, Box::new(Repr::Stdin(tokio::io::stdin())));

    m.insert(1, Box::new(Repr::Stdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    m.insert(2, Box::new(Repr::Stderr(tokio::io::stderr())));
    m
  });

  pub static ref NEW_RESOURCE_TABLE: Mutex<NewResourceTable> = Mutex::new({
    let mut m: BTreeMap<ResourceId, Box<dyn NewResource>> = BTreeMap::new();
    // TODO Load these lazily during lookup?
    m.insert(0, Box::new(NewStdin(tokio::io::stdin())));

    m.insert(1, Box::new(NewStdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    m.insert(2, Box::new(NewStderr(tokio::io::stderr())));
    m
  });
}

// Internal representation of Resource.
pub enum Repr {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
  // Since TcpListener might be closed while there is a pending accept task,
  // we need to track the task so that when the listener is closed,
  // this pending task could be notified and die.
  // Currently TcpListener itself does not take care of this issue.
  // See: https://github.com/tokio-rs/tokio/issues/846
  TcpListener(tokio::net::TcpListener, Option<futures::task::Task>),
  TcpStream(tokio::net::TcpStream),
  TlsStream(Box<TlsStream<TcpStream>>),
  HttpBody(HttpBody),
  Repl(Arc<Mutex<Repl>>),
  // Enum size is bounded by the largest variant.
  // Use `Box` around large `Child` struct.
  // https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant
  Child(Box<tokio_process::Child>),
  ChildStdin(tokio_process::ChildStdin),
  ChildStdout(tokio_process::ChildStdout),
  ChildStderr(tokio_process::ChildStderr),
  Worker(WorkerChannels),
}

pub trait NewResource: Any + Send {
  fn close(&self) {
    unimplemented!();
  }

  fn inspect_repr(&self) -> &str {
    "repr"
  }
}

impl dyn NewResource {
  pub fn downcast_ref<T: NewResource>(&self) -> Option<&T> {
    if Any::type_id(self) == TypeId::of::<T>() {
      let target = self as *const Self as *const T;
      let target = unsafe { &*target };
      Some(target)
    } else {
      None
    }
  }

  pub fn downcast_mut<T: NewResource>(&mut self) -> Option<&mut T> {
    if Any::type_id(self) == TypeId::of::<T>() {
      let target = self as *mut Self as *mut T;
      let target = unsafe { &mut *target };
      Some(target)
    } else {
      None
    }
  }
}

struct NewStdin(tokio::io::Stdin);

impl NewResource for NewStdin {}

struct NewStdout(tokio::fs::File);

impl NewResource for NewStdout {}

struct NewStderr(tokio::io::Stderr);

impl NewResource for NewStderr {}

struct NewFsFile(tokio::fs::File);

impl NewResource for NewFsFile {}

// Since TcpListener might be closed while there is a pending accept task,
// we need to track the task so that when the listener is closed,
// this pending task could be notified and die.
// Currently TcpListener itself does not take care of this issue.
// See: https://github.com/tokio-rs/tokio/issues/846
struct NewTcpListener(tokio::net::TcpListener, Option<futures::task::Task>);

impl NewResource for NewTcpListener {}

struct NewTcpStream(tokio::net::TcpStream);

impl NewResource for NewTcpStream {}

struct NewTlsStream(TlsStream<TcpStream>);

impl NewResource for NewTlsStream {}

struct NewHttpBody(HttpBody);

impl NewResource for NewHttpBody {}

struct NewRepl(Arc<Mutex<Repl>>);

impl NewResource for NewRepl {}

struct NewChild(tokio_process::Child);

impl NewResource for NewChild {}

struct NewChildStdin(tokio_process::ChildStdin);

impl NewResource for NewChildStdin {}

struct NewChildStdout(tokio_process::ChildStdout);

impl NewResource for NewChildStdout {}

struct NewChildStderr(tokio_process::ChildStderr);

impl NewResource for NewChildStderr {}

struct NewWorker(WorkerChannels);

impl NewResource for NewWorker {}

/// If the given rid is open, this returns the type of resource, E.G. "worker".
/// If the rid is closed or was never open, it returns None.
pub fn get_type(rid: ResourceId) -> Option<String> {
  let table = NEW_RESOURCE_TABLE.lock().unwrap();
  table.get(&rid).map(|r| new_inspect_repr(r.clone()))
}

pub fn table_entries() -> Vec<(u32, String)> {
  let table = NEW_RESOURCE_TABLE.lock().unwrap();

  table
    .iter()
    .map(|(key, value)| (*key, new_inspect_repr(&value)))
    .collect()
}

#[test]
fn test_table_entries() {
  let mut entries = table_entries();
  entries.sort();
  assert_eq!(entries[0], (0, String::from("stdin")));
  assert_eq!(entries[1], (1, String::from("stdout")));
  assert_eq!(entries[2], (2, String::from("stderr")));
}

fn new_inspect_repr(resource: &Box<dyn NewResource>) -> String {
  String::from(resource.inspect_repr())
}

// Abstract async file interface.
// Ideally in unix, if Resource represents an OS rid, it will be the same.
#[derive(Clone, Debug)]
pub struct Resource {
  pub rid: ResourceId,
}

impl Resource {
  // TODO Should it return a Resource instead of net::TcpStream?
  pub fn poll_accept(&mut self) -> Poll<(TcpStream, SocketAddr), Error> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);

    match maybe_repr {
      None => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Listener has been closed",
      )),
      Some(repr) => match repr.downcast_mut::<NewTcpListener>() {
        Some(ref mut listener) => {
          let stream = &mut listener.0;
          stream.poll_accept()
        }
        _ => panic!("Cannot accept"),
      },
    }
  }

  /// Track the current task (for TcpListener resource).
  /// Throws an error if another task is already tracked.
  pub fn track_task(&mut self) -> Result<(), std::io::Error> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    // Only track if is TcpListener.
    let repr = match table.get_mut(&self.rid) {
      Some(repr) => repr,
      None => return Ok(()),
    };

    match repr.downcast_mut::<NewTcpListener>() {
      Some(ref mut stream) => {
        let t = &mut stream.1;
        // Currently, we only allow tracking a single accept task for a listener.
        // This might be changed in the future with multiple workers.
        // Caveat: TcpListener by itself also only tracks an accept task at a time.
        // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
        if t.is_some() {
          return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Another accept task is ongoing",
          ));
        }
        t.replace(futures::task::current());
      }
      _ => {}
    }

    Ok(())
  }

  /// Stop tracking a task (for TcpListener resource).
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&mut self) {
    // Only untrack if is TcpListener.
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = match table.get_mut(&self.rid) {
      Some(repr) => repr,
      None => panic!("bad resource"),
    };

    // If TcpListener, we must kill all pending accepts!
    match repr.downcast_mut::<NewTcpListener>() {
      Some(ref mut stream) => {
        let t = &mut stream.1;
        if t.is_some() {
          t.take();
        }
      }
      None => panic!("bad resource"),
    }
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the RESOURCE_TABLE.
  pub fn close(&self) {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.remove(&self.rid).unwrap();
    // If TcpListener, we must kill all pending accepts!
    if let Some(stream) = repr.downcast_ref::<NewTcpListener>() {
      if let Some(t) = &stream.1 {
        // Call notify on the tracked task, so that they would error out.
        t.notify();
      }
    }
  }

  pub fn shutdown(&mut self, how: Shutdown) -> Result<(), ErrBox> {
    let table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get(&self.rid).ok_or_else(bad_resource)?;
    let stream = &repr
      .downcast_ref::<NewTcpStream>()
      .ok_or_else(bad_resource)?;
    TcpStream::shutdown(&stream.0, how).map_err(ErrBox::from)
  }
}

impl Read for Resource {
  fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
    unimplemented!();
  }
}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox>;
}

impl DenoAsyncRead for Resource {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let r = None
      .or_else(|| repr.downcast_mut::<NewFsFile>().map(|f| f.0.poll_read(buf)))
      .or_else(|| repr.downcast_mut::<NewStdin>().map(|f| f.0.poll_read(buf)))
      .or_else(|| {
        repr
          .downcast_mut::<NewTcpStream>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewTlsStream>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewHttpBody>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewChildStdout>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewChildStderr>()
          .map(|f| f.0.poll_read(buf))
      });

    match r {
      Some(r) => r.map_err(ErrBox::from),
      _ => Err(bad_resource()),
    }
  }
}

impl Write for Resource {
  fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
    unimplemented!()
  }

  fn flush(&mut self) -> std::io::Result<()> {
    unimplemented!()
  }
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox>;

  fn shutdown(&mut self) -> Poll<(), ErrBox>;
}

impl DenoAsyncWrite for Resource {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let r = None
      .or_else(|| {
        repr
          .downcast_mut::<NewFsFile>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewStdout>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewStderr>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewTcpStream>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewTlsStream>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<NewChildStdin>()
          .map(|f| f.0.poll_write(buf))
      });

    match r {
      Some(r) => r.map_err(ErrBox::from),
      _ => Err(bad_resource()),
    }
  }

  fn shutdown(&mut self) -> futures::Poll<(), ErrBox> {
    unimplemented!()
  }
}

fn new_rid() -> ResourceId {
  let next_rid = NEXT_RID.fetch_add(1, Ordering::SeqCst);
  next_rid as ResourceId
}

pub fn add_new_resource(resource: Box<dyn NewResource>) -> Resource {
  let rid = new_rid();
  let mut tg = NEW_RESOURCE_TABLE.lock().unwrap();
  let r = tg.insert(rid, resource);
  assert!(r.is_none());
  Resource { rid }
}

pub fn add_fs_file(fs_file: tokio::fs::File) -> Resource {
  add_new_resource(Box::new(NewFsFile(fs_file)))
}

pub fn add_tcp_listener(listener: tokio::net::TcpListener) -> Resource {
  add_new_resource(Box::new(NewTcpListener(listener, None)))
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> Resource {
  add_new_resource(Box::new(NewTcpStream(stream)))
}

pub fn add_tls_stream(stream: TlsStream<TcpStream>) -> Resource {
  add_new_resource(Box::new(NewTlsStream(stream)))
}

pub fn add_reqwest_body(body: ReqwestDecoder) -> Resource {
  let body = HttpBody::from(body);
  add_new_resource(Box::new(NewHttpBody(body)))
}

pub fn add_repl(repl: Repl) -> Resource {
  add_new_resource(Box::new(NewRepl(Arc::new(Mutex::new(repl)))))
}

pub fn add_worker(wc: WorkerChannels) -> Resource {
  add_new_resource(Box::new(NewWorker(wc)))
}

/// Post message to worker as a host or privilged overlord
pub fn post_message_to_worker(
  rid: ResourceId,
  buf: Buf,
) -> futures::sink::Send<mpsc::Sender<Buf>> {
  let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
  let repr = match table.get_mut(&rid) {
    Some(repr) => repr,
    // TODO: replace this panic with `bad_resource`
    _ => panic!("bad resource"),
  };
  let worker = &mut match repr.downcast_mut::<NewWorker>() {
    Some(w) => w,
    None => panic!("bad resource"),
  };
  let wc = &mut worker.0;
  // unwrap here is incorrect, but doing it anyway
  wc.0.clone().send(buf)
}

pub struct WorkerReceiver {
  rid: ResourceId,
}

// Invert the dumbness that tokio_process causes by making Child itself a future.
impl Future for WorkerReceiver {
  type Item = Option<Buf>;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Option<Buf>, ErrBox> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let worker =
      &mut repr.downcast_mut::<NewWorker>().ok_or_else(bad_resource)?;
    let wc = &mut worker.0;
    wc.1.poll().map_err(ErrBox::from)
  }
}

pub fn get_message_from_worker(rid: ResourceId) -> WorkerReceiver {
  WorkerReceiver { rid }
}

pub struct WorkerReceiverStream {
  rid: ResourceId,
}

// Invert the dumbness that tokio_process causes by making Child itself a future.
impl Stream for WorkerReceiverStream {
  type Item = Buf;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Option<Buf>, ErrBox> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let worker =
      &mut repr.downcast_mut::<NewWorker>().ok_or_else(bad_resource)?;
    let wc = &mut worker.0;
    wc.1.poll().map_err(ErrBox::from)
  }
}

pub fn get_message_stream_from_worker(rid: ResourceId) -> WorkerReceiverStream {
  WorkerReceiverStream { rid }
}

pub struct ChildResources {
  pub child_rid: ResourceId,
  pub stdin_rid: Option<ResourceId>,
  pub stdout_rid: Option<ResourceId>,
  pub stderr_rid: Option<ResourceId>,
}

// TODO: move to process
pub fn add_child(mut c: tokio_process::Child) -> ChildResources {
  let child_rid = new_rid();
  let mut tg = NEW_RESOURCE_TABLE.lock().unwrap();

  let mut resources = ChildResources {
    child_rid,
    stdin_rid: None,
    stdout_rid: None,
    stderr_rid: None,
  };

  if c.stdin().is_some() {
    let stdin = c.stdin().take().unwrap();
    let rid = new_rid();
    let r = tg.insert(rid, Box::new(NewChildStdin(stdin)));
    assert!(r.is_none());
    resources.stdin_rid = Some(rid);
  }
  if c.stdout().is_some() {
    let stdout = c.stdout().take().unwrap();
    let rid = new_rid();
    let r = tg.insert(rid, Box::new(NewChildStdout(stdout)));
    assert!(r.is_none());
    resources.stdout_rid = Some(rid);
  }
  if c.stderr().is_some() {
    let stderr = c.stderr().take().unwrap();
    let rid = new_rid();
    let r = tg.insert(rid, Box::new(NewChildStderr(stderr)));
    assert!(r.is_none());
    resources.stderr_rid = Some(rid);
  }

  let r = tg.insert(child_rid, Box::new(NewChild(c)));
  assert!(r.is_none());

  resources
}

pub struct ChildStatus {
  rid: ResourceId,
}

// Invert the dumbness that tokio_process causes by making Child itself a future.
impl Future for ChildStatus {
  type Item = ExitStatus;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<ExitStatus, ErrBox> {
    let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let child = repr.downcast_mut::<NewChild>().ok_or_else(bad_resource)?;
    child.0.poll().map_err(ErrBox::from)
  }
}

pub fn child_status(rid: ResourceId) -> Result<ChildStatus, ErrBox> {
  let table = NEW_RESOURCE_TABLE.lock().unwrap();
  let repr = table.get(&rid).ok_or_else(bad_resource)?;
  let _child = &repr.downcast_ref::<NewChild>().ok_or_else(bad_resource)?;
  Ok(ChildStatus { rid })
}

pub fn get_repl(rid: ResourceId) -> Result<Arc<Mutex<Repl>>, ErrBox> {
  let table = NEW_RESOURCE_TABLE.lock().unwrap();
  let repr = table.get(&rid).ok_or_else(bad_resource)?;
  let repl = &repr.downcast_ref::<NewRepl>().ok_or_else(bad_resource)?;
  Ok(repl.0.clone())
}

// TODO: revamp this after the following lands:
// https://github.com/tokio-rs/tokio/pull/785
pub fn get_file(rid: ResourceId) -> Result<std::fs::File, ErrBox> {
  let mut table = NEW_RESOURCE_TABLE.lock().unwrap();
  // We take ownership of File here.
  // It is put back below while still holding the lock.
  let mut repr = table.remove(&rid).ok_or_else(bad_resource)?;
  let fs_file = repr.downcast::<NewFsFile>().ok_or_else(bad_resource)?;
  // Trait Clone not implemented on tokio::fs::File,
  // so convert to std File first.
  // TODO:
  return Err(bad_resource());
  //  let std_file = fs_file.0.into_std();
  //  // Create a copy and immediately put back.
  //  // We don't want to block other resource ops.
  //  // try_clone() would yield a copy containing the same
  //  // underlying fd, so operations on the copy would also
  //  // affect the one in resource table, and we don't need
  //  // to write back.
  //  let maybe_std_file_copy = std_file.try_clone();
  //  // Insert the entry back with the same rid.
  //  table.insert(
  //    rid,
  //    Box::new(NewFsFile(tokio_fs::File::from_std(std_file))),
  //  );
  //
  //  maybe_std_file_copy.map_err(ErrBox::from)
}

pub fn lookup(rid: ResourceId) -> Result<Resource, ErrBox> {
  debug!("resource lookup {}", rid);
  let table = NEW_RESOURCE_TABLE.lock().unwrap();
  table
    .get(&rid)
    .ok_or_else(bad_resource)
    .map(|_| Resource { rid })
}
