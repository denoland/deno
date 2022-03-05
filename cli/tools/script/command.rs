use std::process::Stdio;
use std::time::Duration;

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdout;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::fs_util;

use super::shell::EnvChange;
use super::shell::EnvState;
use super::shell::ExecuteResult;
use super::shell_parser::Command;

pub enum CommandStdout {
  Child(ChildStdout),
  Channel(UnboundedReceiver<Vec<u8>>),
  Inherit,
  Null,
}

impl CommandStdout {
  /// Wait on completion and drain the output.
  pub async fn drain(self) {
    match self {
      CommandStdout::Child(mut child) => {
        let mut buffer = [0; 512]; // todo: what is an appropriate buffer size?
        while let Ok(size) = child.read(&mut buffer).await {
          // todo: this doesn't seem correct. How to detect the end?
          if size == 0 {
            break;
          }
        }
      }
      CommandStdout::Channel(mut rx) => {
        while rx.recv().await.is_some() {
          // drain, do nothing
        }
      }
      CommandStdout::Inherit | CommandStdout::Null => {}
    }
  }

  /// Write everything to the specified writer
  pub async fn write_all(
    self,
    mut writer: impl AsyncWrite + std::marker::Unpin,
  ) -> Result<(), AnyError> {
    match self {
      CommandStdout::Child(mut child) => {
        let mut buffer = [0; 512]; // todo: what is an appropriate buffer size?
        while let Ok(size) = child.read(&mut buffer).await {
          writer.write(&buffer[..size]).await?;

          // todo: this doesn't seem correct. How to detect the end?
          if size == 0 {
            break;
          }
        }
      }
      CommandStdout::Channel(mut rx) => {
        while let Some(data) = rx.recv().await {
          writer.write(&data).await?;
        }
      }
      CommandStdout::Inherit | CommandStdout::Null => {}
    }
    Ok(())
  }
}

pub struct SpawnableCommand {
  pub stdout: CommandStdout,
  pub spawn: BoxFuture<'static, ExecuteResult>,
}

impl SpawnableCommand {
  fn from_exit_code(stdin: CommandStdout, exit_code: i32) -> Self {
    Self {
      stdout: CommandStdout::Null,
      spawn: async move {
        stdin.drain().await;
        ExecuteResult::Continue(exit_code, Vec::new())
      }
      .boxed(),
    }
  }

  fn with_stdout_text(stdin: CommandStdout, text: String) -> Self {
    let (tx, rx) = unbounded_channel();
    Self {
      stdout: CommandStdout::Channel(rx),
      spawn: async move {
        stdin.drain().await;
        let result = if let Err(err) = tx.send(text.into_bytes()) {
          eprintln!("Error writing to stdout: {}", err);
          ExecuteResult::Continue(1, Vec::new())
        } else {
          ExecuteResult::Continue(0, Vec::new())
        };
        drop(tx);
        result
      }
      .boxed(),
    }
  }
}

pub fn get_spawnable_command(
  command: &Command,
  state: &EnvState,
  stdin: CommandStdout,
  take_stdout: bool,
) -> Result<SpawnableCommand, AnyError> {
  if command.args[0] == "cd" {
    let args = command.args.clone();
    let cwd = state.cwd.clone();
    Ok(SpawnableCommand {
      stdout: CommandStdout::Null,
      spawn: async move {
        stdin.drain().await;
        if args.len() != 2 {
          eprintln!("cd is expected to have 1 argument.");
          ExecuteResult::Continue(1, Vec::new())
        } else {
          // affects the parent state
          let new_dir = cwd.join(&args[1]);
          match fs_util::canonicalize_path(&new_dir) {
            Ok(new_dir) => {
              ExecuteResult::Continue(0, vec![EnvChange::Cd(new_dir)])
            }
            Err(err) => {
              eprintln!("Could not cd to {}.\n\n{}", new_dir.display(), err);
              ExecuteResult::Continue(1, Vec::new())
            }
          }
        }
      }
      .boxed(),
    })
  } else if command.args[0] == "exit" {
    let args = command.args.clone();
    Ok(SpawnableCommand {
      stdout: CommandStdout::Null,
      spawn: async move {
        stdin.drain().await;
        if args.len() != 1 {
          eprintln!("exit had too many arguments.");
          ExecuteResult::Continue(1, Vec::new())
        } else {
          ExecuteResult::Exit
        }
      }
      .boxed(),
    })
  } else if command.args[0] == "pwd" {
    // ignores additional arguments
    Ok(SpawnableCommand::with_stdout_text(
      stdin,
      format!("{}\n", state.cwd.display()),
    ))
  } else if command.args[0] == "true" {
    // ignores additional arguments
    Ok(SpawnableCommand::from_exit_code(stdin, 0))
  } else if command.args[0] == "false" {
    // ignores additional arguments
    Ok(SpawnableCommand::from_exit_code(stdin, 1))
  } else if command.args[0] == "sleep" {
    let args = command.args.clone();
    Ok(SpawnableCommand {
      stdout: CommandStdout::Null,
      spawn: async move {
        stdin.drain().await;
        // the time to sleep is the sum of all the arguments
        let mut total_time_ms = 0;
        for arg in args.iter().skip(1) {
          match arg.parse::<f64>() {
            Ok(value_s) => {
              let ms = (value_s * 1000f64) as u64;
              total_time_ms += ms;
            }
            Err(err) => {
              eprintln!("Error parsing sleep argument to number: {}", err);
              return ExecuteResult::Continue(1, Vec::new());
            }
          }
        }
        tokio::time::sleep(Duration::from_millis(total_time_ms)).await;
        ExecuteResult::Continue(0, Vec::new())
      }
      .boxed(),
    })
  } else if command.args[0] == "echo" {
    Ok(SpawnableCommand::with_stdout_text(
      stdin,
      format!("{}\n", command.args[1..].join(" ")),
    ))
  } else {
    let mut state = state.clone();
    for env_var in &command.env_vars {
      state.apply_env_var(env_var);
    }
    let mut sub_command = tokio::process::Command::new(&command.args[0]);
    let (stdin, stdin_rx) = match stdin {
      CommandStdout::Channel(rx) => (Stdio::piped(), Some(rx)),
      CommandStdout::Child(child) => (child.try_into().unwrap(), None),
      CommandStdout::Inherit => (Stdio::inherit(), None),
      CommandStdout::Null => (Stdio::null(), None),
    };
    let mut child = sub_command
      .args(&command.args[1..])
      .envs(&state.env_vars)
      .stdout(if take_stdout {
        Stdio::piped()
      } else {
        Stdio::inherit()
      })
      .stdin(stdin)
      .stderr(Stdio::inherit())
      .current_dir(state.cwd)
      .spawn()?;
    if let Some(mut channel) = stdin_rx {
      // spawn a task to pipe the messages from the provided
      // stdout to this process' stdin
      let mut child_stdin = child.stdin.take().unwrap();
      tokio::task::spawn(async move {
        while let Some(message) = channel.recv().await {
          if child_stdin.write_all(&message).await.is_err() {
            return;
          }
        }
      });
    }
    Ok(SpawnableCommand {
      stdout: if take_stdout {
        let child_stdout = child.stdout.take().unwrap();
        CommandStdout::Child(child_stdout)
      } else {
        CommandStdout::Inherit
      },
      spawn: async move {
        match child.wait().await {
          Ok(status) => {
            // TODO(THIS PR): Is unwrapping to 1 ok here?
            ExecuteResult::Continue(status.code().unwrap_or(1), Vec::new())
          }
          Err(err) => {
            eprintln!("{}", err);
            ExecuteResult::Continue(1, Vec::new())
          }
        }
      }
      .boxed(),
    })
  }
}
