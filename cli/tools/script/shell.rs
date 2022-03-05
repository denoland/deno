use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::fs_util;

use super::shell_parser::Command;
use super::shell_parser::Sequence;
use super::shell_parser::SequentialList;
use super::shell_types::EnvChange;
use super::shell_types::EnvState;
use super::shell_types::ExecuteResult;
use super::shell_types::ExecutedSequence;
use super::shell_types::ShellPipe;
use super::shell_types::ShellPipeSender;

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
        execute_top_sequence(item.sequence, state).await
      }));
    } else {
      match execute_top_sequence(item.sequence, state.clone()).await {
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

async fn execute_top_sequence(
  sequence: Sequence,
  state: EnvState,
) -> ExecuteResult {
  let command = execute_sequence(sequence, state, ShellPipe::InheritStdin);
  // todo: something better at outputting to stdout?
  let output_task = tokio::task::spawn(async move {
    command.stdout.pipe_to_stdout().await;
  });
  let result = command.task.await;
  output_task.await.unwrap();
  result
}

// todo(THIS PR): clean up this function
fn execute_sequence(
  sequence: Sequence,
  mut state: EnvState,
  stdin: ShellPipe,
) -> ExecutedSequence {
  match sequence {
    Sequence::EnvVar(var) => ExecutedSequence::from_result(
      ExecuteResult::Continue(0, vec![EnvChange::SetEnvVar(var)]),
    ),
    Sequence::Command(command) => start_command(&command, &state, stdin),
    Sequence::BooleanList(list) => {
      let (stdout_tx, stdout) = ShellPipe::channel();
      ExecutedSequence {
        stdout,
        task: async move {
          // todo(THIS PR): clean this up
          let mut changes = vec![];
          let first_result = execute_and_wait_sequence(
            list.current,
            state.clone(),
            stdin,
            stdout_tx.clone(),
          )
          .await;
          let exit_code = match first_result {
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
            let next_result = execute_and_wait_sequence(
              next,
              state.clone(),
              // seems suspect, but good enough for now
              ShellPipe::InheritStdin,
              stdout_tx.clone(),
            )
            .await;
            match next_result {
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
        .boxed(),
      }
    }
    Sequence::Pipeline(pipeline) => {
      let (stdout_tx, stdout) = ShellPipe::channel();
      ExecutedSequence {
        stdout,
        task: async move {
          let sequences = pipeline.into_vec();
          let mut wait_tasks = vec![];
          let mut last_input = Some(stdin);
          for sequence in sequences.into_iter() {
            let executed_sequence = execute_sequence(
              sequence,
              state.clone(),
              last_input.take().unwrap(),
            );
            last_input = Some(executed_sequence.stdout);
            wait_tasks.push(executed_sequence.task);
          }
          // todo: something better
          let output_task = tokio::task::spawn({
            async move {
              last_input.unwrap().pipe_to_sender(stdout_tx).await;
            }
          });
          let mut results = futures::future::join_all(wait_tasks).await;
          output_task.await.unwrap();
          let last_result = results.pop().unwrap();
          match last_result {
            ExecuteResult::Exit => ExecuteResult::Continue(1, Vec::new()),
            ExecuteResult::Continue(exit_code, _) => {
              ExecuteResult::Continue(exit_code, Vec::new())
            }
          }
        }
        .boxed(),
      }
    }
  }
}

async fn execute_and_wait_sequence(
  sequence: Sequence,
  state: EnvState,
  stdin: ShellPipe,
  sender: ShellPipeSender,
) -> ExecuteResult {
  let command = execute_sequence(sequence, state, stdin);
  // todo: something better
  let output_task = tokio::task::spawn({
    async move {
      command.stdout.pipe_to_sender(sender).await;
    }
  });
  let result = command.task.await;
  output_task.await.unwrap();
  result
}

fn start_command(
  command: &Command,
  state: &EnvState,
  stdin: ShellPipe,
) -> ExecutedSequence {
  if command.args[0] == "cd" {
    let args = command.args.clone();
    let cwd = state.cwd.clone();
    let (tx, stdout) = ShellPipe::channel();
    ExecutedSequence {
      stdout,
      task: async move {
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
    }
  } else if command.args[0] == "exit" {
    let args = command.args.clone();
    let (tx, stdout) = ShellPipe::channel();
    ExecutedSequence {
      stdout,
      task: async move {
        drop(tx); // close stdout
        if args.len() != 1 {
          eprintln!("exit had too many arguments.");
          ExecuteResult::Continue(1, Vec::new())
        } else {
          ExecuteResult::Exit
        }
      }
      .boxed(),
    }
  } else if command.args[0] == "pwd" {
    // ignores additional arguments
    ExecutedSequence::with_stdout_text(format!("{}\n", state.cwd.display()))
  } else if command.args[0] == "echo" {
    ExecutedSequence::with_stdout_text(format!(
      "{}\n",
      command.args[1..].join(" ")
    ))
  } else if command.args[0] == "true" {
    // ignores additional arguments
    ExecutedSequence::from_exit_code(0)
  } else if command.args[0] == "false" {
    // ignores additional arguments
    ExecutedSequence::from_exit_code(1)
  } else if command.args[0] == "sleep" {
    let args = command.args.clone();
    let (tx, stdout) = ShellPipe::channel();
    ExecutedSequence {
      stdout,
      task: async move {
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
    }
  } else {
    let mut state = state.clone();
    for env_var in &command.env_vars {
      state.apply_env_var(env_var);
    }
    let mut sub_command = tokio::process::Command::new(&command.args[0]);
    let child = sub_command
      .args(&command.args[1..])
      .envs(&state.env_vars)
      .stdout(Stdio::piped())
      .stdin(match &stdin {
        ShellPipe::InheritStdin => Stdio::inherit(),
        ShellPipe::Channel(_) => Stdio::piped(),
      })
      .stderr(Stdio::inherit())
      .current_dir(state.cwd)
      .spawn();

    let mut child = match child {
      Ok(child) => child,
      Err(err) => {
        eprintln!("Error launching '{}': {}", &command.args[0], err);
        return ExecutedSequence::from_result(ExecuteResult::Continue(
          1,
          Vec::new(),
        ));
      }
    };

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
    let (stdout_tx, stdout) = ShellPipe::channel();

    ExecutedSequence {
      stdout,
      task: async move {
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
                if stdout_tx.send(buffer[..size].to_vec()).is_err() {
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
    }
  }
}
