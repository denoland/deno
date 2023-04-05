// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::Result;

pub fn benchmark() -> Result<HashMap<String, f64>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let ws_dir = manifest_dir.join("bench").join("websocket");
  for entry in std::fs::read_dir(&ws_dir)? {
    let port = crate::http::get_port();
    let entry = entry?;
    let pathbuf = entry.path();
    let path = pathbuf.to_str().unwrap();
    let file_stem = pathbuf.file_stem().unwrap().to_str().unwrap();

    let mut cmd = Command::new(deno_exe);
    cmd
      .arg("run")
      .arg("-A")
      .arg("--unstable")
      .arg(path)
      .arg(&port.to_string());

    std::thread::sleep(Duration::from_secs(5)); // wait for server to wake up.

    let load_test = test_util::prebuilt_tool_path("load_test");
    assert!(load_test.is_file());
    // ./load_test 100 0.0.0.0 8000 0 0
    // Running benchmark now...
    // Msg/sec: 161327.500000
    // Msg/sec: 163977.000000
    // ^C⏎
    let mut cmd = Command::new(load_test);
    let mut process = cmd
      .arg("100")
      .arg("0.0.0.0")
      .arg(&port.to_string())
      .arg("0")
      .arg("0")
      .spawn()
      .unwrap();

    // Let it run for 5 seconds. It won't complete so we have to kill it.
    std::thread::sleep(Duration::from_secs(5));
    process.kill().unwrap();

    let output = process.wait_with_output().unwrap();
    let stdout = std::str::from_utf8(&output.stdout).unwrap();

    let msg_per_sec = stdout
      .lines()
      .filter(|line| line.starts_with("Msg/sec:"))
      .map(|line| line.split(": ").nth(1).unwrap().parse::<f64>().unwrap())
      .max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

    res.insert(file_stem.to_string(), msg_per_sec);

  }

  Ok(res)
}
