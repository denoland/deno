// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::request_properties::HttpConnectionProperties;
use crate::response_body::CompletionHandle;
use crate::response_body::ResponseBytes;
use deno_core::error::AnyError;
use http::request::Parts;
use hyper1::body::Incoming;
use hyper1::upgrade::OnUpgrade;

use slab::Slab;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ptr::NonNull;

pub type Request = hyper1::Request<Incoming>;
pub type Response = hyper1::Response<ResponseBytes>;
pub type SlabId = u32;

pub struct HttpSlabRecord {
  request_info: HttpConnectionProperties,
  request_parts: Parts,
  request_body: Option<Incoming>,
  // The response may get taken before we tear this down
  response: Option<Response>,
  promise: CompletionHandle,
  been_dropped: bool,
  #[cfg(feature = "__zombie_http_tracking")]
  alive: bool,
}

thread_local! {
  static SLAB: RefCell<Slab<HttpSlabRecord>> = RefCell::new(Slab::with_capacity(1024));
}

macro_rules! http_trace {
  ($index:expr, $args:tt) => {
    #[cfg(feature = "__http_tracing")]
    {
      let total = SLAB.with(|x| x.try_borrow().map(|x| x.len()));
      if let Ok(total) = total {
        println!("HTTP id={} total={}: {}", $index, total, format!($args));
      } else {
        println!("HTTP id={} total=?: {}", $index, format!($args));
      }
    }
  };
}

/// Hold a lock on the slab table and a reference to one entry in the table.
pub struct SlabEntry(
  NonNull<HttpSlabRecord>,
  SlabId,
  RefMut<'static, Slab<HttpSlabRecord>>,
);

pub fn slab_get(index: SlabId) -> SlabEntry {
  http_trace!(index, "slab_get");
  let mut lock: RefMut<'static, Slab<HttpSlabRecord>> = SLAB.with(|x| {
    // SAFETY: We're extracting a lock here and placing it into an object that is thread-local, !Send as a &'static
    unsafe { std::mem::transmute(x.borrow_mut()) }
  });
  let Some(entry) = lock.get_mut(index as usize) else {
    panic!("HTTP state error: Attemped to access invalid request {} ({} in total available)",
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

fn slab_insert_raw(
  request_parts: Parts,
  request_body: Option<Incoming>,
  request_info: HttpConnectionProperties,
) -> SlabId {
  let index = SLAB.with(|slab| {
    let mut slab = slab.borrow_mut();
    slab.insert(HttpSlabRecord {
      request_info,
      request_parts,
      request_body,
      response: Some(Response::new(ResponseBytes::default())),
      been_dropped: false,
      promise: CompletionHandle::default(),
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
) -> SlabId {
  let (request_parts, request_body) = request.into_parts();
  slab_insert_raw(request_parts, Some(request_body), request_info)
}

pub fn slab_drop(index: SlabId) {
  http_trace!(index, "slab_drop");
  let mut entry = slab_get(index);
  let record = entry.self_mut();
  assert!(
    !record.been_dropped,
    "HTTP state error: Entry has already been dropped"
  );
  record.been_dropped = true;
  if record.promise.is_completed() {
    drop(entry);
    slab_expunge(index);
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
  pub fn take_body(&mut self) -> Incoming {
    self.self_mut().request_body.take().unwrap()
  }

  /// Complete this entry, potentially expunging it if it is complete.
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

  /// Get a mutable reference to the response.
  pub fn response(&mut self) -> &mut Response {
    self.self_mut().response.as_mut().unwrap()
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
    );
    let entry = slab_get(id);
    entry.complete();
    slab_drop(id);
  }
}
