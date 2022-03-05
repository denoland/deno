use deno_core::error::AnyError;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

/// This sender and receiver structs cause the sender to wait on the
/// receiver to signal that it's ready before sending messages across
/// the channel.
pub struct ShellPipeReceiver {
  is_ready: bool,
  ready: Sender<()>,
  receiver: Receiver<Vec<u8>>,
}

impl ShellPipeReceiver {
  pub async fn recv(&mut self) -> Option<Vec<u8>> {
    if !self.is_ready {
      self.ready.send(()).await.unwrap();
      self.is_ready = true;
    }
    self.receiver.recv().await
  }
}

pub struct ShellPipeSender {
  is_ready: bool,
  ready: Receiver<()>,
  sender: Sender<Vec<u8>>,
}

impl ShellPipeSender {
  pub async fn send(&mut self, value: Vec<u8>) -> Result<(), AnyError> {
    if !self.is_ready {
      let _ = self.ready.recv().await;
      self.is_ready = true;
    }
    self.sender.send(value).await?;
    Ok(())
  }
}

/// Used to communicate between commands.
pub enum ShellPipe {
  /// Pull messages from stdin.
  InheritStdin,
  /// Receives pushed messages from a channel.
  Channel(ShellPipeReceiver),
}

impl ShellPipe {
  pub fn channel() -> (ShellPipeSender, ShellPipe) {
    let (ready_tx, ready_rx) = tokio::sync::mpsc::channel(1);
    let (data_tx, data_rx) = tokio::sync::mpsc::channel(1);
    (
      ShellPipeSender {
        is_ready: false,
        ready: ready_rx,
        sender: data_tx,
      },
      ShellPipe::Channel(ShellPipeReceiver {
        is_ready: false,
        ready: ready_tx,
        receiver: data_rx,
      }),
    )
  }

  /// Wait on completion and drain the output.
  pub async fn drain(self) {
    match self {
      ShellPipe::InheritStdin => {
        // don't need to drain stdin because it is pull only
      }
      ShellPipe::Channel(mut rx) => {
        while rx.recv().await.is_some() {
          // drain, do nothing
        }
      }
    }
  }

  /// Write everything to the specified writer
  pub async fn write_all(
    self,
    mut writer: impl AsyncWrite + std::marker::Unpin,
  ) -> Result<(), AnyError> {
    match self {
      ShellPipe::InheritStdin => unreachable!(),
      ShellPipe::Channel(mut rx) => {
        while let Some(data) = rx.recv().await {
          writer.write(&data).await?;
        }
      }
    }
    Ok(())
  }

  /// Pipes this pipe to the current process' stdout.
  pub async fn pipe_to_stdout(self) {
    let _ = self.write_all(tokio::io::stdout()).await;
  }
}
