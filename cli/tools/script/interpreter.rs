use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;

use crate::fs_util;
use crate::tools::script::shell_parser::SequentialItem;

use super::shell_parser::EnvVar;
use super::shell_parser::ListOperator;
use super::shell_parser::SequentialList;
use super::shell_parser::ShellCommand;
use super::shell_parser::ShellCommandBody;

#[derive(Clone)]
struct EnvState {
  pub env_vars: HashMap<String, String>,
  pub cwd: PathBuf,
}

impl EnvState {
  pub fn apply_env_var(&mut self, var: &EnvVar) {
    if var.value.is_empty() {
      self.env_vars.remove(&var.name);
    } else {
      self.env_vars.insert(var.name.clone(), var.value.clone());
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

  pub fn apply_changes(&mut self, changes: &[EnvChange]) {
    for change in changes {
      self.apply_change(change);
    }
  }
}

enum EnvChange {
  SetEnvVar(EnvVar),
  Cd(PathBuf),
}

pub async fn execute(
  list: SequentialList,
  env_vars: HashMap<String, String>,
  cwd: PathBuf,
  additional_cli_args: Vec<String>,
) -> Result<i32, AnyError> {
  assert!(cwd.is_absolute());
  let list = append_cli_args(list, additional_cli_args)?;
  let mut state = EnvState { env_vars, cwd };
  let mut async_handles = Vec::new();
  let mut final_exit_code = 0;
  for item in list.items {
    match item {
      SequentialItem::Async(item) => {
        let state = state.clone();
        async_handles.push(tokio::task::spawn(async move {
          let result = execute_command(item, state).await;
          if let ExecuteCommandResult::Continue(_, _, Some(err)) = result {
            eprintln!("{}", err);
          }
        }));
      }
      SequentialItem::Command(item) => {
        match execute_command(item, state.clone()).await {
          ExecuteCommandResult::Exit => return Ok(0),
          ExecuteCommandResult::Continue(exit_code, changes, err) => {
            state.apply_changes(&changes);
            if let Some(err) = err {
              eprintln!("{}", err);
            }
            // use the final sequential item's exit code
            final_exit_code = exit_code;
          }
        }
      }
    }
  }

  // wait for async commands to complete
  futures::future::join_all(async_handles).await;

  Ok(final_exit_code)
}

enum ExecuteCommandResult {
  Exit,
  Continue(i32, Vec<EnvChange>, Option<AnyError>),
}

fn execute_command(
  item: ShellCommand,
  mut state: EnvState,
) -> BoxFuture<'static, ExecuteCommandResult> {
  // recursive async functions require boxing
  async move {
    let mut changes = Vec::new();
    let body_result: Result<i32, AnyError> = match item.body {
      ShellCommandBody::EnvVar(var) => {
        state.apply_env_var(&var);
        changes.push(EnvChange::SetEnvVar(var));
        Ok(0)
      }
      ShellCommandBody::Command(command) => {
        if command.args[0] == "cd" {
          if command.args.len() != 2 {
            Err(anyhow!("cd is expected to have 1 argument."))
          } else {
            // affects the parent state
            let new_dir = state.cwd.join(&command.args[1]);
            match fs_util::canonicalize_path(&new_dir) {
              Ok(new_dir) => {
                state.cwd = new_dir;
                Ok(0)
              }
              Err(err) => Err(anyhow!(
                "Could not cd to {}.\n\n{}",
                new_dir.display(),
                err
              )),
            }
          }
        } else if command.args[0] == "pwd" && command.args.len() == 1 {
          println!("{}", state.cwd.display());
          Ok(0)
        } else if command.args[0] == "exit" && command.args.len() == 1 {
          return ExecuteCommandResult::Exit;
        } else if command.args[0] == "true" && command.args.len() == 1 {
          Ok(0)
        } else if command.args[0] == "false" && command.args.len() == 1 {
          Ok(1)
        } else if command.args[0] == "sleep" && command.args.len() == 2 {
          match command.args[1].parse::<f64>() {
            Ok(value_s) => {
              let ms = (value_s * 1000f64) as u64;
              tokio::time::sleep(Duration::from_millis(ms)).await;
              Ok(0)
            }
            Err(err) => {
              Err(anyhow!("Error parsing sleep argument to number: {}", err))
            }
          }
        } else if command.args[0] == "echo" {
          println!("{}", command.args[1..].join(" "));
          Ok(0)
        } else {
          let mut state = state.clone();
          for env_var in &command.env_vars {
            state.apply_env_var(env_var);
          }
          let mut sub_command = tokio::process::Command::new(&command.args[0]);
          let result = sub_command
            .args(&command.args[1..])
            .envs(&state.env_vars)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .current_dir(state.cwd)
            .status()
            .await;
          match result {
            // TODO: Is unwrapping to 1 ok here?
            Ok(status) => Ok(status.code().unwrap_or(1)),
            Err(err) => Err(anyhow!("{}", err)),
          }
        }
      }
    };

    let was_success = body_result.as_ref().map(|c| *c == 0).unwrap_or(false);
    let mut maybe_next = item.next;
    while let Some(next) = maybe_next.take() {
      if next.op == ListOperator::Or && !was_success
        || next.op == ListOperator::And && was_success
      {
        let next_command = execute_command(next.command, state.clone()).await;
        return match next_command {
          ExecuteCommandResult::Exit => ExecuteCommandResult::Exit,
          ExecuteCommandResult::Continue(exit_code, sub_changes, err) => {
            changes.extend(sub_changes);
            ExecuteCommandResult::Continue(exit_code, changes, err)
          }
        };
      }
      maybe_next = next.command.next;
    }

    match body_result {
      Ok(code) => ExecuteCommandResult::Continue(code, changes, None),
      Err(err) => ExecuteCommandResult::Continue(1, changes, Some(err)),
    }
  }
  .boxed()
}

/// When a user calls `deno task <task-name> -- <args>`, we want
/// to append those CLI arguments to the last command.
fn append_cli_args(
  mut list: SequentialList,
  args: Vec<String>,
) -> Result<SequentialList, AnyError> {
  if args.is_empty() {
    return Ok(list);
  }

  if let Some(list_item) = list.items.last_mut() {
    match list_item {
      SequentialItem::Command(cmd) => {
        // get the last item
        let mut cmd = cmd;
        while let Some(next) = cmd.next.as_mut() {
          cmd = &mut next.command;
        }

        match &mut cmd.body {
          ShellCommandBody::EnvVar(_) => {
            bail!("Cannot append CLI arguments to a command that updates environment variables.");
          }
          ShellCommandBody::Command(cmd) => {
            cmd.args.extend(args);
          }
        }
      }
      SequentialItem::Async(_) => {
        // this ended with an `&`, so error
        bail!("Cannot append CLI arguments to an async command.");
      }
    }
  }

  Ok(list)
}
