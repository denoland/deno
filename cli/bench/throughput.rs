// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::Result;
use deno_core::serde_json::{Number, Value};
use std::{
  path::PathBuf,
  process::Command,
  time::{Duration, Instant},
};

const MB: usize = 1024 * 1024;
const SERVER_ADDR: &str = "0.0.0.0:4544";
const CLIENT_ADDR: &str = "127.0.0.1 4544";

pub(crate) fn cat(deno_exe: &PathBuf, megs: usize) -> Result<Value> {
  let size = megs * MB;
  let shell_cmd = format!(
    "{} run --allow-read cli/tests/cat.ts /dev/zero | head -c {}",
    deno_exe.to_str().unwrap(),
    size
  );
  println!("{}", shell_cmd);
  let cmd = &["sh", "-c", &shell_cmd];

  let start = Instant::now();
  let _ = test_util::run_collect(cmd, None, None, None, true);
  let end = Instant::now();

  Ok(Value::Number(
    Number::from_f64((end - start).as_secs_f64()).unwrap(),
  ))
}

pub(crate) fn tcp(deno_exe: &PathBuf, megs: usize) -> Result<Value> {
  let size = megs * MB;

  // The GNU flavor of `nc` requires the `-N` flag to shutdown the network socket after EOF on stdin
  let nc_command = if cfg!(target_os = "linux") {
    "nc -N"
  } else {
    "nc"
  };

  let shell_cmd = format!(
    "head -c {} /dev/zero | {} {}",
    size, nc_command, CLIENT_ADDR
  );
  println!("{}", shell_cmd);
  let cmd = &["sh", "-c", &shell_cmd];

  // Run deno echo server in the background.
  let mut echo_server = Command::new(deno_exe.to_str().unwrap())
    .args(&[
      "run",
      "--allow-net",
      "cli/tests/echo_server.ts",
      SERVER_ADDR,
    ])
    .spawn()?;

  std::thread::sleep(Duration::from_secs(5)); // wait for deno to wake up. TODO racy.

  let start = Instant::now();
  let _ = test_util::run_collect(cmd, None, None, None, true);
  let end = Instant::now();

  echo_server.kill()?;

  Ok(Value::Number(
    Number::from_f64((end - start).as_secs_f64()).unwrap(),
  ))
}
