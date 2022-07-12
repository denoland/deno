// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::Result;
use std::collections::HashMap;
use std::path::Path;
use test_util::strip_ansi_codes;
pub use test_util::{parse_wrk_output, parse_deno_bench_output, WrkOutput as HttpBenchmarkResult};
use crate::http::{get_port, run};
use std::process::Stdio;
use std::process::Command;
use std::process::Output;

pub fn setup() {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let runtimes_dir = manifest_dir.join("bench").join("runtimes");
  run_and_collect_output(deno_exe, vec!["task", "setup"], &runtimes_dir);
}

pub fn ssr() -> Result<HashMap<String, HttpBenchmarkResult>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let runtimes_dir = manifest_dir.join("bench").join("runtimes");
    
  // node <path> <port>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world.node.js").to_str().unwrap().to_string();
    res.insert(
      "node".to_string(),
      run(
        &["node", &path, &port.to_string()],
        port,
        None,
        None,
        None,
      )?,
    );
  } 

  // bun run <path> <port>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world.bun.jsx").to_str().unwrap().to_string();

    // Bun does not support Windows.
    #[cfg(target_arch = "x86_64")]
    #[cfg(not(target_vendor = "apple"))]
    let bun_exe = test_util::prebuilt_tool_path("bun");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "x86_64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-x64");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "aarch64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-aarch64");

    res.insert(
      "bun".to_string(),
      run(
        &[bun_exe.to_str().unwrap(), "run", &path, &port.to_string()],
        port,
        None,
        None,
        None,
      )?,
    );
  }
  
  // deno run -A --unstable <path> <addr>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world.deno.jsx").to_str().unwrap().to_string();
    res.insert(
      "deno".to_string(),
      run(
        &[
          deno_exe,
          "run",
          "--allow-all",
          "--unstable",
          &path,
          &port.to_string(),
        ],
        port,
        None,
        None,
        None,
      )?,
    );
  }

  Ok(res)
}

pub fn sqlite() -> Result<HashMap<String, HashMap<String, f64>>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let runtimes_dir = manifest_dir.join("bench").join("runtimes");
    
  // node 
  {
    res.insert(
      "node".to_string(),
      parse_deno_bench_output(&run_and_collect_output(
        "node",
        vec!["./sqlite/query.better-sqlite3.mjs"],
        &runtimes_dir
      )),
    );
  } 

  // bun <path> <port>
  {
    // Bun does not support Windows.
    #[cfg(target_arch = "x86_64")]
    #[cfg(not(target_vendor = "apple"))]
    let bun_exe = test_util::prebuilt_tool_path("bun");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "x86_64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-x64");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "aarch64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-aarch64");

    res.insert(
      "bun".to_string(),
      parse_deno_bench_output(&run_and_collect_output(
        bun_exe.to_str().unwrap(),
        vec!["run", "./sqlite/query.bun.js"],
        &runtimes_dir
      )),
    );
  }
  
  // deno run -A --unstable <path> <addr>
  {
    res.insert(
      "deno".to_string(),
      parse_deno_bench_output(&run_and_collect_output(
        deno_exe,
        vec!["bench", "--allow-all", "--unstable", "./sqlite/query.deno.js"],
        &runtimes_dir
      )),
    );
  }

  Ok(res)
}

pub fn ffi() -> Result<HashMap<String, HashMap<String, f64>>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let runtimes_dir = manifest_dir.join("bench").join("runtimes");
    
  // TODO(bartlomieju): node

  // bun <path> <port>
  {
    // Bun does not support Windows.
    #[cfg(target_arch = "x86_64")]
    #[cfg(not(target_vendor = "apple"))]
    let bun_exe = test_util::prebuilt_tool_path("bun");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "x86_64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-x64");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "aarch64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-aarch64");

    res.insert(
      "bun".to_string(),
      parse_deno_bench_output(&run_and_collect_output(
        bun_exe.to_str().unwrap(),
        vec!["run", "./ffi/ffi.bun.js"],
        &runtimes_dir
      )),
    );
  }
  
  // deno run -A --unstable <path> <addr>
  {
    res.insert(
      "deno".to_string(),
      parse_deno_bench_output(&run_and_collect_output(
        deno_exe,
        vec!["bench", "--allow-all", "--unstable", "./ffi/ffi.deno.js"],
        &runtimes_dir
      )),
    );
  }

  Ok(res)
}

fn run_and_collect_output(
  bin: &str,
  args: Vec<&str>,
  cwd: &Path
) -> String {
  let mut command = Command::new(bin);
  command
    .args(args)
    .current_dir(cwd)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let process = command
    .spawn()
    .expect("failed to spawn script");
  let Output {
    stdout,
    stderr,
    status,
  } = process.wait_with_output().expect("failed to wait on child");
  assert!(status.success());
  let _stderr = String::from_utf8(stderr).unwrap();
  let stdout = String::from_utf8(stdout).unwrap();
  strip_ansi_codes(&stdout).to_string()
}
