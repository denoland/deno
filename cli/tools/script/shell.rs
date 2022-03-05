use std::collections::HashMap;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;

use super::command::get_spawnable_command;
use super::command::CommandPipe;
use super::shell_parser::EnvVar;
use super::shell_parser::Sequence;
use super::shell_parser::SequentialList;

#[derive(Clone)]
pub struct EnvState {
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

  fn apply_change(&mut self, change: &EnvChange) {
    match change {
      EnvChange::SetEnvVar(var) => self.apply_env_var(var),
      EnvChange::Cd(new_dir) => {
        self.cwd = new_dir.clone();
      }
    }
  }

  fn apply_changes(&mut self, changes: &[EnvChange]) {
    for change in changes {
      self.apply_change(change);
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
    if item.is_async {
      let state = state.clone();
      async_handles.push(tokio::task::spawn(async move {
        execute_sequence(item.sequence, state).await
      }));
    } else {
      match execute_sequence(item.sequence, state.clone()).await {
        ExecuteResult::Exit => return Ok(0),
        ExecuteResult::Continue(exit_code, changes) => {
          state.apply_changes(&changes);
          // use the final sequential item's exit code
          final_exit_code = exit_code;
        }
      }
    }
  }

  // wait for async commands to complete
  futures::future::join_all(async_handles).await;

  Ok(final_exit_code)
}

// todo(THIS PR): clean up this function
fn execute_sequence(
  sequence: Sequence,
  mut state: EnvState,
) -> BoxFuture<'static, ExecuteResult> {
  // recursive async functions require boxing
  async move {
    match sequence {
      Sequence::EnvVar(var) => {
        ExecuteResult::Continue(0, vec![EnvChange::SetEnvVar(var)])
      }
      Sequence::Command(command) => {
        match get_spawnable_command(
          &command,
          &state,
          CommandPipe::Inherit,
          false,
        ) {
          Ok(command) => command.spawn.await,
          Err(err) => {
            eprintln!("{}", err);
            ExecuteResult::Continue(1, Vec::new())
          }
        }
      }
      Sequence::BooleanList(list) => {
        // todo(THIS PR): clean this up
        let mut changes = vec![];
        let first_command = execute_sequence(list.current, state.clone()).await;
        let exit_code = match first_command {
          ExecuteResult::Exit => return ExecuteResult::Exit,
          ExecuteResult::Continue(exit_code, sub_changes) => {
            state.apply_changes(&sub_changes);
            changes.extend(sub_changes);
            exit_code
          }
        };
        let next = if list.op.moves_next_for_exit_code(exit_code) {
          Some(list.next)
        } else {
          let mut next = list.next;
          loop {
            // boolean lists always move right on the tree
            match next {
              Sequence::BooleanList(list) => {
                if list.op.moves_next_for_exit_code(exit_code) {
                  break Some(list.next);
                }
                next = list.next;
              }
              _ => break None,
            }
          }
        };
        if let Some(next) = next {
          match execute_sequence(next, state.clone()).await {
            ExecuteResult::Exit => ExecuteResult::Exit,
            ExecuteResult::Continue(exit_code, sub_changes) => {
              changes.extend(sub_changes);
              ExecuteResult::Continue(exit_code, changes)
            }
          }
        } else {
          ExecuteResult::Continue(exit_code, changes)
        }
      }
      Sequence::Pipeline(_) => todo!(),
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

  // todo(THIS PR): this part and remove this clippy
  #[allow(clippy::redundant_pattern_matching)]
  if let Some(_) = list.items.last_mut() {
    todo!();
  }

  Ok(list)
}
