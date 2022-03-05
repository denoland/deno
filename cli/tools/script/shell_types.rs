use std::collections::HashMap;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

use super::shell_parser::EnvVar;

#[derive(Clone)]
pub struct EnvState {
  pub env_vars: HashMap<String, String>,
  pub cwd: PathBuf,
}

impl EnvState {
  pub fn apply_changes(&mut self, changes: &[EnvChange]) {
    for change in changes {
      self.apply_change(change);
    }
  }

  pub fn apply_change(&mut self, change: &EnvChange) {
    match change {
      EnvChange::SetEnvVar(var) => self.apply_env_var(var),
      EnvChange::Cd(new_dir) => {
        self.cwd = new_dir.clone();
      }
    }
  }

  pub fn apply_env_var(&mut self, var: &EnvVar) {
    if var.value.is_empty() {
      self.env_vars.remove(&var.name);
    } else {
      self.env_vars.insert(var.name.clone(), var.value.clone());
    }
  }
}

pub enum EnvChange {
  SetEnvVar(EnvVar),
  Cd(PathBuf),
}

pub enum ExecuteResult {
  Exit,
  Continue(i32, Vec<EnvChange>),
}

pub struct ExecutedSequence {
  pub stdout: ShellPipe,
  pub wait: BoxFuture<'static, ExecuteResult>,
}

impl ExecutedSequence {
  pub fn from_exit_code(exit_code: i32) -> Self {
    Self::from_result(ExecuteResult::Continue(exit_code, Vec::new()))
  }

  pub fn from_result(execute_result: ExecuteResult) -> Self {
    let (tx, stdout) = ShellPipe::channel();
    Self {
      stdout,
      wait: async move {
        drop(tx); // close stdout
        execute_result
      }
      .boxed(),
    }
  }

  pub fn with_stdout_text(text: String) -> Self {
    let (tx, stdout) = ShellPipe::channel();
    Self {
      stdout,
      wait: async move {
        let _ = tx.send(text.into_bytes());
        drop(tx); // close stdout
        ExecuteResult::Continue(0, Vec::new())
      }
      .boxed(),
    }
  }
}

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

  /// Pipes this pipe to the specified sender.
  pub async fn pipe_to_sender(self, sender: ShellPipeSender) {
    match self {
      ShellPipe::InheritStdin => unreachable!(),
      ShellPipe::Channel(mut rx) => {
        while let Some(data) = rx.recv().await {
          if sender.send(data).is_err() {
            break;
          }
        }
      }
    }
  }
}
