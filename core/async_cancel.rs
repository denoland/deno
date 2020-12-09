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
use std::fmt::Display;
use std::fmt::Formatter;
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

  /// Cancel all cancelable futures that are bound to this handle. Note that
  /// this method does not require a mutable reference to the `CancelHandle`.
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
    // `future` and `registration` fields are dropped in order to unlink it from
    // its cancel handle.
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

impl Display for Canceled {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
  use std::any::Any;
  use std::cell::UnsafeCell;
  use std::marker::PhantomPinned;
  use std::mem::replace;
  use std::pin::Pin;
  use std::ptr::NonNull;
  use std::rc::Rc;
  use std::rc::Weak;

  impl<F: Future> Cancelable<F> {
    pub(super) fn new(future: F, cancel_handle: RcRef<CancelHandle>) -> Self {
      let head_node = RcRef::map(cancel_handle, |r| &r.node);
      let registration = Registration::WillRegister { head_node };
      Self::Pending {
        future,
        registration,
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
        Registration::WillRegister { head_node } if head_node.is_canceled() => {
          return Poll::Ready(Err(Canceled));
        }
        _ => {}
      }

      match future.poll(cx) {
        Poll::Ready(res) => return Poll::Ready(Ok(res)),
        Poll::Pending => {}
      }

      // Register this future with its `CancelHandle`, saving the `Waker` that
      // can be used to make the runtime poll this future when it is canceled.
      // When already registered, update the stored `Waker` if necessary.
      let head_node = match &*registration {
        Registration::WillRegister { .. } => {
          match registration.as_mut().project_replace(Default::default()) {
            RegistrationProjectionOwned::WillRegister { head_node } => {
              Some(head_node)
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
      node.register(cx.waker(), head_node)?;

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
      head_node: RcRef<Node>,
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

  #[derive(Debug)]
  pub struct Node {
    inner: UnsafeCell<NodeInner>,
    _pin: PhantomPinned,
  }

  impl Node {
    /// If necessary, register a `Cancelable` node with a `CancelHandle`, and
    /// save or update the `Waker` that can wake with this cancelable future.
    pub fn register(
      &self,
      waker: &Waker,
      head_rc: Option<RcRef<Node>>,
    ) -> Result<(), Canceled> {
      match head_rc.as_ref().map(RcRef::split) {
        Some((head, rc)) => {
          // Register this `Cancelable` node with a `CancelHandle` head node.
          assert_ne!(self, head);
          let self_inner = unsafe { &mut *self.inner.get() };
          let head_inner = unsafe { &mut *head.inner.get() };
          self_inner.link(waker, head_inner, rc)
        }
        None => {
          // This `Cancelable` has already been linked to a `CancelHandle` head
          // node; just update our stored `Waker` if necessary.
          let inner = unsafe { &mut *self.inner.get() };
          inner.update_waker(waker)
        }
      }
    }

    pub fn cancel(&self) {
      let inner = unsafe { &mut *self.inner.get() };
      inner.cancel();
    }

    pub fn is_canceled(&self) -> bool {
      let inner = unsafe { &mut *self.inner.get() };
      inner.is_canceled()
    }
  }

  impl Default for Node {
    fn default() -> Self {
      Self {
        inner: UnsafeCell::new(NodeInner::Unlinked),
        _pin: PhantomPinned,
      }
    }
  }

  impl Drop for Node {
    fn drop(&mut self) {
      let inner = unsafe { &mut *self.inner.get() };
      inner.unlink();
    }
  }

  impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
      self as *const _ == other as *const _
    }
  }

  #[derive(Debug)]
  enum NodeInner {
    Unlinked,
    Linked {
      kind: NodeKind,
      prev: NonNull<NodeInner>,
      next: NonNull<NodeInner>,
    },
    Canceled,
  }

  impl NodeInner {
    fn as_non_null(&mut self) -> NonNull<Self> {
      NonNull::from(self)
    }

    fn link(
      &mut self,
      waker: &Waker,
      head: &mut Self,
      rc_pin: &Rc<dyn Any>,
    ) -> Result<(), Canceled> {
      // The future should not have been linked to a cancel handle before.
      assert!(matches!(self, NodeInner::Unlinked));

      match head {
        NodeInner::Unlinked => {
          *head = NodeInner::Linked {
            kind: NodeKind::head(rc_pin),
            prev: self.as_non_null(),
            next: self.as_non_null(),
          };
          *self = NodeInner::Linked {
            kind: NodeKind::item(waker),
            prev: head.as_non_null(),
            next: head.as_non_null(),
          };
          Ok(())
        }
        NodeInner::Linked {
          kind: NodeKind::Head { .. },
          prev: next_prev_nn,
          ..
        } => {
          let prev = unsafe { &mut *next_prev_nn.as_ptr() };
          match prev {
            NodeInner::Linked {
              kind: NodeKind::Item { .. },
              next: prev_next_nn,
              ..
            } => {
              *self = NodeInner::Linked {
                kind: NodeKind::item(waker),
                prev: replace(next_prev_nn, self.as_non_null()),
                next: replace(prev_next_nn, self.as_non_null()),
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

    fn update_waker(&mut self, new_waker: &Waker) -> Result<(), Canceled> {
      match self {
        NodeInner::Unlinked => Ok(()),
        NodeInner::Linked {
          kind: NodeKind::Item { waker },
          ..
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

    /// If this node is linked to other nodes, remove it from the chain. This
    /// method is called (only) by the drop handler for `Node`. It is suitable
    /// for both 'head' and 'item' nodes.
    fn unlink(&mut self) {
      if let NodeInner::Linked {
        prev: mut prev_nn,
        next: mut next_nn,
        ..
      } = replace(self, NodeInner::Unlinked)
      {
        if prev_nn == next_nn {
          // There were only two nodes in this chain; after unlinking ourselves
          // the other node is no longer linked.
          let other = unsafe { prev_nn.as_mut() };
          *other = NodeInner::Unlinked;
        } else {
          // The chain had more than two nodes.
          match unsafe { prev_nn.as_mut() } {
            NodeInner::Linked {
              next: prev_next_nn, ..
            } => {
              *prev_next_nn = next_nn;
            }
            _ => unreachable!(),
          }
          match unsafe { next_nn.as_mut() } {
            NodeInner::Linked {
              prev: next_prev_nn, ..
            } => {
              *next_prev_nn = prev_nn;
            }
            _ => unreachable!(),
          }
        }
      }
    }

    /// Mark this node and all linked nodes for cancellation. Note that `self`
    /// must refer to a head (`CancelHandle`) node.
    fn cancel(&mut self) {
      let mut head_nn = NonNull::from(self);
      let mut item_nn;

      // Mark the head node as canceled.
      match replace(unsafe { head_nn.as_mut() }, NodeInner::Canceled) {
        NodeInner::Linked {
          kind: NodeKind::Head { .. },
          next: next_nn,
          ..
        } => item_nn = next_nn,
        NodeInner::Unlinked | NodeInner::Canceled => return,
        _ => unreachable!(),
      };

      // Cancel all item nodes in the chain, waking each stored `Waker`.
      while item_nn != head_nn {
        match replace(unsafe { item_nn.as_mut() }, NodeInner::Canceled) {
          NodeInner::Linked {
            kind: NodeKind::Item { waker },
            next: next_nn,
            ..
          } => {
            waker.wake();
            item_nn = next_nn;
          }
          _ => unreachable!(),
        }
      }
    }

    /// Returns true if this node has been marked for cancellation. Note that
    /// `self` must refer to a head (`CancelHandle`) node.
    fn is_canceled(&self) -> bool {
      match self {
        NodeInner::Unlinked => false,
        NodeInner::Linked {
          kind: NodeKind::Head { .. },
          ..
        } => false,
        NodeInner::Canceled => true,
        _ => unreachable!(),
      }
    }
  }

  #[derive(Debug)]
  enum NodeKind {
    /// In a chain of linked nodes, the "head" node is owned by the
    /// `CancelHandle`. A chain usually contains at most one head node; however
    /// when a `CancelHandle` is dropped before the futures associated with it
    /// are dropped, a chain may temporarily contain no head node at all.
    Head {
      /// The `weak_pin` field adds adds a weak reference to the `Rc` guarding
      /// the heap allocation that contains the `CancelHandle`. Without this
      /// extra weak reference, `Rc::get_mut()` might succeed and allow the
      /// `CancelHandle` to be moved when it isn't safe to do so.
      weak_pin: Weak<dyn Any>,
    },
    /// All item nodes in a chain are associated with a `Cancelable` head node.
    Item {
      /// If this future indeed does get canceled, the waker is needed to make
      /// sure that the canceled future gets polled as soon as possible.
      waker: Waker,
    },
  }

  impl NodeKind {
    fn head(rc_pin: &Rc<dyn Any>) -> Self {
      let weak_pin = Rc::downgrade(rc_pin);
      Self::Head { weak_pin }
    }

    fn item(waker: &Waker) -> Self {
      let waker = waker.clone();
      Self::Item { waker }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::AnyError;
  use futures::future::pending;
  use futures::future::poll_fn;
  use futures::future::ready;
  use futures::future::FutureExt;
  use futures::future::TryFutureExt;
  use futures::select;
  use futures::task::noop_waker_ref;
  use futures::task::Context;
  use futures::task::Poll;
  use std::convert::Infallible as Never;
  use std::io;
  use tokio::net::TcpStream;
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
      // Cancel a network I/O future right after polling it.
      let cancel_handle = Rc::new(CancelHandle::new());
      let result = loop {
        select! {
          r = TcpStream::connect("1.2.3.4:12345")
            .try_or_cancel(&cancel_handle) => break r,
          default => cancel_handle.cancel(),
        };
      };
      let error = result.unwrap_err();
      assert_eq!(error.kind(), io::ErrorKind::Interrupted);
      assert_eq!(error.to_string().as_str(), "operation canceled");
    }
  }

  #[test]
  fn cancel_handle_pinning() {
    let mut cancel_handle = CancelHandle::new_rc();

    // There is only one reference to `cancel_handle`, so `Rc::get_mut()` should
    // succeed.
    assert!(Rc::get_mut(&mut cancel_handle).is_some());

    let mut future = pending::<Never>().or_cancel(&cancel_handle);
    let future = unsafe { Pin::new_unchecked(&mut future) };

    // There are two `Rc<CancelHandle>` references now, so this fails.
    assert!(Rc::get_mut(&mut cancel_handle).is_none());

    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(future.poll(&mut cx).is_pending());

    // Polling `future` has established a link between the future and
    // `cancel_handle`, so both values should be pinned at this point.
    assert!(Rc::get_mut(&mut cancel_handle).is_none());

    cancel_handle.cancel();

    // Canceling or dropping the associated future(s) unlinks them from the
    // cancel handle, therefore `cancel_handle` can now safely be moved again.
    assert!(Rc::get_mut(&mut cancel_handle).is_some());
  }
}
