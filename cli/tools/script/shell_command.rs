use std::process::Stdio;
use std::time::Duration;

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::fs_util;

use super::shell::EnvChange;
use super::shell::EnvState;
use super::shell::ExecuteResult;
use super::shell_parser::Command;
use super::shell_pipe::ShellPipe;

pub struct ShellCommand {
  pub stdout: ShellPipe,
  pub wait: BoxFuture<'static, ExecuteResult>,
}

impl ShellCommand {
  fn from_exit_code(stdin: ShellPipe, exit_code: i32) -> Self {
    let (tx, stdout) = ShellPipe::channel();
    Self {
      stdout,
      wait: async move {
        stdin.drain().await;
        drop(tx); // close stdout
        ExecuteResult::Continue(exit_code, Vec::new())
      }
      .boxed(),
    }
  }

  fn with_stdout_text(stdin: ShellPipe, text: String) -> Self {
    let (mut tx, stdout) = ShellPipe::channel();
    Self {
      stdout,
      wait: async move {
        stdin.drain().await;
        let _ = tx.send(text.into_bytes()).await;
        drop(tx); // close stdout
        ExecuteResult::Continue(0, Vec::new())
      }
      .boxed(),
    }
  }
}

pub fn get_spawnable_command(
  command: &Command,
  state: &EnvState,
  stdin: ShellPipe,
) -> Result<ShellCommand, AnyError> {
  if command.args[0] == "cd" {
    let args = command.args.clone();
    let cwd = state.cwd.clone();
    let (tx, stdout) = ShellPipe::channel();
    Ok(ShellCommand {
      stdout,
      wait: async move {
        stdin.drain().await;
        drop(tx); // close stdout
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
    let (tx, stdout) = ShellPipe::channel();
    Ok(ShellCommand {
      stdout,
      wait: async move {
        stdin.drain().await;
        drop(tx); // close stdout
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
    Ok(ShellCommand::with_stdout_text(
      stdin,
      format!("{}\n", state.cwd.display()),
    ))
  } else if command.args[0] == "echo" {
    Ok(ShellCommand::with_stdout_text(
      stdin,
      format!("{}\n", command.args[1..].join(" ")),
    ))
  } else if command.args[0] == "true" {
    // ignores additional arguments
    Ok(ShellCommand::from_exit_code(stdin, 0))
  } else if command.args[0] == "false" {
    // ignores additional arguments
    Ok(ShellCommand::from_exit_code(stdin, 1))
  } else if command.args[0] == "sleep" {
    let args = command.args.clone();
    let (tx, stdout) = ShellPipe::channel();
    Ok(ShellCommand {
      stdout,
      wait: async move {
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
        drop(tx); // close stdout
        ExecuteResult::Continue(0, Vec::new())
      }
      .boxed(),
    })
  } else {
    let mut state = state.clone();
    for env_var in &command.env_vars {
      state.apply_env_var(env_var);
    }
    let mut sub_command = tokio::process::Command::new(&command.args[0]);
    let mut child = sub_command
      .args(&command.args[1..])
      .envs(&state.env_vars)
      .stdout(Stdio::piped())
      .stdin(match &stdin {
        ShellPipe::InheritStdin => Stdio::inherit(),
        ShellPipe::Channel(_) => Stdio::piped(),
      })
      .stderr(Stdio::inherit())
      .current_dir(state.cwd)
      .spawn()?;

    if let ShellPipe::Channel(mut channel) = stdin {
      // spawn a task to pipe the messages from the provided
      // channel to this process' stdin
      let mut child_stdin = child.stdin.take().unwrap();
      tokio::task::spawn(async move {
        while let Some(message) = channel.recv().await {
          if child_stdin.write_all(&message).await.is_err() {
            return;
          }
        }
      });
    }

    let mut child_stdout = child.stdout.take().unwrap();
    let (mut stdout_tx, stdout) = ShellPipe::channel();

    Ok(ShellCommand {
      stdout,
      wait: async move {
        let (process_exit_tx, mut process_exit_rx) =
          tokio::sync::mpsc::channel(1);
        // spawn a task to pipe the messages from the process' stdout to the channel
        tokio::task::spawn(async move {
          let mut buffer = [0; 512]; // todo: what is an appropriate buffer size?
          loop {
            tokio::select! {
              _ = process_exit_rx.recv() => {
                drop(stdout_tx); // close stdout
                break;
              }
              size = child_stdout.read(&mut buffer) => {
                let size = match size {
                  Ok(size) => size,
                  Err(_) => break,
                };
                if stdout_tx.send(buffer[..size].to_vec()).await.is_err() {
                  break;
                }
              }
            }
          }
        });

        let result = match child.wait().await {
          Ok(status) => {
            // TODO(THIS PR): Is unwrapping to 1 ok here?
            ExecuteResult::Continue(status.code().unwrap_or(1), Vec::new())
          }
          Err(err) => {
            eprintln!("{}", err);
            ExecuteResult::Continue(1, Vec::new())
          }
        };
        // signal to the stdout reader that it's complete
        let _ = process_exit_tx.send(()).await;
        result
      }
      .boxed(),
    })
  }
}
