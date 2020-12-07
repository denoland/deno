// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::RcLike;
use futures::future::FusedFuture;
use futures::future::Future;
use futures::future::TryFuture;
use futures::task::Context;
use futures::task::Poll;
use pin_project::pin_project;
use std::any::type_name;
use std::error::Error;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::rc::Rc;

use self::internal as i;

#[derive(Debug, Default)]
pub struct CancelHandle {
  node: i::Node,
}

impl CancelHandle {
  pub fn new() -> Self {
    Default::default()
  }

  pub fn new_rc() -> Rc<Self> {
    Rc::new(Self::new())
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
pub enum Cancelable<F> {
  Pending {
    #[pin]
    future: F,
    #[pin]
    registration: i::Registration,
  },
  Terminated,
}

impl<F: Future> Future for Cancelable<F> {
  type Output = Result<F::Output, Canceled>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let poll_result = match self.as_mut().project() {
      CancelableProjection::Pending {
        future,
        registration,
      } => Self::poll_pending(future, registration, cx),
      CancelableProjection::Terminated => {
        panic!("{}::poll() called after completion", type_name::<Self>())
      }
    };
    // Fuse: if this Future is completed or canceled, make sure the inner
    // `future` and `registration` fields are dropped in order to unlink it
    // from its cancellation handle.
    if matches!(poll_result, Poll::Ready(_)) {
      self.set(Cancelable::Terminated)
    }
    poll_result
  }
}

impl<F: Future> FusedFuture for Cancelable<F> {
  fn is_terminated(&self) -> bool {
    matches!(self, Self::Terminated)
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

pub trait CancelFuture
where
  Self: Future + Sized,
{
  fn or_cancel<H: RcLike<CancelHandle>>(
    self,
    cancel_handle: H,
  ) -> Cancelable<Self> {
    Cancelable::new(self, cancel_handle.into())
  }
}
impl<F> CancelFuture for F where F: Future {}

pub trait CancelTryFuture
where
  Self: TryFuture + Sized,
  Canceled: Into<Self::Error>,
{
  fn try_or_cancel<H: RcLike<CancelHandle>>(
    self,
    cancel_handle: H,
  ) -> TryCancelable<Self> {
    TryCancelable::new(self, cancel_handle.into())
  }
}
impl<F> CancelTryFuture for F
where
  F: TryFuture,
  Canceled: Into<F::Error>,
{
}

#[derive(Copy, Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct Canceled;

impl fmt::Display for Canceled {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "operation canceled")
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
      Self::Pending {
        future,
        registration: Registration::new(cancel_handle),
      }
    }

    pub(super) fn poll_pending(
      future: Pin<&mut F>,
      mut registration: Pin<&mut Registration>,
      cx: &mut Context,
    ) -> Poll<Result<F::Output, Canceled>> {
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
      if let NodeInner::Linked {
        prev: mut prev_nn,
        next: mut next_nn,
        ..
      } = take(&mut self.inner).into_inner()
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::AnyError;
  use futures::future::poll_fn;
  use futures::future::ready;
  use futures::future::FutureExt;
  use futures::future::TryFutureExt;
  use futures::select;
  use futures::task::noop_waker_ref;
  use futures::task::Context;
  use futures::task::Poll;
  use std::io;
  use tokio::fs::metadata;
  use tokio::spawn;

  fn box_fused<'a, F: FusedFuture + 'a>(
    future: F,
  ) -> Pin<Box<dyn FusedFuture<Output = F::Output> + 'a>> {
    Box::pin(future)
  }

  async fn ready_in_n(name: &str, count: usize) -> &str {
    let mut remaining = count as isize;
    poll_fn(|_| {
      assert!(remaining >= 0);
      if remaining == 0 {
        Poll::Ready(name)
      } else {
        remaining -= 1;
        Poll::Pending
      }
    })
    .await
  }

  #[test]
  fn cancel_future() {
    let cancel_now = CancelHandle::new_rc();
    let cancel_at_0 = CancelHandle::new_rc();
    let cancel_at_1 = CancelHandle::new_rc();
    let cancel_at_4 = CancelHandle::new_rc();
    let cancel_never = CancelHandle::new_rc();

    cancel_now.cancel();

    let mut futures = vec![
      box_fused(ready("A").or_cancel(&cancel_now)),
      box_fused(ready("B").or_cancel(&cancel_at_0)),
      box_fused(ready("C").or_cancel(&cancel_at_1)),
      box_fused(
        ready_in_n("D", 0)
          .or_cancel(&cancel_never)
          .try_or_cancel(&cancel_now),
      ),
      box_fused(
        ready_in_n("E", 1)
          .or_cancel(&cancel_at_1)
          .try_or_cancel(&cancel_at_1),
      ),
      box_fused(ready_in_n("F", 2).or_cancel(&cancel_at_1)),
      box_fused(ready_in_n("G", 3).or_cancel(&cancel_at_4)),
      box_fused(ready_in_n("H", 4).or_cancel(&cancel_at_4)),
      box_fused(ready_in_n("I", 5).or_cancel(&cancel_at_4)),
      box_fused(ready_in_n("J", 5).map(Ok)),
      box_fused(ready_in_n("K", 5).or_cancel(cancel_never)),
    ];

    let mut cx = Context::from_waker(noop_waker_ref());

    for i in 0..=5 {
      match i {
        0 => cancel_at_0.cancel(),
        1 => cancel_at_1.cancel(),
        4 => cancel_at_4.cancel(),
        2 | 3 | 5 => {}
        _ => unreachable!(),
      }

      let results = futures
        .iter_mut()
        .filter(|fut| !fut.is_terminated())
        .filter_map(|fut| match fut.poll_unpin(&mut cx) {
          Poll::Pending => None,
          Poll::Ready(res) => Some(res),
        })
        .collect::<Vec<_>>();

      match i {
        0 => assert_eq!(
          results,
          [Err(Canceled), Err(Canceled), Ok("C"), Err(Canceled)]
        ),
        1 => assert_eq!(results, [Ok("E"), Err(Canceled)]),
        2 => assert_eq!(results, []),
        3 => assert_eq!(results, [Ok("G")]),
        4 => assert_eq!(results, [Ok("H"), Err(Canceled)]),
        5 => assert_eq!(results, [Ok("J"), Ok("K")]),
        _ => unreachable!(),
      }
    }

    assert_eq!(futures.into_iter().any(|fut| !fut.is_terminated()), false);

    let cancel_handles = [cancel_now, cancel_at_0, cancel_at_1, cancel_at_4];
    assert_eq!(cancel_handles.iter().any(|c| !c.is_canceled()), false);
  }

  #[tokio::test]
  async fn cancel_try_future() {
    {
      // Cancel a spawned task before it actually runs.
      let cancel_handle = Rc::new(CancelHandle::new());
      let future = spawn(async { panic!("the task should not be spawned") })
        .map_err(AnyError::from)
        .try_or_cancel(&cancel_handle);
      cancel_handle.cancel();
      let error = future.await.unwrap_err();
      assert!(error.downcast_ref::<Canceled>().is_some());
      assert_eq!(error.to_string().as_str(), "operation canceled");
    }

    {
      // Cancel a file system future right after polling it.
      let cancel_handle = Rc::new(CancelHandle::new());
      let fake_path = "/🚫/...🤯.../🧻";
      let result = loop {
        select! {
          r = metadata(fake_path).try_or_cancel(&cancel_handle) => break r,
          default => cancel_handle.cancel(),
        };
      };
      let error = result.unwrap_err();
      assert_eq!(error.kind(), io::ErrorKind::Interrupted);
      assert_eq!(error.to_string().as_str(), "operation canceled");
    }
  }
}
