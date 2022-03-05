use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;

use super::shell::EnvState;
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
  pub spawn: BoxFuture<'static, Result<i32, AnyError>>,
}

impl SpawnableCommand {
  fn from_exit_code(exit_code: i32) -> Self {
    Self {
      stdin: CommandPipe::Null,
      spawn: async move { Ok(exit_code) }.boxed(),
    }
  }

  fn with_stdout_text(mut stdout: CommandPipe, text: String) -> Self {
    Self {
      stdin: CommandPipe::Null,
      spawn: async move {
        stdout.write_text(text).await?;
        Ok(0)
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
  if matches!(command.args[0].as_str(), "exit" | "cd") {
    // will only get here in a pipeline and in a pipeline these commands
    // do nothing other than returning a `0` exit code
    Ok(SpawnableCommand::from_exit_code(0))
  } else if command.args[0] == "pwd" && command.args.len() == 1 {
    Ok(SpawnableCommand::with_stdout_text(
      stdout,
      format!("{}\n", state.cwd.display()),
    ))
  } else if command.args[0] == "true" && command.args.len() == 1 {
    Ok(SpawnableCommand::from_exit_code(0))
  } else if command.args[0] == "false" && command.args.len() == 1 {
    Ok(SpawnableCommand::from_exit_code(1))
  } else if command.args[0] == "sleep" && command.args.len() == 2 {
    let sleep_time = command.args[1].clone();
    Ok(SpawnableCommand {
      stdin: CommandPipe::Null,
      spawn: async move {
        match sleep_time.parse::<f64>() {
          Ok(value_s) => {
            let ms = (value_s * 1000f64) as u64;
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Ok(0)
          }
          Err(err) => {
            Err(anyhow!("Error parsing sleep argument to number: {}", err))
          }
        }
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
        let status = child.wait().await?;
        // TODO(THIS PR): Is unwrapping to 1 ok here?
        Ok(status.code().unwrap_or(1))
      }
      .boxed(),
    })
  }
}
