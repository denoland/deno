use deno_core::error::AnyError;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

pub type ShellPipeReceiver = UnboundedReceiver<Vec<u8>>;
pub type ShellPipeSender = UnboundedSender<Vec<u8>>;

/// Used to communicate between commands.
pub enum ShellPipe {
  /// Pull messages from stdin.
  InheritStdin,
  /// Receives pushed messages from a channel.
  Channel(ShellPipeReceiver),
}

impl ShellPipe {
  pub fn channel() -> (ShellPipeSender, ShellPipe) {
    let (data_tx, data_rx) = tokio::sync::mpsc::unbounded_channel();
    (data_tx, ShellPipe::Channel(data_rx))
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
