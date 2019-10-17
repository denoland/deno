// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::deno_error::bad_resource;
use crate::state::WorkerChannels;
use deno::Buf;
use deno::ErrBox;
use downcast_rs::Downcast;
use futures;
use futures::Future;
use futures::Poll;
use futures::Sink;
use futures::Stream;
use std;
use std::any::Any;
use std::collections::BTreeMap;
use std::io::{Error, Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::client::TlsStream;

pub type ResourceId = u32; // Sometimes referred to RID.

// These store Deno's file descriptors. These are not necessarily the operating
// system ones.
type ResourceMap = BTreeMap<ResourceId, Box<dyn DenoResource>>;

#[cfg(not(windows))]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

#[cfg(windows)]
extern crate winapi;

lazy_static! {
  // Starts at 3 because stdio is [0-2].
  static ref NEXT_RID: AtomicUsize = AtomicUsize::new(3);
  pub static ref RESOURCE_TABLE: Mutex<ResourceMap> = Mutex::new({
    let mut m: BTreeMap<ResourceId, Box<dyn DenoResource>> = BTreeMap::new();
    // TODO Load these lazily during lookup?
    m.insert(0, Box::new(ResourceStdin(tokio::io::stdin())));

    m.insert(1, Box::new(ResourceStdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    m.insert(2, Box::new(ResourceStderr(tokio::io::stderr())));
    m
  });
}

//struct ResourceTable {
//  map: Mutex<ResourceMap>,
//  next_rid: AtomicUsize,
//}
//
//
//impl ResourceTable {
//  pub fn add(&self, _resource: Box<dyn DenoResource>) -> Resource {
//    unimplemented!();
//  }
//
//  pub fn get(&self, rid: &ResourceId) -> Result<&Box<dyn DenoResource>, ErrBox> {
//    let m = self.map.lock().unwrap();
//    m.get(rid).ok_or_else(bad_resource)
//  }
//
//  pub fn get_as<T: DenoResource>(&self, rid: &ResourceId) -> Result<&T, ErrBox> {
//    self.get(rid)?.downcast_ref::<T>().ok_or_else(bad_resource)
//  }
//
//  pub fn get_mut(&self, rid: &ResourceId) -> Result<&mut Box<dyn DenoResource>, ErrBox> {
//    let mut m = self.map.lock().unwrap();
//    m.get_mut(rid).ok_or_else(bad_resource)
//  }
//
//  pub fn get_mut_as<T: DenoResource>(&self, rid: &ResourceId) -> Result<&mut T, ErrBox> {
//    self.get_mut(rid)?.downcast_mut::<T>().ok_or_else(bad_resource)
//  }
//
//  pub fn with_resource<F>(&self, rid: &ResourceId, f: F)
//  where
//    F: Fn(&Box<dyn DenoResource>),
//  {
//    let m = self.map.lock().unwrap();
//    let resource = m.get(rid).ok_or_else(bad_resource);
//    f(resource)
//  }
//
//  pub fn with_mut_resource<F>(&self, rid: &ResourceId, f: F) -> Result<(), ErrBox>
//  where
//    F: Fn(&mut Box<dyn DenoResource>),
//  {
//    let mut m = self.map.lock().unwrap();
//    let resource = m.get_mut(rid).ok_or_else(bad_resource)?;
//    f(resource)
//  }
//}

pub fn with_resource<F, R>(rid: &ResourceId, f: F) -> Result<R, ErrBox>
where
  F: FnOnce(&Box<dyn DenoResource>) -> Result<R, ErrBox>,
{
  let table = RESOURCE_TABLE.lock().unwrap();
  let repr = table.get(&rid).ok_or_else(bad_resource)?;
  let rv = f(repr)?;
  Ok(rv)
}

pub fn with_mut_resource<F, R>(rid: &ResourceId, f: F) -> Result<R, ErrBox>
where
  F: FnOnce(&mut Box<dyn DenoResource>) -> Result<R, ErrBox>,
{
  let mut table = RESOURCE_TABLE.lock().unwrap();
  let repr = table.get_mut(&rid).ok_or_else(bad_resource)?;
  let rv = f(repr)?;
  Ok(rv)
}

// TODO: rename
/// Abstract type representing resource in Deno.
pub trait DenoResource: Downcast + Any + Send {
  fn inspect_repr(&self) -> &str {
    unimplemented!();
  }
}
impl_downcast!(DenoResource);

//impl Clone for Box<dyn DenoResource> {
//  fn clone(&self) -> Box<dyn DenoResource> {
//    self.box_clone()
//  }
//}

//impl dyn DenoResource {
//  pub fn downcast_ref<T: DenoResource>(&self) -> Option<&T> {
//    if Any::type_id(self) == TypeId::of::<T>() {
//      let target = self as *const Self as *const T;
//      let target = unsafe { &*target };
//      Some(target)
//    } else {
//      None
//    }
//  }
//
//  pub fn downcast_mut<T: DenoResource>(&mut self) -> Option<&mut T> {
//    if Any::type_id(self) == TypeId::of::<T>() {
//      let target = self as *mut Self as *mut T;
//      let target = unsafe { &mut *target };
//      Some(target)
//    } else {
//      None
//    }
//  }
//}

struct ResourceStdin(tokio::io::Stdin);

impl DenoResource for ResourceStdin {}

struct ResourceStdout(tokio::fs::File);

impl DenoResource for ResourceStdout {}

struct ResourceStderr(tokio::io::Stderr);

impl DenoResource for ResourceStderr {}

struct ResourceFsFile(tokio::fs::File);

impl DenoResource for ResourceFsFile {}

// Since TcpListener might be closed while there is a pending accept task,
// we need to track the task so that when the listener is closed,
// this pending task could be notified and die.
// Currently TcpListener itself does not take care of this issue.
// See: https://github.com/tokio-rs/tokio/issues/846
struct ResourceTcpListener(
  tokio::net::TcpListener,
  Option<futures::task::Task>,
);

impl DenoResource for ResourceTcpListener {}

struct ResourceTcpStream(tokio::net::TcpStream);

impl DenoResource for ResourceTcpStream {}

struct ResourceTlsStream(TlsStream<TcpStream>);

impl DenoResource for ResourceTlsStream {}

struct ResourceWorker(WorkerChannels);

impl DenoResource for ResourceWorker {}

/// If the given rid is open, this returns the type of resource, E.G. "worker".
/// If the rid is closed or was never open, it returns None.
pub fn get_type(rid: ResourceId) -> Option<String> {
  let table = RESOURCE_TABLE.lock().unwrap();
  table.get(&rid).map(inspect_repr)
}

pub fn table_entries() -> Vec<(u32, String)> {
  let table = RESOURCE_TABLE.lock().unwrap();

  table
    .iter()
    .map(|(key, value)| (*key, inspect_repr(&value)))
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

fn inspect_repr(resource: &Box<dyn DenoResource>) -> String {
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);

    match maybe_repr {
      None => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Listener has been closed",
      )),
      Some(repr) => match repr.downcast_mut::<ResourceTcpListener>() {
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    // Only track if is TcpListener.
    let repr = match table.get_mut(&self.rid) {
      Some(repr) => repr,
      None => return Ok(()),
    };

    match repr.downcast_mut::<ResourceTcpListener>() {
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = match table.get_mut(&self.rid) {
      Some(repr) => repr,
      None => panic!("bad resource"),
    };

    // If TcpListener, we must kill all pending accepts!
    match repr.downcast_mut::<ResourceTcpListener>() {
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.remove(&self.rid).unwrap();
    // If TcpListener, we must kill all pending accepts!
    if let Some(stream) = repr.downcast_ref::<ResourceTcpListener>() {
      if let Some(t) = &stream.1 {
        // Call notify on the tracked task, so that they would error out.
        t.notify();
      }
    }
  }

  pub fn shutdown(&mut self, how: Shutdown) -> Result<(), ErrBox> {
    let table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.get(&self.rid).ok_or_else(bad_resource)?;
    let stream = &repr
      .downcast_ref::<ResourceTcpStream>()
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let r = None
      .or_else(|| {
        repr
          .downcast_mut::<ResourceFsFile>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceStdin>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceTcpStream>()
          .map(|f| f.0.poll_read(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceTlsStream>()
          .map(|f| f.0.poll_read(buf))
      });
    //      .or_else(|| {
    //        repr
    //          .downcast_mut::<ResourceHttpBody>()
    //          .map(|f| f.0.poll_read(buf))
    //      });
    //      .or_else(|| {
    //        repr
    //          .downcast_mut::<ResourceChildStdout>()
    //          .map(|f| f.0.poll_read(buf))
    //      })
    //      .or_else(|| {
    //        repr
    //          .downcast_mut::<ResourceChildStderr>()
    //          .map(|f| f.0.poll_read(buf))
    //      });

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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let r = None
      .or_else(|| {
        repr
          .downcast_mut::<ResourceFsFile>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceStdout>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceStderr>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceTcpStream>()
          .map(|f| f.0.poll_write(buf))
      })
      .or_else(|| {
        repr
          .downcast_mut::<ResourceTlsStream>()
          .map(|f| f.0.poll_write(buf))
      });
    //      .or_else(|| {
    //        repr
    //          .downcast_mut::<ResourceChildStdin>()
    //          .map(|f| f.0.poll_write(buf))
    //      });

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

pub fn add_resource(resource: Box<dyn DenoResource>) -> Resource {
  let rid = new_rid();
  let mut tg = RESOURCE_TABLE.lock().unwrap();
  let r = tg.insert(rid, resource);
  assert!(r.is_none());
  Resource { rid }
}

pub fn add_fs_file(fs_file: tokio::fs::File) -> Resource {
  add_resource(Box::new(ResourceFsFile(fs_file)))
}

pub fn add_tcp_listener(listener: tokio::net::TcpListener) -> Resource {
  add_resource(Box::new(ResourceTcpListener(listener, None)))
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> Resource {
  add_resource(Box::new(ResourceTcpStream(stream)))
}

pub fn add_tls_stream(stream: TlsStream<TcpStream>) -> Resource {
  add_resource(Box::new(ResourceTlsStream(stream)))
}

pub fn add_worker(wc: WorkerChannels) -> Resource {
  add_resource(Box::new(ResourceWorker(wc)))
}

/// Post message to worker as a host or privilged overlord
pub fn post_message_to_worker(
  rid: ResourceId,
  buf: Buf,
) -> futures::sink::Send<mpsc::Sender<Buf>> {
  let mut table = RESOURCE_TABLE.lock().unwrap();
  let repr = match table.get_mut(&rid) {
    Some(repr) => repr,
    // TODO: replace this panic with `bad_resource`
    _ => panic!("bad resource"),
  };
  let worker = &mut match repr.downcast_mut::<ResourceWorker>() {
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let worker = &mut repr
      .downcast_mut::<ResourceWorker>()
      .ok_or_else(bad_resource)?;
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
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let repr = table.get_mut(&self.rid).ok_or_else(bad_resource)?;
    let worker = &mut repr
      .downcast_mut::<ResourceWorker>()
      .ok_or_else(bad_resource)?;
    let wc = &mut worker.0;
    wc.1.poll().map_err(ErrBox::from)
  }
}

pub fn get_message_stream_from_worker(rid: ResourceId) -> WorkerReceiverStream {
  WorkerReceiverStream { rid }
}

// TODO: revamp this after the following lands:
// https://github.com/tokio-rs/tokio/pull/785
pub fn get_file(rid: ResourceId) -> Result<std::fs::File, ErrBox> {
  let mut table = RESOURCE_TABLE.lock().unwrap();
  // We take ownership of File here.
  // It is put back below while still holding the lock.
  let repr = table.remove(&rid).ok_or_else(bad_resource)?;
  let fs_file = repr
    .downcast::<ResourceFsFile>()
    .or_else(|_| Err(bad_resource()))?;
  // Trait Clone not implemented on tokio::fs::File,
  // so convert to std File first.
  let std_file = fs_file.0.into_std();
  // Create a copy and immediately put back.
  // We don't want to block other resource ops.
  // try_clone() would yield a copy containing the same
  // underlying fd, so operations on the copy would also
  // affect the one in resource table, and we don't need
  // to write back.
  let maybe_std_file_copy = std_file.try_clone();
  // Insert the entry back with the same rid.
  table.insert(
    rid,
    Box::new(ResourceFsFile(tokio_fs::File::from_std(std_file))),
  );

  maybe_std_file_copy.map_err(ErrBox::from)
}

pub fn lookup(rid: ResourceId) -> Result<Resource, ErrBox> {
  debug!("resource lookup {}", rid);
  let table = RESOURCE_TABLE.lock().unwrap();
  table
    .get(&rid)
    .ok_or_else(bad_resource)
    .map(|_| Resource { rid })
}
