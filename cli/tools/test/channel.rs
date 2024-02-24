// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::TestEvent;
use super::TestStdioStream;
use deno_core::futures::future::poll_fn;
use deno_core::parking_lot;
use deno_core::parking_lot::lock_api::RawMutex;
use deno_runtime::deno_io::pipe;
use deno_runtime::deno_io::AsyncPipeRead;
use deno_runtime::deno_io::PipeRead;
use deno_runtime::deno_io::PipeWrite;
use std::fmt::Display;
use std::io::Write;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::ReadBuf;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::WeakUnboundedSender;

/// 8-byte sync marker that is unlikely to appear in normal output. Equivalent
/// to the string `"\u{200B}\0\u{200B}\0"`.
const SYNC_MARKER: &[u8; 8] = &[226, 128, 139, 0, 226, 128, 139, 0];

const BUFFER_SIZE: usize = 4096;

/// The test channel has been closed and cannot be used to send further messages.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
  which: TestStdioStream,
  read_opt: Option<AsyncPipeRead>,
  sender: UnboundedSender<(usize, TestEvent)>,
}

impl TestStream {
  fn new(
    id: usize,
    which: TestStdioStream,
    pipe_reader: PipeRead,
    sender: UnboundedSender<(usize, TestEvent)>,
  ) -> std::io::Result<Self> {
    // This may fail if the tokio runtime is shutting down
    let read_opt = Some(pipe_reader.into_async()?);
    Ok(Self {
      id,
      which,
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
      .send((self.id, TestEvent::Output(self.which, buffer)))
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

  /// Attempt to read from a given stream, pushing all of the data in it into the given
  /// [`UnboundedSender`] before returning.
  async fn pipe(&mut self) {
    let mut buffer = [0_u8; BUFFER_SIZE];
    let mut buf = ReadBuf::new(&mut buffer);
    let res = {
      // No more stream, so just return.
      let Some(stream) = &mut self.read_opt else {
        return;
      };
      poll_fn(|cx| Pin::new(&mut *stream).poll_read(cx, &mut buf)).await
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
          if flush.ends_with(SYNC_MARKER) {
            flush.truncate(flush.len() - SYNC_MARKER.len());
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
    let (stdout_reader, mut stdout_writer) = pipe().unwrap();
    let (stderr_reader, mut stderr_writer) = pipe().unwrap();
    let (sync_sender, mut sync_receiver) =
      tokio::sync::mpsc::unbounded_channel::<SendMutex>();
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
        let mut test_stdout = TestStream::new(
          id,
          TestStdioStream::Stdout,
          stdout_reader,
          sender.clone(),
        )?;
        let mut test_stderr =
          TestStream::new(id, TestStdioStream::Stderr, stderr_reader, sender)?;

        // This function will be woken whenever a stream or the receiver is ready
        loop {
          tokio::select! {
            _ = test_stdout.pipe(), if test_stdout.is_alive() => {},
            _ = test_stderr.pipe(), if test_stdout.is_alive() => {},
            recv = sync_receiver.recv() => {
              match recv {
                // If the channel closed, we assume that all important data from the streams was synced,
                // so we just end this task immediately.
                None => { break },
                Some(mutex) => {
                  // If we fail to write the sync marker for flush (likely in the case where the runtime is shutting down),
                  // we instead just release the mutex and bail.
                  let success = stdout_writer.write_all(SYNC_MARKER).is_ok()
                    && stderr_writer.write_all(SYNC_MARKER).is_ok();
                  if success {
                    for stream in [&mut test_stdout, &mut test_stderr] {
                      stream.read_until_sync_marker().await;
                    }
                  }
                  drop(mutex);
                }
              }
            }
          }
        }

        Ok::<_, std::io::Error>(())
      }))?;

      Ok::<_, std::io::Error>(())
    });

    let sender = TestEventSender {
      id,
      ref_count: Default::default(),
      sender: self.sender.clone(),
      sync_sender,
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
  ref_count: Arc<()>,
  sender: UnboundedSender<(usize, TestEvent)>,
  sync_sender: UnboundedSender<SendMutex>,
}

impl Clone for TestEventSender {
  fn clone(&self) -> Self {
    Self {
      id: self.id,
      ref_count: self.ref_count.clone(),
      sender: self.sender.clone(),
      sync_sender: self.sync_sender.clone(),
    }
  }
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
    let mutex = parking_lot::RawMutex::INIT;
    mutex.lock();
    self.sync_sender.send(SendMutex(&mutex as _))?;
    mutex.lock();
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::unsync::spawn;
  use deno_core::unsync::spawn_blocking;

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
        TestEvent::Output(_, vec) => {
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
    test_util::timeout!(60);
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
