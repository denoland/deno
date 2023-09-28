// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::request_properties::HttpConnectionProperties;
use crate::response_body::CompletionHandle;
use crate::response_body::ResponseBytes;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ResourceId;
use http::request::Parts;
use http::HeaderMap;
use hyper1::body::Incoming;
use hyper1::upgrade::OnUpgrade;

use scopeguard::defer;
use slab::Slab;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ptr::NonNull;
use std::rc::Rc;

pub type Request = hyper1::Request<Incoming>;
pub type Response = hyper1::Response<ResponseBytes>;
pub type SlabId = u32;

#[repr(transparent)]
#[derive(Clone, Default)]
pub struct RefCount(pub Rc<()>);

enum RequestBodyState {
  Incoming(Incoming),
  Resource(HttpRequestBodyAutocloser),
}

impl From<Incoming> for RequestBodyState {
  fn from(value: Incoming) -> Self {
    RequestBodyState::Incoming(value)
  }
}

/// Ensures that the request body closes itself when no longer needed.
pub struct HttpRequestBodyAutocloser(ResourceId, Rc<RefCell<OpState>>);

impl HttpRequestBodyAutocloser {
  pub fn new(res: ResourceId, op_state: Rc<RefCell<OpState>>) -> Self {
    Self(res, op_state)
  }
}

impl Drop for HttpRequestBodyAutocloser {
  fn drop(&mut self) {
    if let Ok(res) = self.1.borrow_mut().resource_table.take_any(self.0) {
      res.close();
    }
  }
}

pub async fn new_slab_future(
  request: Request,
  request_info: HttpConnectionProperties,
  refcount: RefCount,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> Result<Response, hyper::Error> {
  let index = slab_insert(request, request_info, refcount);
  defer! {
    slab_drop(index);
  }
  let rx = slab_get(index).promise();
  // Safe to unwrap as channel receiver is never closed.
  tx.send(index).await.unwrap();
  http_trace!(index, "SlabFuture await");
  rx.await;
  http_trace!(index, "SlabFuture complete");
  let response = slab_get(index).take_response();
  Ok(response)
}

pub struct HttpSlabRecord {
  request_info: HttpConnectionProperties,
  request_parts: Parts,
  request_body: Option<RequestBodyState>,
  /// The response may get taken before we tear this down
  response: Option<Response>,
  promise: CompletionHandle,
  trailers: Rc<RefCell<Option<HeaderMap>>>,
  been_dropped: bool,
  /// Use a `Rc` to keep track of outstanding requests. We don't use this, but
  /// when it drops, it decrements the refcount of the server itself.
  refcount: Option<RefCount>,
  #[cfg(feature = "__zombie_http_tracking")]
  alive: bool,
}

thread_local! {
  pub(crate) static SLAB: RefCell<Slab<HttpSlabRecord>> = const { RefCell::new(Slab::new()) };
}

macro_rules! http_trace {
  ($index:expr, $args:tt) => {
    #[cfg(feature = "__http_tracing")]
    {
      let total = $crate::slab::SLAB.with(|x| x.try_borrow().map(|x| x.len()));
      if let Ok(total) = total {
        println!("HTTP id={} total={}: {}", $index, total, format!($args));
      } else {
        println!("HTTP id={} total=?: {}", $index, format!($args));
      }
    }
  };
}

pub(crate) use http_trace;

/// Hold a lock on the slab table and a reference to one entry in the table.
pub struct SlabEntry(
  NonNull<HttpSlabRecord>,
  SlabId,
  RefMut<'static, Slab<HttpSlabRecord>>,
);

const SLAB_CAPACITY: usize = 1024;

pub fn slab_init() {
  SLAB.with(|slab: &RefCell<Slab<HttpSlabRecord>>| {
    // Note that there might already be an active HTTP server, so this may just
    // end up adding room for an additional SLAB_CAPACITY items. All HTTP servers
    // on a single thread share the same slab.
    let mut slab = slab.borrow_mut();
    slab.reserve(SLAB_CAPACITY);
  })
}

pub fn slab_get(index: SlabId) -> SlabEntry {
  http_trace!(index, "slab_get");
  let mut lock: RefMut<'static, Slab<HttpSlabRecord>> = SLAB.with(|x| {
    // SAFETY: We're extracting a lock here and placing it into an object that is thread-local, !Send as a &'static
    unsafe { std::mem::transmute(x.borrow_mut()) }
  });
  let Some(entry) = lock.get_mut(index as usize) else {
    panic!("HTTP state error: Attempted to access invalid request {} ({} in total available)",
    index,
    lock.len())
  };
  #[cfg(feature = "__zombie_http_tracking")]
  {
    assert!(entry.alive, "HTTP state error: Entry is not alive");
  }
  let entry = NonNull::new(entry as _).unwrap();

  SlabEntry(entry, index, lock)
}

#[allow(clippy::let_and_return)]
fn slab_insert_raw(
  request_parts: Parts,
  request_body: Option<Incoming>,
  request_info: HttpConnectionProperties,
  refcount: RefCount,
) -> SlabId {
  let index = SLAB.with(|slab| {
    let mut slab = slab.borrow_mut();
    let body = ResponseBytes::default();
    let trailers = body.trailers();
    let request_body = request_body.map(|r| r.into());
    slab.insert(HttpSlabRecord {
      request_info,
      request_parts,
      request_body,
      response: Some(Response::new(body)),
      trailers,
      been_dropped: false,
      promise: CompletionHandle::default(),
      refcount: Some(refcount),
      #[cfg(feature = "__zombie_http_tracking")]
      alive: true,
    })
  }) as u32;
  http_trace!(index, "slab_insert");
  index
}

pub fn slab_insert(
  request: Request,
  request_info: HttpConnectionProperties,
  refcount: RefCount,
) -> SlabId {
  let (request_parts, request_body) = request.into_parts();
  slab_insert_raw(request_parts, Some(request_body), request_info, refcount)
}

pub fn slab_drop(index: SlabId) {
  http_trace!(index, "slab_drop");
  let mut entry = slab_get(index);
  let record = entry.self_mut();
  assert!(
    !record.been_dropped,
    "HTTP state error: Entry has already been dropped"
  );

  // The logic here is somewhat complicated. A slab record cannot be expunged until it has been dropped by Rust AND
  // the promise has been completed (indicating that JavaScript is done processing). However, if Rust has finished
  // dealing with this entry, we DO want to clean up some of the associated items -- namely the request body, which
  // might include actual resources, and the refcount, which is keeping the server alive.
  record.been_dropped = true;
  if record.promise.is_completed() {
    drop(entry);
    slab_expunge(index);
  } else {
    // Take the request body, as the future has been dropped and this will allow some resources to close
    record.request_body.take();
    // Take the refcount keeping the server alive. The future is no longer alive, which means this request
    // is toast.
    record.refcount.take();
  }
}

fn slab_expunge(index: SlabId) {
  SLAB.with(|slab| {
    #[cfg(__zombie_http_tracking)]
    {
      slab.borrow_mut().get_mut(index as usize).unwrap().alive = false;
    }
    #[cfg(not(__zombie_http_tracking))]
    {
      slab.borrow_mut().remove(index as usize);
    }
  });
  http_trace!(index, "slab_expunge");
}

impl SlabEntry {
  fn self_ref(&self) -> &HttpSlabRecord {
    // SAFETY: We have the lock and we're borrowing lifetime from self
    unsafe { self.0.as_ref() }
  }

  fn self_mut(&mut self) -> &mut HttpSlabRecord {
    // SAFETY: We have the lock and we're borrowing lifetime from self
    unsafe { self.0.as_mut() }
  }

  /// Perform the Hyper upgrade on this entry.
  pub fn upgrade(&mut self) -> Result<OnUpgrade, AnyError> {
    // Manually perform the upgrade. We're peeking into hyper's underlying machinery here a bit
    self
      .self_mut()
      .request_parts
      .extensions
      .remove::<OnUpgrade>()
      .ok_or_else(|| AnyError::msg("upgrade unavailable"))
  }

  /// Take the Hyper body from this entry.
  pub fn take_body(&mut self) -> Option<Incoming> {
    let body_holder = &mut self.self_mut().request_body;
    let body = body_holder.take();
    match body {
      Some(RequestBodyState::Incoming(body)) => Some(body),
      x => {
        *body_holder = x;
        None
      }
    }
  }

  pub fn take_resource(&mut self) -> Option<HttpRequestBodyAutocloser> {
    let body_holder = &mut self.self_mut().request_body;
    let body = body_holder.take();
    match body {
      Some(RequestBodyState::Resource(res)) => Some(res),
      x => {
        *body_holder = x;
        None
      }
    }
  }

  /// Replace the request body with a resource ID and the OpState we'll need to shut it down.
  /// We cannot keep just the resource itself, as JS code might be reading from the resource ID
  /// to generate the response data (requiring us to keep it in the resource table).
  pub fn put_resource(&mut self, res: HttpRequestBodyAutocloser) {
    self.self_mut().request_body = Some(RequestBodyState::Resource(res));
  }

  /// Complete this entry, potentially expunging it if it is fully complete (ie: dropped as well).
  pub fn complete(self) {
    let promise = &self.self_ref().promise;
    assert!(
      !promise.is_completed(),
      "HTTP state error: Entry has already been completed"
    );
    http_trace!(self.1, "SlabEntry::complete");
    promise.complete(true);
    // If we're all done, we need to drop ourself to release the lock before we expunge this record
    if self.self_ref().been_dropped {
      let index = self.1;
      drop(self);
      slab_expunge(index);
    }
  }

  /// Has the future for this entry been dropped? ie, has the underlying TCP connection
  /// been closed?
  pub fn cancelled(&self) -> bool {
    self.self_ref().been_dropped
  }

  /// Get a mutable reference to the response.
  pub fn response(&mut self) -> &mut Response {
    self.self_mut().response.as_mut().unwrap()
  }

  /// Get a mutable reference to the trailers.
  pub fn trailers(&mut self) -> &RefCell<Option<HeaderMap>> {
    &self.self_mut().trailers
  }

  /// Take the response.
  pub fn take_response(&mut self) -> Response {
    self.self_mut().response.take().unwrap()
  }

  /// Get a reference to the connection properties.
  pub fn request_info(&self) -> &HttpConnectionProperties {
    &self.self_ref().request_info
  }

  /// Get a reference to the request parts.
  pub fn request_parts(&self) -> &Parts {
    &self.self_ref().request_parts
  }

  /// Get a reference to the completion handle.
  pub fn promise(&self) -> CompletionHandle {
    self.self_ref().promise.clone()
  }

  /// Get a reference to the response body completion handle.
  pub fn body_promise(&self) -> CompletionHandle {
    self
      .self_ref()
      .response
      .as_ref()
      .unwrap()
      .body()
      .completion_handle()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_net::raw::NetworkStreamType;
  use http::Request;

  #[test]
  fn test_slab() {
    let req = Request::builder().body(()).unwrap();
    let (parts, _) = req.into_parts();
    let id = slab_insert_raw(
      parts,
      None,
      HttpConnectionProperties {
        peer_address: "".into(),
        peer_port: None,
        local_port: None,
        stream_type: NetworkStreamType::Tcp,
      },
      RefCount::default(),
    );
    let entry = slab_get(id);
    entry.complete();
    slab_drop(id);
  }
}
