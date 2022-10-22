// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::Result;
use std::sync::atomic::{AtomicU16, Ordering};
use std::{collections::HashMap, path::Path, process::Command, time::Duration};
pub use test_util::{parse_wrk_output, WrkOutput as HttpBenchmarkResult};
// Some of the benchmarks in this file have been renamed. In case the history
// somehow gets messed up:
//   "node_http" was once called "node"
//   "deno_tcp" was once called "deno"
//   "deno_http" was once called "deno_net_http"

const DURATION: &str = "10s";

pub fn benchmark(
  target_path: &Path,
) -> Result<HashMap<String, HttpBenchmarkResult>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let hyper_hello_exe = target_path.join("test_server");
  let hyper_hello_exe = hyper_hello_exe.to_str().unwrap();

  let core_http_json_ops_exe = target_path.join("examples/http_bench_json_ops");
  let core_http_json_ops_exe = core_http_json_ops_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let http_dir = manifest_dir.join("bench").join("http");
  for entry in std::fs::read_dir(http_dir.clone())? {
    let entry = entry?;
    let pathbuf = entry.path();
    let path = pathbuf.to_str().unwrap();
    if path.ends_with(".lua") {
      continue;
    }
    let name = entry.file_name().into_string().unwrap();
    let file_stem = pathbuf.file_stem().unwrap().to_str().unwrap();

    let lua_script = http_dir.join(format!("{}.lua", file_stem));
    let mut maybe_lua = None;
    if lua_script.exists() {
      maybe_lua = Some(lua_script.to_str().unwrap());
    }

    let port = get_port();
    if name.starts_with("node") {
      // node <path> <port>
      res.insert(
        file_stem.to_string(),
        run(
          &["node", path, &port.to_string()],
          port,
          None,
          None,
          maybe_lua,
        )?,
      );
    } else if name.starts_with("bun") && !cfg!(target_os = "windows") {
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
      #[cfg(target_os = "linux")]
      #[cfg(target_arch = "aarch64")]
      let bun_exe = test_util::prebuilt_tool_path("bun-aarch64");

      // bun <path> <port>
      res.insert(
        file_stem.to_string(),
        run(
          &[bun_exe.to_str().unwrap(), path, &port.to_string()],
          port,
          None,
          None,
          maybe_lua,
        )?,
      );
    } else {
      // deno run -A --unstable <path> <addr>
      res.insert(
        file_stem.to_string(),
        run(
          &[
            deno_exe,
            "run",
            "--allow-all",
            "--unstable",
            path,
            &server_addr(port),
          ],
          port,
          None,
          None,
          maybe_lua,
        )?,
      );
    }
  }

  // "core_http_json_ops" previously had a "bin op" counterpart called "core_http_bin_ops",
  // which was previously also called "deno_core_http_bench", "deno_core_single"
  res.insert(
    "core_http_json_ops".to_string(),
    core_http_json_ops(core_http_json_ops_exe)?,
  );
  res.insert("hyper".to_string(), hyper_http(hyper_hello_exe)?);

  Ok(res)
}

fn run(
  server_cmd: &[&str],
  port: u16,
  env: Option<Vec<(String, String)>>,
  origin_cmd: Option<&[&str]>,
  lua_script: Option<&str>,
) -> Result<HttpBenchmarkResult> {
  // Wait for port 4544 to become available.
  // TODO Need to use SO_REUSEPORT with tokio::net::TcpListener.
  std::thread::sleep(Duration::from_secs(5));

  let mut origin = None;
  if let Some(cmd) = origin_cmd {
    let mut com = Command::new(cmd[0]);
    com.args(&cmd[1..]);
    if let Some(env) = env.clone() {
      com.envs(env);
    }
    origin = Some(com.spawn()?);
  };

  println!("{}", server_cmd.join(" "));
  let mut server = {
    let mut com = Command::new(server_cmd[0]);
    com.args(&server_cmd[1..]);
    if let Some(env) = env {
      com.envs(env);
    }
    com.spawn()?
  };

  std::thread::sleep(Duration::from_secs(5)); // wait for server to wake up. TODO racy.

  let wrk = test_util::prebuilt_tool_path("wrk");
  assert!(wrk.is_file());

  let addr = format!("http://127.0.0.1:{}/", port);
  let mut wrk_cmd =
    vec![wrk.to_str().unwrap(), "-d", DURATION, "--latency", &addr];

  if let Some(lua_script) = lua_script {
    wrk_cmd.push("-s");
    wrk_cmd.push(lua_script);
  }

  println!("{}", wrk_cmd.join(" "));
  let output = test_util::run_collect(&wrk_cmd, None, None, None, true).0;

  std::thread::sleep(Duration::from_secs(1)); // wait to capture failure. TODO racy.

  println!("{}", output);
  assert!(
    server.try_wait()?.map_or(true, |s| s.success()),
    "server ended with error"
  );

  server.kill()?;
  if let Some(mut origin) = origin {
    origin.kill()?;
  }

  Ok(parse_wrk_output(&output))
}

static NEXT_PORT: AtomicU16 = AtomicU16::new(4544);
fn get_port() -> u16 {
  let p = NEXT_PORT.load(Ordering::SeqCst);
  NEXT_PORT.store(p.wrapping_add(1), Ordering::SeqCst);
  p
}

fn server_addr(port: u16) -> String {
  format!("0.0.0.0:{}", port)
}

fn core_http_json_ops(exe: &str) -> Result<HttpBenchmarkResult> {
  // let port = get_port();
  println!("http_benchmark testing CORE http_bench_json_ops");
  run(&[exe], 4570, None, None, None)
}

fn hyper_http(exe: &str) -> Result<HttpBenchmarkResult> {
  let port = get_port();
  println!("http_benchmark testing RUST hyper");
  run(&[exe, &port.to_string()], port, None, None, None)
}
