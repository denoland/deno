use std::cell::UnsafeCell;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::RawWaker;
use std::task::RawWakerVTable;
use std::task::Waker;
use std::time::Duration;

pub struct Runtime {
  state: UnsafeCell<RuntimeState>,
}

struct RuntimeState {
  free: Vec<usize>,
  used: Vec<Io>, // Never shrinks but sooner or later turns semi-stable.
  poll: mio::Poll,
  events: mio::Events,
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

impl RuntimeState {
  fn new() -> io::Result<Self> {
    Ok(Self {
      free: Vec::new(),
      used: Vec::new(),
      poll: mio::Poll::new()?,
      events: mio::Events::with_capacity(256),
    })
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
    // These can be dummies because the future doesn't need waking up,
    // we simply poll it whenever the event loop makes progress.
    unsafe fn drop_waker(_: *const ()) {}
    unsafe fn wake_waker(_: *const ()) {}
    unsafe fn wake_waker_by_ref(_: *const ()) {}
    unsafe fn clone_waker(data: *const ()) -> RawWaker {
      RawWaker::new(data, &VTABLE)
    }

    const VTABLE: RawWakerVTable = RawWakerVTable::new(
      clone_waker,
      wake_waker,
      wake_waker_by_ref,
      drop_waker,
    );

    let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
    // SAFETY: upholds RawWaker and RawWakerVTable contract.
    let waker = unsafe { Waker::from_raw(raw_waker) };
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

      loop {
        let timeout = None; // TODO(bnoordhuis) Timer support.

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

  // wouldblock=true indicates the caller already tried the operation,
  // got EWOULDBLOCK, and now wants to sleep until readable/writable.
  async fn can(
    self: &Rc<Self>,
    interest: mio::Interest,
    slot: &UnsafeCell<usize>,
    source: &UnsafeCell<impl mio::event::Source>,
    wouldblock: bool,
  ) -> io::Result<bool> {
    let slot = unsafe_borrow_mut(slot);

    if !wouldblock && *slot == usize::MAX {
      return Ok(true); // Optimistically assume I/O object is ready.
    }

    let state = unsafe_borrow_mut(&self.state);
    let io = get_or_new(&mut state.free, &mut state.used, slot);

    let want_read = interest.is_readable();
    let want_write = interest.is_writable();

    if !wouldblock && want_read && io.readable.unwrap_or(true) {
      return Ok(true); // It's either readable or optimistically assume it is.
    }

    if !wouldblock && want_write && io.writable.unwrap_or(true) {
      return Ok(true); // It's either writable or optimistically assume it is.
    }

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

    let slot = *slot; // Drops borrow.
    let mut first_time = Some(io); // Drops borrow after first take().

    poll_fn(|cx| {
      let io = if let Some(io) = first_time.take() {
        io
      } else {
        let io = &mut unsafe_borrow_mut(&self.state).used[slot];
        if io.error
          || want_read && io.read_closed
          || want_write && io.write_closed
        {
          return Ok(false).into();
        }
        if want_read && io.readable == Some(true) {
          return Ok(true).into();
        }
        if want_write && io.writable == Some(true) {
          return Ok(true).into();
        }
        io
      };
      if want_read {
        io.read_waker = Some(cx.waker().clone());
      }
      if want_write {
        io.write_waker = Some(cx.waker().clone());
      }
      Poll::Pending
    })
    .await
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
  ) -> io::Result<()> {
    let slot = std::mem::replace(unsafe_borrow_mut(slot), usize::MAX);

    if slot == usize::MAX {
      return Ok(());
    }

    let state = unsafe_borrow_mut(&self.state);

    // Intentionally leaks the slot on error because it's not safe to reuse it.
    // On the bright side: barring program bugs that should never happen.
    state
      .poll
      .registry()
      .deregister(unsafe_borrow_mut(source))?;

    state.used[slot] = Io::default();
    state.free.push(slot);

    Ok(())
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
        c::setsockopt(
          unsafe_borrow_mut(&self.inner).as_raw_fd(),
          c::IPPROTO_TCP,
          c::TCP_DEFER_ACCEPT,
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
    let mut wouldblock = false;
    loop {
      self
        .runtime
        .can(mio::Interest::READABLE, &self.slot, &self.inner, wouldblock)
        .await?;
      match unsafe_borrow_mut(&self.inner).accept() {
        Err(ref e) if is_interrupted(e) => (),
        Err(ref e) if is_wouldblock(e) => wouldblock = true,
        Err(e) => return Err(e),
        Ok((stream, addr)) => {
          let stream = TcpStream::from_mio(&self.runtime, stream);
          return Ok((stream, addr));
        }
      }
    }
  }
}

impl Drop for TcpListener {
  fn drop(&mut self) {
    if let Err(e) = self.runtime.deregister(&self.slot, &self.inner) {
      eprintln!("polloi: deregister: {}", e);
    }
  }
}

impl TcpStream {
  pub(crate) fn from_mio(
    runtime: &Rc<Runtime>,
    stream: mio::net::TcpStream,
  ) -> Self {
    Self {
      runtime: Rc::clone(runtime),
      inner: UnsafeCell::new(stream),
      slot: UnsafeCell::new(usize::MAX),
    }
  }

  pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
    let interest = mio::Interest::READABLE;
    let mut wouldblock = false;
    loop {
      if !self
        .runtime
        .can(interest, &self.slot, &self.inner, wouldblock)
        .await?
      {
        return Ok(0); // Connection hangup.
      }
      use std::io::Read;
      match unsafe_borrow_mut(&self.inner).read(buf) {
        Err(ref e) if is_interrupted(e) => (),
        Err(ref e) if is_wouldblock(e) => wouldblock = true,
        Err(e) => return Err(e),
        Ok(n) => {
          if n < buf.len() {
            self.runtime.partial(interest, &self.slot, &self.inner)?;
          }
          return Ok(n);
        }
      }
    }
  }

  pub async fn write(&self, buf: &[u8]) -> io::Result<usize> {
    let interest = mio::Interest::WRITABLE;
    let mut wouldblock = false;
    loop {
      if !self
        .runtime
        .can(interest, &self.slot, &self.inner, wouldblock)
        .await?
      {
        return Ok(0); // Connection hangup.
      }
      use std::io::Write;
      match unsafe_borrow_mut(&self.inner).write(buf) {
        Err(ref e) if is_interrupted(e) => (),
        Err(ref e) if is_wouldblock(e) => wouldblock = true,
        Err(e) => return Err(e),
        Ok(n) => {
          if n < buf.len() {
            self.runtime.partial(interest, &self.slot, &self.inner)?;
          }
          return Ok(n);
        }
      }
    }
  }
}

impl Drop for TcpStream {
  fn drop(&mut self) {
    if let Err(e) = self.runtime.deregister(&self.slot, &self.inner) {
      eprintln!("polloi: deregister: {}", e);
    }
  }
}

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

#[allow(clippy::mut_from_ref)] // Shut up, clippy. That's the whole point.
fn unsafe_borrow_mut<T>(cell: &UnsafeCell<T>) -> &mut T {
  // SAFETY: callers take care to not create concurrent mutable references.
  unsafe { &mut *cell.get() }
}

fn is_interrupted(e: &io::Error) -> bool {
  e.kind() == io::ErrorKind::Interrupted
}

fn is_wouldblock(e: &io::Error) -> bool {
  e.kind() == io::ErrorKind::WouldBlock
}

fn poll_fn<F, T>(f: F) -> PollFn<F>
where
  F: FnMut(&mut Context) -> Poll<T>,
{
  PollFn { f }
}

struct PollFn<F> {
  f: F,
}

impl<F, T> Future for PollFn<F>
where
  F: FnMut(&mut Context) -> Poll<T>,
  F: Unpin,
{
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<T> {
    (self.get_mut().f)(cx)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn rt() -> Rc<Runtime> {
    Runtime::new().expect("create new runtime")
  }

  #[test]
  fn block_on() {
    rt().block_on(async {})
  }
}
