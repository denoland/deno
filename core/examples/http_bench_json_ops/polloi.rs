// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

#![allow(dead_code)]

use std::cell::UnsafeCell;
use std::future::poll_fn;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::Poll::Pending;
use std::task::Poll::Ready;
use std::task::RawWaker;
use std::task::RawWakerVTable;
use std::task::Waker;
use std::time::Duration;
use std::time::Instant;

pub struct Runtime {
  state: UnsafeCell<RuntimeState>,
}

struct RuntimeState {
  free: Vec<usize>,
  used: Vec<Io>, // Never shrinks but sooner or later turns semi-stable.
  poll: mio::Poll,
  events: mio::Events,
  tasks: Vec<Pin<Box<dyn Future<Output = ()>>>>,
  refs: usize,
  timers: Vec<Timer>, // unordered list; great when few timers, bad when many
  timerids: u64,
}

#[derive(Default)]
struct Io {
  readable: Option<bool>, // Tri-state: yes, no, unknown
  writable: Option<bool>, // Tri-state: yes, no, unknown
  error: bool,
  read_closed: bool,
  write_closed: bool,
  read_waker: Option<Waker>,
  write_waker: Option<Waker>,
}

struct Timer {
  instant: Instant,
  waker: Waker,
  id: u64,
}

impl RuntimeState {
  fn new() -> io::Result<Self> {
    Ok(Self {
      free: Vec::new(),
      used: Vec::new(),
      poll: mio::Poll::new()?,
      events: mio::Events::with_capacity(256),
      tasks: vec![],
      refs: 0,
      timers: Vec::new(),
      timerids: 0,
    })
  }

  fn deregister(&mut self, slot: usize, source: &mut impl mio::event::Source) {
    assert_ne!(slot, usize::MAX);

    self
      .poll
      .registry()
      .deregister(source)
      .expect("polloi: i/o error");

    self.refs = self
      .refs
      .checked_sub(1)
      .expect("polloi: reference count underflow");

    self.used[slot] = Io::default();
    self.free.push(slot);
  }
}

pub struct TcpListener {
  runtime: Rc<Runtime>,
  inner: UnsafeCell<mio::net::TcpListener>,
  slot: UnsafeCell<usize>,
}

pub struct TcpStream {
  runtime: Rc<Runtime>,
  inner: UnsafeCell<mio::net::TcpStream>,
  slot: UnsafeCell<usize>,
}

impl Runtime {
  pub fn new() -> io::Result<Rc<Self>> {
    let state = RuntimeState::new()?;
    let state = UnsafeCell::new(state);
    Ok(Rc::new(Self { state }))
  }

  pub fn block_on<Fut: Future>(self: &Rc<Self>, mut fut: Fut) -> Fut::Output {
    // We can use a dummy here because the future doesn't need waking up,
    // we simply poll it whenever the event loop makes progress.
    let waker = dummy_waker();
    let cx = &mut Context::from_waker(&waker);

    // Start out with twice the capacity of the |events| array because each
    // I/O object pushes up to two wakers onto the queue. The exact size does
    // not matter too much, it's just a minor optimization. It scales up
    // quickly even when we get the initial size wrong.
    let mut wakers = Vec::with_capacity(512);

    loop {
      // SAFETY: pinned on stack, never moved.
      let fut = unsafe { Pin::new_unchecked(&mut fut) };

      if let Poll::Ready(result) = fut.poll(cx) {
        return result;
      }

      let state = unsafe_borrow_mut(&self.state);

      let tasks = &mut state.tasks;
      let mut i = 0;
      while i < tasks.len() {
        if let Poll::Ready(()) = tasks[i].as_mut().poll(cx) {
          tasks.swap_remove(i);
        } else {
          i += 1;
        }
      }

      loop {
        let timers = &mut state.timers;
        let mut timeout = None;

        if !timers.is_empty() {
          let now = Instant::now();
          let mut i = 0;

          while i < timers.len() {
            if let Some(dur) = timers[i].instant.checked_duration_since(now) {
              if let Some(current) = timeout {
                if current > dur {
                  timeout = Some(dur);
                }
              } else {
                timeout = Some(dur);
              }
              i += 1;
            } else {
              let timer = timers.swap_remove(i); // expired timer
              wakers.push(timer.waker);
              timeout = Some(Duration::ZERO);
            }
          }
        }

        if timeout.is_none() && state.refs == 0 {
          timeout = Some(Duration::ZERO);
        }

        // mio truncates the duration to milliseconds
        // so round up to avoid poll() returning early
        if let Some(timeout) = timeout.as_mut() {
          *timeout = timeout
            .checked_add(Duration::from_micros(500))
            .unwrap_or(*timeout);
        }

        match state.poll.poll(&mut state.events, timeout) {
          Err(ref e) if is_interrupted(e) => continue,
          Err(e) => panic!("polloi: i/o error: {}", e),
          Ok(()) => break,
        }
      }

      for event in &state.events {
        let io = &mut state.used[event.token().0];

        if event.is_readable() {
          if let Some(waker) = io.read_waker.take() {
            wakers.push(waker);
          }
          io.readable = Some(true);
        }

        if event.is_writable() {
          if let Some(waker) = io.write_waker.take() {
            wakers.push(waker);
          }
          io.writable = Some(true);
        }

        io.error |= event.is_error();
        io.read_closed |= event.is_read_closed();
        io.write_closed |= event.is_write_closed();
      }

      for waker in &wakers {
        waker.wake_by_ref();
      }

      wakers.clear();
    }
  }

  pub fn delay<'a>(self: &'a Rc<Self>, dur: Duration) -> Delay<'a> {
    Delay {
      runtime: self,
      dur: Some(dur),
      id: None,
    }
  }

  // TODO(bnoordhuis) don't box futures that are Unpin
  pub fn spawn<Fut>(self: &Rc<Self>, fut: Fut)
  where
    Fut: Future<Output = ()> + 'static,
  {
    let waker = dummy_waker();
    let cx = &mut Context::from_waker(&waker);
    let mut fut = Box::pin(fut);
    if fut.as_mut().poll(cx).is_pending() {
      unsafe_borrow_mut(&self.state).tasks.push(fut);
    }
  }

  fn can(
    self: &Rc<Self>,
    waker: &Waker,
    interest: mio::Interest,
    slot: &UnsafeCell<usize>,
    source: &UnsafeCell<impl mio::event::Source>,
  ) -> io::Result<Poll<bool>> {
    let slot = unsafe_borrow_mut(slot);

    if *slot == usize::MAX {
      return Ok(Ready(true)); // Optimistically assume I/O object is ready.
    }

    let state = unsafe_borrow_mut(&self.state);
    let io = get_or_new(&mut state.free, &mut state.used, slot);

    let want_read = interest.is_readable();
    let want_write = interest.is_writable();

    if want_read && io.read_closed {
      return Ok(Ready(false));
    }

    if want_write && io.write_closed {
      return Ok(Ready(false));
    }

    if want_read && io.readable.unwrap_or(true) {
      return Ok(Ready(true)); // Either readable or optimistically assume it is.
    }

    if want_write && io.writable.unwrap_or(true) {
      return Ok(Ready(true)); // Either writable or optimistically assume it is.
    }

    let new = io.readable.is_none() && io.writable.is_none();
    let new_read = want_read && io.readable.is_none();
    let new_write = want_write && io.writable.is_none();

    // Either: 1) we first-time register,
    //         2) add an interest, or
    //         3) do nothing.
    if new {
      state.poll.registry().register(
        unsafe_borrow_mut(source),
        mio::Token(*slot),
        interest,
      )?;
      state.refs += 1;
    } else if new_read ^ new_write {
      state.poll.registry().reregister(
        unsafe_borrow_mut(source),
        mio::Token(*slot),
        mio::Interest::READABLE | mio::Interest::WRITABLE,
      )?;
    }

    if want_read {
      io.readable = Some(false);
      io.read_waker = Some(waker.clone());
    }

    if want_write {
      io.writable = Some(false);
      io.write_waker = Some(waker.clone());
    }

    Ok(Pending)
  }

  fn partial(
    self: &Rc<Self>,
    interest: mio::Interest,
    slot: &UnsafeCell<usize>,
    source: &UnsafeCell<impl mio::event::Source>,
  ) -> io::Result<()> {
    let want_read = interest.is_readable();
    let want_write = interest.is_writable();

    let state = unsafe_borrow_mut(&self.state);
    let slot = unsafe_borrow_mut(slot);
    let io = get_or_new(&mut state.free, &mut state.used, slot);

    let new = io.readable.is_none() && io.writable.is_none();
    let new_read = want_read && io.readable.is_none();
    let new_write = want_write && io.writable.is_none();

    // Either: 1) we first-time register, 2) add an interest, or 3) do nothing.
    if new {
      state.poll.registry().register(
        unsafe_borrow_mut(source),
        mio::Token(*slot),
        interest,
      )?;
      state.refs += 1;
    } else if new_read ^ new_write {
      state.poll.registry().reregister(
        unsafe_borrow_mut(source),
        mio::Token(*slot),
        mio::Interest::READABLE | mio::Interest::WRITABLE,
      )?;
    }

    if want_read {
      io.readable = Some(false);
    }

    if want_write {
      io.writable = Some(false);
    }

    Ok(())
  }

  fn deregister(
    self: &Rc<Self>,
    slot: &UnsafeCell<usize>,
    source: &UnsafeCell<impl mio::event::Source>,
  ) {
    let slot = unsafe_borrow_mut(slot);
    let slot = std::mem::replace(slot, usize::MAX); // poison slot

    if slot == usize::MAX {
      return;
    }

    let state = unsafe_borrow_mut(&self.state);
    let source = unsafe_borrow_mut(source);
    state.deregister(slot, source);
  }
}

pub struct Delay<'a> {
  runtime: &'a Rc<Runtime>,
  dur: Option<Duration>,
  id: Option<u64>,
}

impl<'a> Unpin for Delay<'a> {}

impl<'a> Future for Delay<'a> {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
    if let Some(dur) = self.dur.take() {
      let waker = cx.waker().clone();
      let state = unsafe_borrow_mut(&self.runtime.state);

      let timerids = &mut state.timerids;
      let id = std::mem::replace(timerids, *timerids + 1);

      let now = Instant::now();
      let instant = now.checked_add(dur).unwrap_or(now);
      state.timers.push(Timer { instant, waker, id });

      Pending
    } else {
      Ready(())
    }
  }
}

impl<'a> Drop for Delay<'a> {
  fn drop(&mut self) {
    if let Some(id) = self.id.take() {
      let state = unsafe_borrow_mut(&self.runtime.state);
      let timers = &mut state.timers;

      if let Some(index) = timers.iter().position(|timer| timer.id == id) {
        timers.swap_remove(index);
      }
    }
  }
}

impl TcpListener {
  pub fn bind(runtime: &Rc<Runtime>, addr: SocketAddr) -> io::Result<Self> {
    let inner = mio::net::TcpListener::bind(addr)?;
    Ok(Self {
      inner: UnsafeCell::new(inner),
      runtime: Rc::clone(runtime),
      slot: UnsafeCell::new(usize::MAX),
    })
  }

  pub fn set_defer_accept(&self, dur: Duration) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
      use std::os::unix::io::AsRawFd;
      let seconds = dur.as_secs().try_into().unwrap_or(u32::MAX);
      // SAFETY: libc call.
      let rc = unsafe {
        libc::setsockopt(
          unsafe_borrow_mut(&self.inner).as_raw_fd(),
          libc::IPPROTO_TCP,
          libc::TCP_DEFER_ACCEPT,
          &seconds as *const _ as *const _,
          std::mem::size_of_val(&seconds) as u32,
        )
      };
      if rc == -1 {
        Err(io::Error::last_os_error())
      } else {
        Ok(())
      }
    }
    #[cfg(not(target_os = "linux"))]
    {
      let _ = dur;
      Ok(())
    }
  }

  pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
    poll_fn(|cx| self.try_accept(cx)).await
  }

  pub fn try_accept(
    &self,
    cx: &mut Context,
  ) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
    let interest = mio::Interest::READABLE;
    let inner = &self.inner;
    let slot = &self.slot;
    if Pending == self.runtime.can(cx.waker(), interest, slot, inner)? {
      return Pending;
    }
    match while_interrupted(|| unsafe_borrow_mut(inner).accept()) {
      Ok((stream, addr)) => {
        let stream = TcpStream {
          runtime: Rc::clone(&self.runtime),
          inner: UnsafeCell::new(stream),
          slot: UnsafeCell::new(usize::MAX),
        };
        Ready(Ok((stream, addr)))
      }
      Err(e) if is_wouldblock(&e) => {
        self.runtime.partial(interest, slot, inner)?;
        Pending
      }
      Err(e) => Ready(Err(e)),
    }
  }
}

impl Unpin for TcpListener {}

// impl futures_core::Stream for TcpListener {
//   type Item = io::Result<(TcpStream, SocketAddr)>;

//   fn poll_next(
//     self: Pin<&mut Self>,
//     cx: &mut Context,
//   ) -> Poll<Option<Self::Item>> {
//     self.try_accept(cx).map(Some)
//   }
// }

// impl futures_core::stream::FusedStream for TcpListener {
//   fn is_terminated(&self) -> bool {
//     false
//   }
// }

impl Drop for TcpListener {
  fn drop(&mut self) {
    self.runtime.deregister(&self.slot, &self.inner);
  }
}

impl TcpStream {
  pub async fn connect(
    runtime: &Rc<Runtime>,
    addr: SocketAddr,
  ) -> io::Result<Self> {
    let mut slot = usize::MAX;
    let mut inner = mio::net::TcpStream::connect(addr)?;

    let state = unsafe_borrow_mut(&runtime.state);
    let io = get_or_new(&mut state.free, &mut state.used, &mut slot);
    io.writable = Some(false);

    state.poll.registry().register(
      &mut inner,
      mio::Token(slot),
      mio::Interest::WRITABLE,
    )?;
    state.refs += 1;

    loop {
      let mut r = Pending;

      // wait for writability
      poll_fn(|cx| {
        if r.is_pending() {
          let state = unsafe_borrow_mut(&runtime.state);
          let io = get_or_new(&mut state.free, &mut state.used, &mut slot);
          io.write_waker = Some(cx.waker().clone());
        }
        std::mem::replace(&mut r, Ready(()))
      })
      .await;

      // now check if we're connected
      match inner.peer_addr() {
        Err(ref e) if is_not_connected(e) => (), // not yet, try again
        Err(e) => {
          let state = unsafe_borrow_mut(&runtime.state);
          state.deregister(slot, &mut inner);
          return Err(e);
        }
        Ok(_) => {
          return Ok(Self {
            runtime: Rc::clone(runtime),
            inner: UnsafeCell::new(inner),
            slot: UnsafeCell::new(slot),
          })
        }
      }
    }
  }

  pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
    poll_fn(|cx| self.try_read(cx, buf)).await
  }

  pub async fn write(&self, buf: &[u8]) -> io::Result<usize> {
    poll_fn(|cx| self.try_write_inner(cx, buf)).await
  }

  fn try_read(
    &self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<io::Result<usize>> {
    self.try_io(cx, buf.len(), mio::Interest::READABLE, || {
      io::Read::read(unsafe_borrow_mut(&self.inner), buf)
    })
  }

  fn try_write_inner(
    &self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    self.try_io(cx, buf.len(), mio::Interest::WRITABLE, || {
      io::Write::write(unsafe_borrow_mut(&self.inner), buf)
    })
  }

  pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
    io::Write::write(unsafe_borrow_mut(&self.inner), buf)
  }

  fn try_io(
    &self,
    cx: &mut Context,
    len: usize,
    interest: mio::Interest,
    f: impl FnMut() -> io::Result<usize>,
  ) -> Poll<io::Result<usize>> {
    let inner = &self.inner;
    let slot = &self.slot;
    match self.runtime.can(cx.waker(), interest, slot, inner)? {
      Ready(false) => return Ready(Ok(0)), // EOF
      Ready(true) => (),
      Pending => return Pending,
    }
    match while_interrupted(f) {
      Ok(n) if n < len => {
        self.runtime.partial(interest, slot, inner)?;
        Ready(Ok(n))
      }
      Err(e) if is_wouldblock(&e) => {
        self.runtime.partial(interest, slot, inner)?;
        Pending
      }
      x => Ready(x),
    }
  }
}

impl Unpin for TcpStream {}

impl tokio::io::AsyncRead for TcpStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut tokio::io::ReadBuf,
  ) -> Poll<io::Result<()>> {
    let unfilled =
      // SAFETY: we are careful not to leak |unfilled|
      unsafe { &mut *(buf.unfilled_mut() as *mut [_] as *mut [u8]) };
    if let Ready(n) = self.try_read(cx, unfilled)? {
      // SAFETY: initialized by system call.
      unsafe { buf.assume_init(n) };
      buf.advance(n);
      Ready(Ok(()))
    } else {
      Pending
    }
  }
}

impl tokio::io::AsyncWrite for TcpStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    self.try_write_inner(cx, buf)
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    _cx: &mut Context,
  ) -> Poll<io::Result<()>> {
    Poll::Ready(Ok(()))
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    _cx: &mut Context,
  ) -> Poll<io::Result<()>> {
    Poll::Ready(Ok(()))
  }
}

impl Drop for TcpStream {
  fn drop(&mut self) {
    self.runtime.deregister(&self.slot, &self.inner);
  }
}

#[inline]
fn get_or_new<'a>(
  free: &mut Vec<usize>,
  used: &'a mut Vec<Io>,
  slot: &mut usize,
) -> &'a mut Io {
  if *slot == usize::MAX {
    *slot = free.pop().unwrap_or_else(|| {
      used.push(Io::default());
      used.len() - 1
    });
  }

  &mut used[*slot]
}

#[inline]
fn while_interrupted<R>(mut f: impl FnMut() -> io::Result<R>) -> io::Result<R> {
  loop {
    match f() {
      Err(ref e) if is_interrupted(e) => (),
      x => break x,
    }
  }
}

#[allow(clippy::mut_from_ref)] // Shut up, clippy. That's the whole point.
#[inline]
fn unsafe_borrow_mut<T>(cell: &UnsafeCell<T>) -> &mut T {
  // SAFETY: callers take care to not create concurrent mutable references.
  unsafe { &mut *cell.get() }
}

#[inline]
fn is_interrupted(e: &io::Error) -> bool {
  e.kind() == io::ErrorKind::Interrupted
}

#[inline]
fn is_not_connected(e: &io::Error) -> bool {
  e.kind() == io::ErrorKind::NotConnected
}

#[inline]
fn is_wouldblock(e: &io::Error) -> bool {
  e.kind() == io::ErrorKind::WouldBlock
}

fn dummy_waker() -> Waker {
  unsafe fn drop_waker(_: *const ()) {}
  unsafe fn wake_waker(_: *const ()) {}
  unsafe fn wake_waker_by_ref(_: *const ()) {}
  unsafe fn clone_waker(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
  }

  const VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_waker, wake_waker, wake_waker_by_ref, drop_waker);

  let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
  // SAFETY: upholds RawWaker and RawWakerVTable contract.
  unsafe { Waker::from_raw(raw_waker) }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::AtomicBool;
  use std::sync::atomic::Ordering::Relaxed;

  fn rt() -> Rc<Runtime> {
    Runtime::new().expect("create new runtime")
  }

  #[test]
  fn block_on() {
    rt().block_on(async {})
  }

  #[test]
  fn spawn_immediate() {
    thread_local!(static OK: AtomicBool = AtomicBool::default());
    let ok = || OK.with(|ok| ok.load(Relaxed));
    rt().spawn(async { OK.with(|ok| ok.store(true, Relaxed)) });
    assert!(ok());
  }

  #[test]
  fn spawn_delay() {
    thread_local!(static OK: AtomicBool = AtomicBool::default());
    let ok = || OK.with(|ok| ok.load(Relaxed));
    let rt = rt();
    rt.spawn(async {
      pending_once().await;
      OK.with(|ok| ok.store(true, Relaxed));
    });
    assert!(!ok());
    rt.block_on(async {
      assert!(!ok());
      pending_once().await;
      assert!(ok());
    });
    assert!(ok());
  }

  #[test]
  fn delay() {
    let rt = rt();
    rt.block_on(async {
      let expected = Duration::from_millis(10);
      let before = Instant::now();
      rt.delay(expected).await;
      let actual = Instant::now()
        .checked_duration_since(before)
        .expect("causality violation");
      assert!(actual >= expected);
    })
  }

  #[test]
  fn delay_drop() {
    let rt = rt();
    rt.block_on(async {
      let mut delay = rt.delay(Duration::from_millis(1));
      let _ = poll_fn(|cx| Ready(Pin::new(&mut delay).poll(cx))).await;
      drop(delay);
    })
  }

  async fn pending_once() {
    let mut r = Pending;
    poll_fn(|_| std::mem::replace(&mut r, Ready(()))).await
  }
}
