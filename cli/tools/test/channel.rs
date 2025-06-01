// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Display;
use std::future::poll_fn;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::ready;
use std::task::Poll;
use std::time::Duration;

use deno_core::parking_lot;
use deno_core::parking_lot::lock_api::RawMutex;
use deno_core::parking_lot::lock_api::RawMutexTimed;
use deno_runtime::deno_io::pipe;
use deno_runtime::deno_io::AsyncPipeRead;
use deno_runtime::deno_io::PipeRead;
use deno_runtime::deno_io::PipeWrite;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::ReadBuf;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::WeakUnboundedSender;

use super::TestEvent;

/// 8-byte sync marker that is unlikely to appear in normal output. Equivalent
/// to the string `"\u{200B}\0\u{200B}\0"`.
const SYNC_MARKER: &[u8; 8] = &[226, 128, 139, 0, 226, 128, 139, 0];
const HALF_SYNC_MARKER: &[u8; 4] = &[226, 128, 139, 0];

const BUFFER_SIZE: usize = 4096;

/// The test channel has been closed and cannot be used to send further messages.
#[derive(Debug, Copy, Clone, Eq, PartialEq, deno_error::JsError)]
#[class(generic)]
pub struct ChannelClosedError;

impl std::error::Error for ChannelClosedError {}

impl Display for ChannelClosedError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Test channel closed")
  }
}

impl<T> From<SendError<T>> for ChannelClosedError {
  fn from(_: SendError<T>) -> Self {
    Self
  }
}

#[repr(transparent)]
struct SendMutex(*const parking_lot::RawMutex);
impl Drop for SendMutex {
  fn drop(&mut self) {
    // SAFETY: We know this was locked by the sender
    unsafe {
      (*self.0).unlock();
    }
  }
}

// SAFETY: This is a mutex, so it's safe to send a pointer to it
unsafe impl Send for SendMutex {}

/// Create a [`TestEventSenderFactory`] and [`TestEventReceiver`] pair. The [`TestEventSenderFactory`] may be
/// used to create [`TestEventSender`]s and stdio streams for multiple workers in the system. The [`TestEventReceiver`]
/// will be kept alive until the final [`TestEventSender`] is dropped.
pub fn create_test_event_channel() -> (TestEventSenderFactory, TestEventReceiver)
{
  let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
  (
    TestEventSenderFactory {
      sender,
      worker_id: Default::default(),
    },
    TestEventReceiver { receiver },
  )
}

/// Create a [`TestEventWorkerSender`] and [`TestEventReceiver`] pair.The [`TestEventReceiver`]
/// will be kept alive until the [`TestEventSender`] is dropped.
pub fn create_single_test_event_channel(
) -> (TestEventWorkerSender, TestEventReceiver) {
  let (factory, receiver) = create_test_event_channel();
  (factory.worker(), receiver)
}

/// Polls for the next [`TestEvent`] from any worker. Events from multiple worker
/// streams may be interleaved.
pub struct TestEventReceiver {
  receiver: UnboundedReceiver<(usize, TestEvent)>,
}

impl TestEventReceiver {
  /// Receive a single test event, or `None` if no workers are alive.
  pub async fn recv(&mut self) -> Option<(usize, TestEvent)> {
    self.receiver.recv().await
  }
}

struct TestStream {
  id: usize,
  read_opt: Option<AsyncPipeRead>,
  sender: UnboundedSender<(usize, TestEvent)>,
}

impl TestStream {
  fn new(
    id: usize,
    pipe_reader: PipeRead,
    sender: UnboundedSender<(usize, TestEvent)>,
  ) -> std::io::Result<Self> {
    // This may fail if the tokio runtime is shutting down
    let read_opt = Some(pipe_reader.into_async()?);
    Ok(Self {
      id,
      read_opt,
      sender,
    })
  }

  /// Send a buffer to the test event channel. If the channel no longer exists, shut down the stream
  /// because we can't do anything.
  #[must_use = "If this returns false, don't keep reading because we cannot send"]
  fn send(&mut self, buffer: Vec<u8>) -> bool {
    if buffer.is_empty() {
      true
    } else if self
      .sender
      .send((self.id, TestEvent::Output(buffer)))
      .is_err()
    {
      self.read_opt.take();
      false
    } else {
      true
    }
  }

  fn is_alive(&self) -> bool {
    self.read_opt.is_some()
  }

  /// Cancellation-safe.
  #[inline]
  fn pipe(&mut self) -> impl Future<Output = ()> + '_ {
    poll_fn(|cx| self.poll_pipe(cx))
  }

  /// Attempt to read from a given stream, pushing all of the data in it into the given
  /// [`UnboundedSender`] before returning.
  fn poll_pipe(&mut self, cx: &mut std::task::Context) -> Poll<()> {
    let mut buffer = [0_u8; BUFFER_SIZE];
    let mut buf = ReadBuf::new(&mut buffer);
    let res = {
      // No more stream, we shouldn't hit this case.
      let Some(stream) = &mut self.read_opt else {
        unreachable!();
      };
      ready!(Pin::new(&mut *stream).poll_read(cx, &mut buf))
    };
    match res {
      Ok(_) => {
        let buf = buf.filled().to_vec();
        if buf.is_empty() {
          // The buffer may return empty in EOF conditions and never return an error,
          // so we need to treat this as EOF
          self.read_opt.take();
        } else {
          // Attempt to send the buffer, marking as not alive if the channel is closed
          _ = self.send(buf);
        }
      }
      Err(_) => {
        // Stream errored, so just return and mark this stream as not alive.
        _ = self.send(buf.filled().to_vec());
        self.read_opt.take();
      }
    }
    Poll::Ready(())
  }

  /// Read and "block" until the sync markers have been read.
  async fn read_until_sync_marker(&mut self) {
    let Some(file) = &mut self.read_opt else {
      return;
    };
    let mut flush = Vec::with_capacity(BUFFER_SIZE);
    loop {
      let mut buffer = [0_u8; BUFFER_SIZE];
      match file.read(&mut buffer).await {
        Err(_) | Ok(0) => {
          // EOF or error, just return. We make no guarantees about unflushed data at shutdown.
          self.read_opt.take();
          return;
        }
        Ok(read) => {
          flush.extend(&buffer[0..read]);

          // "ends_with" is cheaper, so check that first
          if flush.ends_with(HALF_SYNC_MARKER) {
            // We might have read the full sync marker.
            if flush.ends_with(SYNC_MARKER) {
              flush.truncate(flush.len() - SYNC_MARKER.len());
            } else {
              flush.truncate(flush.len() - HALF_SYNC_MARKER.len());
            }
            // Try to send our flushed buffer. If the channel is closed, this stream will
            // be marked as not alive.
            _ = self.send(flush);
            return;
          }

          // If we don't end with the marker, then we need to search the bytes we read plus four bytes
          // from before. There's still a possibility that the marker could be split because of a pipe
          // buffer that fills up, forcing the flush to be written across two writes and interleaving
          // data between, but that's a risk we take with this sync marker approach.
          let start =
            (flush.len() - read).saturating_sub(HALF_SYNC_MARKER.len());
          if let Some(offset) =
            memchr::memmem::find(&flush[start..], HALF_SYNC_MARKER)
          {
            flush.truncate(offset);
            // Try to send our flushed buffer. If the channel is closed, this stream will
            // be marked as not alive.
            _ = self.send(flush);
            return;
          }
        }
      }
    }
  }
}

/// A factory for creating [`TestEventSender`]s. This factory must be dropped
/// before the [`TestEventReceiver`] will complete.
pub struct TestEventSenderFactory {
  sender: UnboundedSender<(usize, TestEvent)>,
  worker_id: AtomicUsize,
}

impl TestEventSenderFactory {
  /// Create a [`TestEventWorkerSender`], along with a stdout/stderr stream.
  pub fn worker(&self) -> TestEventWorkerSender {
    let id = self.worker_id.fetch_add(1, Ordering::AcqRel);
    let (stdout_reader, stdout_writer) = pipe().unwrap();
    let (stderr_reader, stderr_writer) = pipe().unwrap();
    let (sync_sender, mut sync_receiver) =
      tokio::sync::mpsc::unbounded_channel::<(SendMutex, SendMutex)>();
    let stdout = stdout_writer.try_clone().unwrap();
    let stderr = stderr_writer.try_clone().unwrap();
    let sender = self.sender.clone();

    // Each worker spawns its own output monitoring and serialization task. This task will
    // poll the stdout/stderr streams and interleave that data with `TestEvents` generated
    // by the test runner worker.
    //
    // Note that this _must_ be a separate thread! Flushing requires locking coördination
    // on two threads and if we're blocking-locked on the mutex we've sent down the sync_receiver,
    // there's no way for us to process the actual flush operation here.
    //
    // Creating a mini-runtime to flush the stdout/stderr is the easiest way to do this, but
    // there's no reason we couldn't do it with non-blocking I/O, other than the difficulty
    // of setting up an I/O reactor in Windows.
    std::thread::spawn(move || {
      let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();
      runtime.block_on(tokio::task::unconstrained(async move {
        let mut test_stdout =
          TestStream::new(id, stdout_reader, sender.clone())?;
        let mut test_stderr = TestStream::new(id, stderr_reader, sender)?;

        // This ensures that the stdout and stderr streams in the select! loop below cannot starve each
        // other.
        let mut alternate_stream_priority = false;

        // This function will be woken whenever a stream or the receiver is ready
        loop {
          alternate_stream_priority = !alternate_stream_priority;
          let (a, b) = if alternate_stream_priority {
            (&mut test_stdout, &mut test_stderr)
          } else {
            (&mut test_stderr, &mut test_stdout)
          };

          tokio::select! {
            biased; // We actually want to poll the channel first
            recv = sync_receiver.recv() => {
              match recv {
                // If the channel closed, we assume that all important data from the streams was synced,
                // so we just end this task immediately.
                None => { break },
                Some((mutex1, mutex2)) => {
                  // Two phase lock: mutex1 indicates that we are done our general read phase and are ready for
                  // the sync phase. mutex2 indicates that we have completed the sync phase. This prevents deadlock
                  // when the pipe is too full to accept the sync marker.
                  drop(mutex1);
                  for stream in [&mut test_stdout, &mut test_stderr] {
                    if stream.is_alive() {
                      stream.read_until_sync_marker().await;
                    }
                  }
                  drop(mutex2);
                }
              }
            }
            // Poll stdout first if `alternate_stream_priority` is true, otherwise poll stderr first.
            // This is necessary because of the `biased` flag above to avoid starvation.
            _ = a.pipe(), if a.is_alive() => {},
            _ = b.pipe(), if b.is_alive() => {},
          }
        }

        Ok::<_, std::io::Error>(())
      }))?;

      Ok::<_, std::io::Error>(())
    });

    let sender = TestEventSender {
      id,
      sender: self.sender.clone(),
      sync_sender,
      stdout_writer,
      stderr_writer,
    };

    TestEventWorkerSender {
      sender,
      stdout,
      stderr,
    }
  }

  /// A [`TestEventWeakSender`] has a unique ID, but will not keep the [`TestEventReceiver`] alive.
  /// This may be useful to add a `SIGINT` or other break handler to tests that isn't part of a
  /// specific test, but handles the overall orchestration of running tests:
  ///
  /// ```nocompile
  /// let mut cancel_sender = test_event_sender_factory.weak_sender();
  /// let sigint_handler_handle = spawn(async move {
  ///   signal::ctrl_c().await.unwrap();
  ///   cancel_sender.send(TestEvent::Sigint).ok();
  /// });
  /// ```
  pub fn weak_sender(&self) -> TestEventWeakSender {
    TestEventWeakSender {
      id: self.worker_id.fetch_add(1, Ordering::AcqRel),
      sender: self.sender.downgrade(),
    }
  }
}

pub struct TestEventWeakSender {
  pub id: usize,
  sender: WeakUnboundedSender<(usize, TestEvent)>,
}

impl TestEventWeakSender {
  pub fn send(&mut self, message: TestEvent) -> Result<(), ChannelClosedError> {
    Ok(
      self
        .sender
        .upgrade()
        .ok_or(ChannelClosedError)?
        .send((self.id, message))?,
    )
  }
}

pub struct TestEventWorkerSender {
  pub sender: TestEventSender,
  pub stdout: PipeWrite,
  pub stderr: PipeWrite,
}

/// Sends messages from a given worker into the test stream. If multiple clones of
/// this sender are kept alive, the worker is kept alive.
///
/// Any unflushed bytes in the stdout or stderr stream associated with this sender
/// are not guaranteed to be sent on drop unless flush is explicitly called.
pub struct TestEventSender {
  pub id: usize,
  sender: UnboundedSender<(usize, TestEvent)>,
  sync_sender: UnboundedSender<(SendMutex, SendMutex)>,
  stdout_writer: PipeWrite,
  stderr_writer: PipeWrite,
}

impl TestEventSender {
  pub fn send(&mut self, message: TestEvent) -> Result<(), ChannelClosedError> {
    // Certain messages require us to ensure that all output has been drained to ensure proper
    // interleaving of messages.
    if message.requires_stdio_sync() {
      self.flush()?;
    }
    Ok(self.sender.send((self.id, message))?)
  }

  /// Ensure that all output has been fully flushed by writing a sync marker into the
  /// stdout and stderr streams and waiting for it on the other side.
  pub fn flush(&mut self) -> Result<(), ChannelClosedError> {
    // Two phase lock: mutex1 indicates that we are done our general read phase and are ready for
    // the sync phase. mutex2 indicates that we have completed the sync phase. This prevents deadlock
    // when the pipe is too full to accept the sync marker.
    let mutex1 = parking_lot::RawMutex::INIT;
    mutex1.lock();
    let mutex2 = parking_lot::RawMutex::INIT;
    mutex2.lock();
    self
      .sync_sender
      .send((SendMutex(&mutex1 as _), SendMutex(&mutex2 as _)))?;
    if !mutex1.try_lock_for(Duration::from_secs(30)) {
      panic!(
        "Test flush deadlock 1, sender closed = {}",
        self.sync_sender.is_closed()
      );
    }
    _ = self.stdout_writer.write_all(SYNC_MARKER);
    _ = self.stderr_writer.write_all(SYNC_MARKER);
    if !mutex2.try_lock_for(Duration::from_secs(30)) {
      panic!(
        "Test flush deadlock 2, sender closed = {}",
        self.sync_sender.is_closed()
      );
    }
    Ok(())
  }
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
#[cfg(test)]
mod tests {
  use deno_core::unsync::spawn;
  use deno_core::unsync::spawn_blocking;

  use super::*;
  use crate::tools::test::TestResult;

  /// Test that output is correctly interleaved with messages.
  #[tokio::test]
  async fn spawn_worker() {
    test_util::timeout!(60);
    let (mut worker, mut receiver) = create_single_test_event_channel();

    let recv_handle = spawn(async move {
      let mut queue = vec![];
      while let Some((_, message)) = receiver.recv().await {
        let msg_str = format!("{message:?}");
        if msg_str.len() > 50 {
          eprintln!("message = {}...", &msg_str[..50]);
        } else {
          eprintln!("message = {}", msg_str);
        }
        queue.push(message);
      }
      eprintln!("done");
      queue
    });
    let send_handle = spawn_blocking(move || {
      worker.stdout.write_all(&[1; 100_000]).unwrap();
      eprintln!("Wrote bytes");
      worker.sender.send(TestEvent::StepWait(1)).unwrap();
      eprintln!("Sent");
      worker.stdout.write_all(&[2; 100_000]).unwrap();
      eprintln!("Wrote bytes 2");
      worker.sender.flush().unwrap();
      eprintln!("Done");
    });
    send_handle.await.unwrap();
    let messages = recv_handle.await.unwrap();

    let mut expected = 1;
    let mut count = 0;
    for message in messages {
      match message {
        TestEvent::Output(vec) => {
          assert_eq!(vec[0], expected);
          count += vec.len();
        }
        TestEvent::StepWait(_) => {
          assert_eq!(count, 100_000);
          count = 0;
          expected = 2;
        }
        _ => unreachable!(),
      }
    }
    assert_eq!(expected, 2);
    assert_eq!(count, 100_000);
  }

  /// Test that flushing a large number of times doesn't hang.
  #[tokio::test]
  async fn test_flush_lots() {
    test_util::timeout!(240);
    let (mut worker, mut receiver) = create_single_test_event_channel();
    let recv_handle = spawn(async move {
      let mut queue = vec![];
      while let Some((_, message)) = receiver.recv().await {
        assert!(!matches!(message, TestEvent::Output(..)));
        queue.push(message);
      }
      eprintln!("Receiver closed");
      queue
    });
    let send_handle = spawn_blocking(move || {
      for _ in 0..100000 {
        worker.sender.send(TestEvent::StepWait(1)).unwrap();
      }
      eprintln!("Sent all messages");
    });
    send_handle.await.unwrap();
    let messages = recv_handle.await.unwrap();
    assert_eq!(messages.len(), 100000);
  }

  /// Test that flushing a large number of times doesn't hang.
  #[tokio::test]
  async fn test_flush_large() {
    test_util::timeout!(240);
    let (mut worker, mut receiver) = create_single_test_event_channel();
    let recv_handle = spawn(async move {
      let mut queue = vec![];
      while let Some((_, message)) = receiver.recv().await {
        if let TestEvent::StepWait(..) = message {
          queue.push(());
        }
      }
      eprintln!("Receiver closed");
      queue
    });
    let send_handle = spawn_blocking(move || {
      for _ in 0..25000 {
        // Write one pipe buffer's worth of message here. We try a few different sizes of potentially
        // blocking writes.
        worker.stderr.write_all(&[0; 4 * 1024]).unwrap();
        worker.sender.send(TestEvent::StepWait(1)).unwrap();
        worker.stderr.write_all(&[0; 16 * 1024]).unwrap();
        worker.sender.send(TestEvent::StepWait(1)).unwrap();
        worker.stderr.write_all(&[0; 64 * 1024]).unwrap();
        worker.sender.send(TestEvent::StepWait(1)).unwrap();
        worker.stderr.write_all(&[0; 128 * 1024]).unwrap();
        worker.sender.send(TestEvent::StepWait(1)).unwrap();
      }
      eprintln!("Sent all messages");
    });
    send_handle.await.unwrap();
    let messages = recv_handle.await.unwrap();
    assert_eq!(messages.len(), 100000);
  }

  /// Test that flushing a large number of times doesn't hang.
  #[tokio::test]
  async fn test_flush_with_close() {
    test_util::timeout!(240);
    let (worker, mut receiver) = create_single_test_event_channel();
    let TestEventWorkerSender {
      mut sender,
      stderr,
      stdout,
    } = worker;
    let recv_handle = spawn(async move {
      let mut queue = vec![];
      while let Some((_, _)) = receiver.recv().await {
        queue.push(());
      }
      eprintln!("Receiver closed");
      queue
    });
    let send_handle = spawn_blocking(move || {
      let mut stdout = Some(stdout);
      let mut stderr = Some(stderr);
      for i in 0..100000 {
        if i == 20000 {
          stdout.take();
        }
        if i == 40000 {
          stderr.take();
        }
        if i % 2 == 0 {
          if let Some(stdout) = &mut stdout {
            stdout.write_all(b"message").unwrap();
          }
        } else if let Some(stderr) = &mut stderr {
          stderr.write_all(b"message").unwrap();
        }
        sender.send(TestEvent::StepWait(1)).unwrap();
      }
      eprintln!("Sent all messages");
    });
    send_handle.await.unwrap();
    let messages = recv_handle.await.unwrap();
    assert_eq!(messages.len(), 130000);
  }

  /// Test that large numbers of interleaved steps are routed properly.
  #[tokio::test]
  async fn test_interleave() {
    test_util::timeout!(60);
    const MESSAGE_COUNT: usize = 10_000;
    let (mut worker, mut receiver) = create_single_test_event_channel();
    let recv_handle = spawn(async move {
      let mut i = 0;
      while let Some((_, message)) = receiver.recv().await {
        if i % 2 == 0 {
          let expected_text = format!("{:08x}", i / 2).into_bytes();
          let TestEvent::Output(text) = message else {
            panic!("Incorrect message: {message:?}");
          };
          assert_eq!(text, expected_text);
        } else {
          let TestEvent::Result(index, TestResult::Ok, 0) = message else {
            panic!("Incorrect message: {message:?}");
          };
          assert_eq!(index, i / 2);
        }
        i += 1;
      }
      eprintln!("Receiver closed");
      i
    });
    let send_handle: deno_core::unsync::JoinHandle<()> =
      spawn_blocking(move || {
        for i in 0..MESSAGE_COUNT {
          worker
            .stderr
            .write_all(format!("{i:08x}").as_str().as_bytes())
            .unwrap();
          worker
            .sender
            .send(TestEvent::Result(i, TestResult::Ok, 0))
            .unwrap();
        }
        eprintln!("Sent all messages");
      });
    send_handle.await.unwrap();
    let messages = recv_handle.await.unwrap();
    assert_eq!(messages, MESSAGE_COUNT * 2);
  }

  #[tokio::test]
  async fn test_sender_shutdown_before_receive() {
    test_util::timeout!(60);
    for _ in 0..10 {
      let (mut worker, mut receiver) = create_single_test_event_channel();
      worker.stderr.write_all(b"hello").unwrap();
      worker
        .sender
        .send(TestEvent::Result(0, TestResult::Ok, 0))
        .unwrap();
      drop(worker);
      let (_, message) = receiver.recv().await.unwrap();
      let TestEvent::Output(text) = message else {
        panic!("Incorrect message: {message:?}");
      };
      assert_eq!(text.as_slice(), b"hello");
      let (_, message) = receiver.recv().await.unwrap();
      let TestEvent::Result(..) = message else {
        panic!("Incorrect message: {message:?}");
      };
      assert!(receiver.recv().await.is_none());
    }
  }

  /// Ensure nothing panics if we're racing the runtime shutdown.
  #[test]
  fn test_runtime_shutdown() {
    test_util::timeout!(60);
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap();
    runtime.block_on(async {
      let (mut worker, mut receiver) = create_single_test_event_channel();
      tokio::task::spawn(async move {
        loop {
          if receiver.recv().await.is_none() {
            break;
          }
        }
      });
      tokio::task::spawn(async move {
        _ = worker.sender.send(TestEvent::Sigint);
      });
    });
  }
}
