// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(unused)]

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::task::ready;
use std::task::Context;
use std::task::Poll;

use deno_core::serde;
use deno_core::serde_json;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::ExternalOpsTracker;
use deno_core::RcRef;
use deno_io::BiPipe;
use deno_io::BiPipeRead;
use deno_io::BiPipeWrite;
use memchr::memchr;
use pin_project_lite::pin_project;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;
use tokio::io::ReadBuf;

/// Tracks whether the IPC resources is currently
/// refed, and allows refing/unrefing it.
pub struct IpcRefTracker {
  refed: AtomicBool,
  tracker: OpsTracker,
}

/// A little wrapper so we don't have to get an
/// `ExternalOpsTracker` for tests. When we aren't
/// cfg(test), this will get optimized out.
enum OpsTracker {
  External(ExternalOpsTracker),
  #[cfg(test)]
  Test,
}

impl OpsTracker {
  fn ref_(&self) {
    match self {
      Self::External(tracker) => tracker.ref_op(),
      #[cfg(test)]
      Self::Test => {}
    }
  }

  fn unref(&self) {
    match self {
      Self::External(tracker) => tracker.unref_op(),
      #[cfg(test)]
      Self::Test => {}
    }
  }
}

impl IpcRefTracker {
  pub fn new(tracker: ExternalOpsTracker) -> Self {
    Self {
      refed: AtomicBool::new(false),
      tracker: OpsTracker::External(tracker),
    }
  }

  #[cfg(test)]
  fn new_test() -> Self {
    Self {
      refed: AtomicBool::new(false),
      tracker: OpsTracker::Test,
    }
  }

  pub fn ref_(&self) {
    if !self.refed.swap(true, std::sync::atomic::Ordering::AcqRel) {
      self.tracker.ref_();
    }
  }

  pub fn unref(&self) {
    if self.refed.swap(false, std::sync::atomic::Ordering::AcqRel) {
      self.tracker.unref();
    }
  }
}

pub struct IpcJsonStreamResource {
  pub read_half: AsyncRefCell<IpcJsonStream>,
  pub write_half: AsyncRefCell<BiPipeWrite>,
  pub cancel: Rc<CancelHandle>,
  pub queued_bytes: AtomicUsize,
  pub ref_tracker: IpcRefTracker,
}

impl deno_core::Resource for IpcJsonStreamResource {
  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl IpcJsonStreamResource {
  pub fn new(
    stream: i64,
    ref_tracker: IpcRefTracker,
  ) -> Result<Self, std::io::Error> {
    let (read_half, write_half) = BiPipe::from_raw(stream as _)?.split();
    Ok(Self {
      read_half: AsyncRefCell::new(IpcJsonStream::new(read_half)),
      write_half: AsyncRefCell::new(write_half),
      cancel: Default::default(),
      queued_bytes: Default::default(),
      ref_tracker,
    })
  }

  #[cfg(all(unix, test))]
  pub fn from_stream(
    stream: tokio::net::UnixStream,
    ref_tracker: IpcRefTracker,
  ) -> Self {
    let (read_half, write_half) = stream.into_split();
    Self {
      read_half: AsyncRefCell::new(IpcJsonStream::new(read_half.into())),
      write_half: AsyncRefCell::new(write_half.into()),
      cancel: Default::default(),
      queued_bytes: Default::default(),
      ref_tracker,
    }
  }

  #[cfg(all(windows, test))]
  pub fn from_stream(
    pipe: tokio::net::windows::named_pipe::NamedPipeClient,
    ref_tracker: IpcRefTracker,
  ) -> Self {
    let (read_half, write_half) = tokio::io::split(pipe);
    Self {
      read_half: AsyncRefCell::new(IpcJsonStream::new(read_half.into())),
      write_half: AsyncRefCell::new(write_half.into()),
      cancel: Default::default(),
      queued_bytes: Default::default(),
      ref_tracker,
    }
  }

  /// writes _newline terminated_ JSON message to the IPC pipe.
  pub async fn write_msg_bytes(
    self: Rc<Self>,
    msg: &[u8],
  ) -> Result<(), io::Error> {
    let mut write_half = RcRef::map(self, |r| &r.write_half).borrow_mut().await;
    write_half.write_all(msg).await?;
    Ok(())
  }
}

// Initial capacity of the buffered reader and the JSON backing buffer.
//
// This is a tradeoff between memory usage and performance on large messages.
//
// 64kb has been chosen after benchmarking 64 to 66536 << 6 - 1 bytes per message.
pub const INITIAL_CAPACITY: usize = 1024 * 64;

/// A buffer for reading from the IPC pipe.
/// Similar to the internal buffer of `tokio::io::BufReader`.
///
/// This exists to provide buffered reading while granting mutable access
/// to the internal buffer (which isn't exposed through `tokio::io::BufReader`
/// or the `AsyncBufRead` trait). `simd_json` requires mutable access to an input
/// buffer for parsing, so this allows us to use the read buffer directly as the
/// input buffer without a copy (provided the message fits).
struct ReadBuffer {
  buffer: Box<[u8]>,
  pos: usize,
  cap: usize,
}

impl ReadBuffer {
  fn new() -> Self {
    Self {
      buffer: vec![0; INITIAL_CAPACITY].into_boxed_slice(),
      pos: 0,
      cap: 0,
    }
  }

  fn get_mut(&mut self) -> &mut [u8] {
    &mut self.buffer
  }

  fn available_mut(&mut self) -> &mut [u8] {
    &mut self.buffer[self.pos..self.cap]
  }

  fn consume(&mut self, n: usize) {
    self.pos = std::cmp::min(self.pos + n, self.cap);
  }

  fn needs_fill(&self) -> bool {
    self.pos >= self.cap
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum IpcJsonStreamError {
  #[class(inherit)]
  #[error("{0}")]
  Io(#[source] std::io::Error),
  #[class(generic)]
  #[error("{0}")]
  SimdJson(#[source] simd_json::Error),
}

// JSON serialization stream over IPC pipe.
//
// `\n` is used as a delimiter between messages.
pub struct IpcJsonStream {
  pipe: BiPipeRead,
  buffer: Vec<u8>,
  read_buffer: ReadBuffer,
}

impl IpcJsonStream {
  fn new(pipe: BiPipeRead) -> Self {
    Self {
      pipe,
      buffer: Vec::with_capacity(INITIAL_CAPACITY),
      read_buffer: ReadBuffer::new(),
    }
  }

  pub async fn read_msg(
    &mut self,
  ) -> Result<Option<serde_json::Value>, IpcJsonStreamError> {
    let mut json = None;
    let nread = read_msg_inner(
      &mut self.pipe,
      &mut self.buffer,
      &mut json,
      &mut self.read_buffer,
    )
    .await
    .map_err(IpcJsonStreamError::Io)?;
    if nread == 0 {
      // EOF.
      return Ok(None);
    }

    let json = match json {
      Some(v) => v,
      None => {
        // Took more than a single read and some buffering.
        simd_json::from_slice(&mut self.buffer[..nread])
          .map_err(IpcJsonStreamError::SimdJson)?
      }
    };

    // Safety: Same as `Vec::clear` but without the `drop_in_place` for
    // each element (nop for u8). Capacity remains the same.
    unsafe {
      self.buffer.set_len(0);
    }

    Ok(Some(json))
  }
}

pin_project! {
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    struct ReadMsgInner<'a, R: ?Sized> {
        reader: &'a mut R,
        buf: &'a mut Vec<u8>,
        json: &'a mut Option<serde_json::Value>,
        // The number of bytes appended to buf. This can be less than buf.len() if
        // the buffer was not empty when the operation was started.
        read: usize,
        read_buffer: &'a mut ReadBuffer,
    }
}

fn read_msg_inner<'a, R>(
  reader: &'a mut R,
  buf: &'a mut Vec<u8>,
  json: &'a mut Option<serde_json::Value>,
  read_buffer: &'a mut ReadBuffer,
) -> ReadMsgInner<'a, R>
where
  R: AsyncRead + ?Sized + Unpin,
{
  ReadMsgInner {
    reader,
    buf,
    json,
    read: 0,
    read_buffer,
  }
}

fn read_msg_internal<R: AsyncRead + ?Sized>(
  mut reader: Pin<&mut R>,
  cx: &mut Context<'_>,
  buf: &mut Vec<u8>,
  read_buffer: &mut ReadBuffer,
  json: &mut Option<serde_json::Value>,
  read: &mut usize,
) -> Poll<io::Result<usize>> {
  loop {
    let (done, used) = {
      // effectively a tiny `poll_fill_buf`, but allows us to get a mutable reference to the buffer.
      if read_buffer.needs_fill() {
        let mut read_buf = ReadBuf::new(read_buffer.get_mut());
        ready!(reader.as_mut().poll_read(cx, &mut read_buf))?;
        read_buffer.cap = read_buf.filled().len();
        read_buffer.pos = 0;
      }
      let available = read_buffer.available_mut();
      if let Some(i) = memchr(b'\n', available) {
        if *read == 0 {
          // Fast path: parse and put into the json slot directly.
          json.replace(
            simd_json::from_slice(&mut available[..i + 1])
              .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
          );
        } else {
          // This is not the first read, so we have to copy the data
          // to make it contiguous.
          buf.extend_from_slice(&available[..=i]);
        }
        (true, i + 1)
      } else {
        buf.extend_from_slice(available);
        (false, available.len())
      }
    };

    read_buffer.consume(used);
    *read += used;
    if done || used == 0 {
      return Poll::Ready(Ok(mem::replace(read, 0)));
    }
  }
}

impl<R: AsyncRead + ?Sized + Unpin> Future for ReadMsgInner<'_, R> {
  type Output = io::Result<usize>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let me = self.project();
    read_msg_internal(
      Pin::new(*me.reader),
      cx,
      me.buf,
      me.read_buffer,
      me.json,
      me.read,
    )
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use deno_core::serde_json::json;
  use deno_core::v8;
  use deno_core::JsRuntime;
  use deno_core::RcRef;
  use deno_core::RuntimeOptions;

  use super::IpcJsonStreamResource;

  #[allow(clippy::unused_async)]
  #[cfg(unix)]
  pub async fn pair() -> (Rc<IpcJsonStreamResource>, tokio::net::UnixStream) {
    let (a, b) = tokio::net::UnixStream::pair().unwrap();

    /* Similar to how ops would use the resource */
    let a = Rc::new(IpcJsonStreamResource::from_stream(
      a,
      super::IpcRefTracker::new_test(),
    ));
    (a, b)
  }

  #[cfg(windows)]
  pub async fn pair() -> (
    Rc<IpcJsonStreamResource>,
    tokio::net::windows::named_pipe::NamedPipeServer,
  ) {
    use tokio::net::windows::named_pipe::ClientOptions;
    use tokio::net::windows::named_pipe::ServerOptions;

    let name =
      format!(r"\\.\pipe\deno-named-pipe-test-{}", rand::random::<u32>());

    let server = ServerOptions::new().create(name.clone()).unwrap();
    let client = ClientOptions::new().open(name).unwrap();

    server.connect().await.unwrap();
    /* Similar to how ops would use the resource */
    let client = Rc::new(IpcJsonStreamResource::from_stream(
      client,
      super::IpcRefTracker::new_test(),
    ));
    (client, server)
  }

  #[allow(clippy::print_stdout)]
  #[tokio::test]
  async fn bench_ipc() -> Result<(), Box<dyn std::error::Error>> {
    // A simple round trip benchmark for quick dev feedback.
    //
    // Only ran when the env var is set.
    if std::env::var_os("BENCH_IPC_DENO").is_none() {
      return Ok(());
    }

    let (ipc, mut fd2) = pair().await;
    let child = tokio::spawn(async move {
      use tokio::io::AsyncWriteExt;

      let size = 1024 * 1024;

      let stri = "x".repeat(size);
      let data = format!("\"{}\"\n", stri);
      for _ in 0..100 {
        fd2.write_all(data.as_bytes()).await?;
      }
      Ok::<_, std::io::Error>(())
    });

    let start = std::time::Instant::now();
    let mut bytes = 0;

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
    loop {
      let Some(msgs) = ipc.read_msg().await? else {
        break;
      };
      bytes += msgs.as_str().unwrap().len();
      if start.elapsed().as_secs() > 5 {
        break;
      }
    }
    let elapsed = start.elapsed();
    let mb = bytes as f64 / 1024.0 / 1024.0;
    println!("{} mb/s", mb / elapsed.as_secs_f64());

    child.await??;

    Ok(())
  }

  #[tokio::test]
  async fn unix_ipc_json() -> Result<(), Box<dyn std::error::Error>> {
    let (ipc, mut fd2) = pair().await;
    let child = tokio::spawn(async move {
      use tokio::io::AsyncReadExt;
      use tokio::io::AsyncWriteExt;

      const EXPECTED: &[u8] = b"\"hello\"\n";
      let mut buf = [0u8; EXPECTED.len()];
      let n = fd2.read_exact(&mut buf).await?;
      assert_eq!(&buf[..n], EXPECTED);
      fd2.write_all(b"\"world\"\n").await?;

      Ok::<_, std::io::Error>(())
    });

    ipc
      .clone()
      .write_msg_bytes(&json_to_bytes(json!("hello")))
      .await?;

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
    let msgs = ipc.read_msg().await?.unwrap();
    assert_eq!(msgs, json!("world"));

    child.await??;

    Ok(())
  }

  fn json_to_bytes(v: deno_core::serde_json::Value) -> Vec<u8> {
    let mut buf = deno_core::serde_json::to_vec(&v).unwrap();
    buf.push(b'\n');
    buf
  }

  #[tokio::test]
  async fn unix_ipc_json_multi() -> Result<(), Box<dyn std::error::Error>> {
    let (ipc, mut fd2) = pair().await;
    let child = tokio::spawn(async move {
      use tokio::io::AsyncReadExt;
      use tokio::io::AsyncWriteExt;

      const EXPECTED: &[u8] = b"\"hello\"\n\"world\"\n";
      let mut buf = [0u8; EXPECTED.len()];
      let n = fd2.read_exact(&mut buf).await?;
      assert_eq!(&buf[..n], EXPECTED);
      fd2.write_all(b"\"foo\"\n\"bar\"\n").await?;
      Ok::<_, std::io::Error>(())
    });

    ipc
      .clone()
      .write_msg_bytes(&json_to_bytes(json!("hello")))
      .await?;
    ipc
      .clone()
      .write_msg_bytes(&json_to_bytes(json!("world")))
      .await?;

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
    let msgs = ipc.read_msg().await?.unwrap();
    assert_eq!(msgs, json!("foo"));

    child.await??;

    Ok(())
  }

  #[tokio::test]
  async fn unix_ipc_json_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let (ipc, mut fd2) = pair().await;
    let child = tokio::spawn(async move {
      tokio::io::AsyncWriteExt::write_all(&mut fd2, b"\n\n").await?;
      Ok::<_, std::io::Error>(())
    });

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
    let _err = ipc.read_msg().await.unwrap_err();

    child.await??;

    Ok(())
  }

  #[test]
  fn memchr() {
    let str = b"hello world";
    assert_eq!(super::memchr(b'h', str), Some(0));
    assert_eq!(super::memchr(b'w', str), Some(6));
    assert_eq!(super::memchr(b'd', str), Some(10));
    assert_eq!(super::memchr(b'x', str), None);

    let empty = b"";
    assert_eq!(super::memchr(b'\n', empty), None);
  }
}
