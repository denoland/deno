use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;

use crate::fs_util;

use super::shell::EnvChange;
use super::shell::EnvState;
use super::shell::ExecuteResult;
use super::shell_parser::Command;

pub enum CommandPipe {
  Child(ChildStdin),
  Inherit,
  Null,
}

impl CommandPipe {
  pub async fn write_text(&mut self, text: String) -> Result<(), AnyError> {
    match self {
      CommandPipe::Child(child) => {
        child.write_all(text.as_bytes()).await?;
      }
      CommandPipe::Inherit => {
        write!(std::io::stdout(), "{}", text)?;
      }
      CommandPipe::Null => {}
    }
    Ok(())
  }
}

pub struct SpawnableCommand {
  pub stdin: CommandPipe,
  pub spawn: BoxFuture<'static, ExecuteResult>,
}

impl SpawnableCommand {
  fn from_exit_code(exit_code: i32) -> Self {
    Self {
      stdin: CommandPipe::Null,
      spawn: async move { ExecuteResult::Continue(exit_code, Vec::new()) }
        .boxed(),
    }
  }

  fn with_stdout_text(mut stdout: CommandPipe, text: String) -> Self {
    Self {
      stdin: CommandPipe::Null,
      spawn: async move {
        if let Err(err) = stdout.write_text(text).await {
          eprintln!("Error outputting to stdout: {}", err);
          ExecuteResult::Continue(1, Vec::new())
        } else {
          ExecuteResult::Continue(0, Vec::new())
        }
      }
      .boxed(),
    }
  }
}

pub fn get_spawnable_command(
  command: &Command,
  state: &EnvState,
  stdout: CommandPipe,
  take_stdin: bool,
) -> Result<SpawnableCommand, AnyError> {
  if command.args[0] == "cd" {
    let args = command.args.clone();
    let cwd = state.cwd.clone();
    Ok(SpawnableCommand {
      stdin: CommandPipe::Null,
      spawn: async move {
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
      stdin: CommandPipe::Null,
      spawn: async move {
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
      stdout,
      format!("{}\n", state.cwd.display()),
    ))
  } else if command.args[0] == "true" {
    // ignores additional arguments
    Ok(SpawnableCommand::from_exit_code(0))
  } else if command.args[0] == "false" {
    // ignores additional arguments
    Ok(SpawnableCommand::from_exit_code(1))
  } else if command.args[0] == "sleep" {
    let args = command.args.clone();
    Ok(SpawnableCommand {
      stdin: CommandPipe::Null,
      spawn: async move {
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
      stdout,
      format!("{}\n", command.args[1..].join(" ")),
    ))
  } else {
    let mut state = state.clone();
    for env_var in &command.env_vars {
      state.apply_env_var(env_var);
    }
    let mut sub_command = tokio::process::Command::new(&command.args[0]);
    let mut child = sub_command
      .args(&command.args[1..])
      .envs(&state.env_vars)
      .stdout(match stdout {
        CommandPipe::Child(child) => child.try_into().unwrap(),
        CommandPipe::Inherit => Stdio::inherit(),
        CommandPipe::Null => Stdio::null(),
      })
      .stdin(if take_stdin {
        Stdio::piped()
      } else {
        Stdio::inherit()
      })
      .stderr(Stdio::inherit())
      .current_dir(state.cwd)
      .spawn()?;
    Ok(SpawnableCommand {
      stdin: if take_stdin {
        let child_stdin = child.stdin.take().unwrap();
        CommandPipe::Child(child_stdin)
      } else {
        CommandPipe::Inherit
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
