// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::RcLike;
use futures::future::FusedFuture;
use futures::future::Future;
use futures::future::TryFuture;
use futures::task::Context;
use futures::task::Poll;
use pin_project::pin_project;
use std::error::Error;
use std::fmt;
use std::io;
use std::pin::Pin;

use self::internal as i;

#[derive(Debug, Default)]
pub struct CancelHandle {
  node: i::Node,
}

impl CancelHandle {
  pub fn new() -> Self {
    Default::default()
  }

  // Cancel all cancellable futures that are linked to this cancel handle.
  // This method does not require a mutable reference to the `CancelHandle`.
  pub fn cancel(&self) {
    self.node.cancel();
  }

  pub fn is_canceled(&self) -> bool {
    self.node.is_canceled()
  }
}

#[pin_project(project = CancelableProjection)]
#[derive(Debug)]
pub struct Cancelable<F> {
  #[pin]
  future: Option<F>,
  #[pin]
  registration: i::Registration,
}

impl<F: Future> Future for Cancelable<F> {
  type Output = Result<F::Output, Canceled>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    self.poll_fused(cx)
  }
}

impl<F: Future> FusedFuture for Cancelable<F> {
  fn is_terminated(&self) -> bool {
    self.future.is_none()
  }
}

#[pin_project(project = TryCancelableProjection)]
#[derive(Debug)]
pub struct TryCancelable<F> {
  #[pin]
  inner: Cancelable<F>,
}

impl<F, T, E> Future for TryCancelable<F>
where
  F: Future<Output = Result<T, E>>,
  Canceled: Into<E>,
{
  type Output = F::Output;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let TryCancelableProjection { inner } = self.project();
    match inner.poll(cx) {
      Poll::Pending => Poll::Pending,
      Poll::Ready(Ok(result)) => Poll::Ready(result),
      Poll::Ready(Err(err)) => Poll::Ready(Err(err.into())),
    }
  }
}

impl<F, T, E> FusedFuture for TryCancelable<F>
where
  F: Future<Output = Result<T, E>>,
  Canceled: Into<E>,
{
  fn is_terminated(&self) -> bool {
    self.inner.is_terminated()
  }
}

pub trait CancelFuture: Future + Sized {
  fn or_cancel<H: RcLike<CancelHandle>>(
    self,
    cancel_handle: &H,
  ) -> Cancelable<Self> {
    Cancelable::new(self, cancel_handle.clone().into())
  }
}
impl<F: Future + Sized> CancelFuture for F {}

pub trait CancelTryFuture: TryFuture + CancelFuture
where
  Canceled: Into<Self::Error>,
{
  fn try_or_cancel<H: RcLike<CancelHandle>>(
    self,
    cancel_handle: &H,
  ) -> TryCancelable<Self> {
    TryCancelable::new(self, cancel_handle.clone().into())
  }
}
impl<F> CancelTryFuture for F
where
  F: TryFuture + CancelFuture,
  Canceled: Into<Self::Error>,
{
}

#[derive(Copy, Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct Canceled;

impl fmt::Display for Canceled {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "canceled")
  }
}

impl Error for Canceled {}

impl From<Canceled> for io::Error {
  fn from(_: Canceled) -> Self {
    io::Error::new(io::ErrorKind::Interrupted, Canceled)
  }
}

mod internal {
  use super::CancelHandle;
  use super::Cancelable;
  use super::Canceled;
  use super::TryCancelable;
  use crate::RcRef;
  use futures::future::Future;
  use futures::task::Context;
  use futures::task::Poll;
  use futures::task::Waker;
  use pin_project::pin_project;
  use std::cell::UnsafeCell;
  use std::marker::PhantomPinned;
  use std::mem::replace;
  use std::mem::take;
  use std::pin::Pin;
  use std::ptr::NonNull;

  impl<F: Future> Cancelable<F> {
    pub(super) fn new(future: F, cancel_handle: RcRef<CancelHandle>) -> Self {
      Self {
        future: Some(future),
        registration: Registration::new(cancel_handle),
      }
    }

    fn poll_unfused(
      self: Pin<&mut Self>,
      cx: &mut Context,
    ) -> Poll<Result<F::Output, Canceled>> {
      let self_projection = self.project();
      let future = self_projection
        .future
        .as_pin_mut()
        .expect("polled Cancelable after completion");
      let mut registration = self_projection.registration;

      // If this future is being polled for the first time, perform an extra
      // cancellation check _before_ polling the inner future. The reason to do
      // this is that polling the inner future for the first time might start
      // some activity that cannot actually be canceled (e.g. running a compute
      // job in a thread pool), so we should try to never start it at all.
      match &*registration {
        Registration::WillRegister { cancel_handle }
          if cancel_handle.is_canceled() =>
        {
          return Poll::Ready(Err(Canceled));
        }
        _ => {}
      }

      match future.poll(cx) {
        Poll::Ready(res) => return Poll::Ready(Ok(res)),
        Poll::Pending => {}
      }

      let cancel_handle = match &*registration {
        Registration::WillRegister { .. } => {
          match registration.as_mut().project_replace(Default::default()) {
            RegistrationProjectionOwned::WillRegister { cancel_handle } => {
              Some(cancel_handle)
            }
            _ => unreachable!(),
          }
        }
        _ => None,
      };

      let node = match registration.project() {
        RegistrationProjection::Registered { node } => node,
        _ => unreachable!(),
      };

      let waker = cx.waker();
      match cancel_handle {
        Some(cancel_handle) => node.link(&cancel_handle.node, waker)?,
        None => node.update(waker)?,
      }

      Poll::Pending
    }

    pub(super) fn poll_fused(
      mut self: Pin<&mut Self>,
      cx: &mut Context,
    ) -> Poll<Result<F::Output, Canceled>> {
      let poll_result = self.as_mut().poll_unfused(cx);
      // Fuse: if this Future is completed or canceled, drop the inner future
      // and drop any references/links to the cancel handle.
      if matches!(poll_result, Poll::Ready(_)) {
        self.set(Self {
          future: None,
          registration: Default::default(),
        })
      }
      poll_result
    }
  }

  impl<F: Future> TryCancelable<F> {
    pub(super) fn new(future: F, cancel_handle: RcRef<CancelHandle>) -> Self {
      Self {
        inner: Cancelable::new(future, cancel_handle),
      }
    }
  }

  #[pin_project(project = RegistrationProjection,
                project_replace = RegistrationProjectionOwned)]
  #[derive(Debug)]
  pub enum Registration {
    WillRegister {
      cancel_handle: RcRef<CancelHandle>,
    },
    Registered {
      #[pin]
      node: Node,
    },
  }

  impl Default for Registration {
    fn default() -> Self {
      Self::Registered {
        node: Default::default(),
      }
    }
  }

  impl Registration {
    fn new(cancel_handle: RcRef<CancelHandle>) -> Self {
      Self::WillRegister { cancel_handle }
    }
  }

  #[derive(Debug)]
  pub struct Node {
    inner: UnsafeCell<NodeInner>,
    _pin: PhantomPinned,
  }

  impl Default for Node {
    fn default() -> Self {
      Self {
        inner: Default::default(),
        _pin: PhantomPinned,
      }
    }
  }

  impl Drop for Node {
    fn drop(&mut self) {
      let _ = self.unlink();
    }
  }

  #[derive(Debug)]
  enum NodeInner {
    Unlinked,
    Linked {
      prev: NonNull<NodeInner>,
      next: NonNull<NodeInner>,
      waker: Option<Waker>,
    },
    Canceled,
  }

  impl Default for NodeInner {
    fn default() -> Self {
      Self::Unlinked
    }
  }

  impl Node {
    fn inner_pin_mut(self: Pin<&mut Self>) -> Pin<&mut NodeInner> {
      unsafe { self.map_unchecked_mut(|self_mut| &mut *self_mut.inner.get()) }
    }

    fn inner_non_null(&self) -> NonNull<NodeInner> {
      unsafe { NonNull::new_unchecked(self.inner.get()) }
    }

    fn link(
      self: Pin<&mut Self>,
      head: &Node,
      waker: &Waker,
    ) -> Result<(), Canceled> {
      let mut self_nn = self.inner_non_null();
      let mut head_nn = head.inner_non_null();

      // `self` and `head` must be different nodes, otherwise we'd violate
      // borrowing rules below.
      assert_ne!(&*self, head);
      let self_inner_mut = unsafe { self_nn.as_mut() };
      let head_inner_mut = unsafe { head_nn.as_mut() };

      // The linked node must be unlinked prior to calling `link()`.
      assert!(matches!(self_inner_mut, NodeInner::Unlinked));

      let waker = waker.clone();

      match head_inner_mut {
        NodeInner::Unlinked => {
          *head_inner_mut = NodeInner::Linked {
            prev: self.inner_non_null(),
            next: self.inner_non_null(),
            waker: None,
          };
          *self_inner_mut = NodeInner::Linked {
            prev: head.inner_non_null(),
            next: head.inner_non_null(),
            waker: Some(waker),
          };
          Ok(())
        }
        NodeInner::Linked {
          prev: next_prev_nn_mut,
          waker: None,
          ..
        } => {
          let prev_inner_mut = unsafe { &mut *next_prev_nn_mut.as_ptr() };
          match prev_inner_mut {
            NodeInner::Linked {
              next: prev_next_nn_mut,
              waker: Some(_),
              ..
            } => {
              *self_inner_mut = NodeInner::Linked {
                prev: replace(next_prev_nn_mut, self.inner_non_null()),
                next: replace(prev_next_nn_mut, self.inner_non_null()),
                waker: Some(waker),
              };
              Ok(())
            }
            _ => unreachable!(),
          }
        }
        NodeInner::Canceled => Err(Canceled),
        _ => unreachable!(),
      }
    }

    fn update(self: Pin<&mut Self>, new_waker: &Waker) -> Result<(), Canceled> {
      match &mut *self.inner_pin_mut() {
        NodeInner::Unlinked => Ok(()),
        NodeInner::Linked {
          waker: Some(waker), ..
        } => {
          if !waker.will_wake(new_waker) {
            *waker = new_waker.clone();
          }
          Ok(())
        }
        NodeInner::Canceled => Err(Canceled),
        _ => unreachable!(),
      }
    }

    fn unlink(&mut self) {
      let inner = take(&mut self.inner).into_inner();
      if let NodeInner::Linked {
        prev: mut prev_nn,
        next: mut next_nn,
        ..
      } = inner
      {
        if prev_nn == next_nn {
          take(unsafe { prev_nn.as_mut() });
        } else {
          match unsafe { prev_nn.as_mut() } {
            NodeInner::Linked {
              next: prev_next_nn_mut,
              ..
            } => {
              *prev_next_nn_mut = next_nn;
            }
            _ => unreachable!(),
          }
          match unsafe { next_nn.as_mut() } {
            NodeInner::Linked {
              prev: next_prev_nn_mut,
              ..
            } => {
              *next_prev_nn_mut = prev_nn;
            }
            _ => unreachable!(),
          }
        }
      }
    }

    /// Mark this node and all linked nodes for cancellation. Note that `self`
    /// must be a head node (associated with a CancelHandle).
    pub(super) fn cancel(&self) {
      let mut head_nn = self.inner_non_null();
      let mut cur_nn =
        match replace(unsafe { head_nn.as_mut() }, NodeInner::Canceled) {
          NodeInner::Unlinked | NodeInner::Canceled => return,
          NodeInner::Linked {
            next: next_nn,
            waker: None,
            ..
          } => next_nn,
          _ => unreachable!(),
        };
      while cur_nn != head_nn {
        cur_nn = match replace(unsafe { cur_nn.as_mut() }, NodeInner::Canceled)
        {
          NodeInner::Linked {
            next: next_nn,
            waker: Some(waker),
            ..
          } => {
            waker.wake();
            next_nn
          }
          _ => unreachable!(),
        }
      }
    }

    /// Returns true if this node has been marked for cancellation. Note that
    /// `self` must be a head node (associated with a CancelHandle).
    pub(super) fn is_canceled(&self) -> bool {
      let head_nn = self.inner_non_null();
      match unsafe { head_nn.as_ref() } {
        NodeInner::Unlinked | NodeInner::Linked { waker: None, .. } => false,
        NodeInner::Canceled => true,
        _ => unreachable!(),
      }
    }
  }

  impl Eq for Node {}

  impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
      self as *const _ == other as *const _
    }
  }
}
